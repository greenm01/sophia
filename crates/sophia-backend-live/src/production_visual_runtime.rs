use crate::*;
use sophia_engine::*;
use sophia_protocol::*;
use sophia_renderer_live::*;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub struct LiveProductionVisualRuntime {
    production: sophia_engine::ProductionSessionCoordinator,
    outputs: LiveProductionOutputRuntimeSet,
    layers: BTreeMap<SurfaceId, SurfaceTransaction>,
    presentation_feedback: crate::LiveProductionPresentFeedbackCoordinator,
    present_scheduler: LiveProductionPresentScheduler,
    present_feedback_sink: Box<dyn FnMut(crate::LivePresentFeedbackOutcome)>,
}

impl LiveProductionVisualRuntime {
    pub fn new(
        outputs: &[sophia_engine::HeadlessOutput],
        first_transactions: &[SurfaceTransaction],
        native_scanout: Option<&mut LiveProductionNativeScanout>,
        initial_native_frames: Option<Vec<LiveProductionComposedFrame>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_committed_surfaces(
            outputs,
            seed_committed_surfaces(first_transactions),
            native_scanout,
            initial_native_frames,
        )
    }

    pub fn new_with_committed_surfaces(
        outputs: &[sophia_engine::HeadlessOutput],
        committed_surfaces: Vec<CommittedSurfaceState>,
        native_scanout: Option<&mut LiveProductionNativeScanout>,
        initial_native_frames: Option<Vec<LiveProductionComposedFrame>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let production = sophia_engine::ProductionSessionCoordinator::new(
            sophia_engine::HeadlessEngine::default(),
        )
        .with_committed_surfaces(committed_surfaces.clone());
        let output_runtimes = LiveProductionOutputRuntimeSet::new(
            outputs,
            &committed_surfaces,
            native_scanout,
            initial_native_frames,
        )?;
        Ok(Self {
            production,
            outputs: output_runtimes,
            layers: BTreeMap::new(),
            presentation_feedback: Default::default(),
            present_scheduler: LiveProductionPresentScheduler::default(),
            present_feedback_sink: Box::new(|_| {}),
        })
    }

    pub fn initialize_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        frames: &[LiveProductionComposedFrame],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.outputs
            .initialize_native_scanout(native_scanout, frames)
    }

    pub fn with_present_feedback_sink(
        mut self,
        sink: impl FnMut(crate::LivePresentFeedbackOutcome) + 'static,
    ) -> Self {
        self.present_feedback_sink = Box::new(sink);
        self
    }

    pub fn with_m4_proof_controls(
        mut self,
        first_acquire_delay: Option<Duration>,
        reject_first_present: bool,
        diagnose_first_mixed_export: bool,
    ) -> Self {
        self.present_scheduler = self.present_scheduler.with_controls(
            first_acquire_delay,
            reject_first_present,
            diagnose_first_mixed_export,
        );
        self
    }

    pub fn run_cpu_production_cycle(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        scene: &mut LiveProductionCpuScene,
        updates: Vec<crate::LiveCpuBufferUpdate>,
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
        defer_frame: bool,
        output_descriptors: &[sophia_engine::HeadlessOutput],
        native_scanout: Option<&mut LiveProductionNativeScanout>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<
        (
            LiveProductionCpuCycleSubmission<crate::LiveBackendRuntimeTickReport>,
            Vec<CommittedSurfaceState>,
        ),
        Box<dyn std::error::Error>,
    > {
        self.presentation_feedback
            .observe_authority_resources(batch)?;
        self.layers
            .retain(|surface, _| !batch.removed_surfaces.contains(surface));
        for transaction in &batch.transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        let active_transactions = self.layers.values().cloned().collect::<Vec<_>>();
        if batch.transactions.iter().any(|transaction| {
            !self
                .production
                .committed_surfaces()
                .iter()
                .any(|committed| committed.surface == transaction.surface)
        }) {
            let seeded = seed_missing_committed_surfaces(
                self.production.committed_surfaces(),
                &active_transactions,
            );
            self.production.replace_committed_surfaces(seeded);
        }
        let intake = AuthorityTransactionIntake::new(batch.transaction, batch.transactions.clone())
            .with_surface_removals(batch.removed_surfaces.clone());
        let (production, outputs) = (&mut self.production, &mut self.outputs);
        let output_count = outputs.output_count();
        let event_count = authority_transaction_count(&batch.transactions);
        let mut native_scanout = native_scanout;
        let create_native_frames = native_scanout.is_some();
        let mut adapter = LiveProductionCpuCycleAdapter::new(
            scene,
            updates,
            raised_surface,
            cursor_position,
            defer_frame,
            create_native_frames,
            output_descriptors,
            move |_cycle,
                  committed: &[CommittedSurfaceState],
                  authority_commits: &[TransactionCommit],
                  native_frames: Option<Vec<LiveProductionComposedFrame>>| {
                let native_frames = native_frames.unwrap_or_default();
                if let Some(native_scanout) = native_scanout.as_deref_mut() {
                    outputs.initialize_native_scanout(native_scanout, &native_frames)?;
                }
                let mut native_frames = native_frames.into_iter();
                let mut output_adapter = crate::LiveProductionOutputRuntimeAdapter::new(
                    output_count,
                    |index,
                     snapshot: &[CommittedSurfaceState]|
                     -> Result<_, Box<dyn std::error::Error>> {
                        outputs.run_output(index, snapshot, |runtime| {
                            let input = compositor_tick_input(
                                &active_transactions,
                                event_count,
                                authority_commits.to_vec(),
                                wm_update.clone(),
                            );
                            Ok(match native_scanout.as_deref_mut() {
                                Some(native_scanout) => {
                                    if let Some(next_frame) = native_frames.next() {
                                        native_scanout.queue_frame(index, next_frame);
                                    }
                                    if runtime.rendered_primary_plane_scanout_in_flight() {
                                        runtime.run_tick(input)?
                                    } else {
                                        native_scanout.run_tick(index, runtime, input)?
                                    }
                                }
                                None => runtime.run_tick(input)?,
                            })
                        })
                    },
                );
                (0..output_count)
                    .map(|index| output_adapter.run_output(index, committed))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .next()
                    .ok_or_else(|| "persistent backend runtime has no outputs".into())
            },
        );
        let report = production
            .run_cycle(std::slice::from_ref(&intake), &mut adapter)
            .map_err(|error| {
                format!(
                    "production CPU cycle failed in phase {:?}: {}",
                    error.phase, error.source
                )
            })?;
        Ok((report.submission, report.committed_surfaces))
    }

    pub fn run_gpu_production_cycle(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        scene: &mut LiveProductionCpuScene,
        updates: Vec<crate::LiveCpuBufferUpdate>,
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
        defer_frame: bool,
        output_descriptors: &[sophia_engine::HeadlessOutput],
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<(LiveProductionCpuSubmission, Vec<CommittedSurfaceState>), Box<dyn std::error::Error>>
    {
        let committed_surfaces = self.committed_surfaces().to_vec();
        scene.apply_updates(updates, &committed_surfaces)?;
        let compose_started = Instant::now();
        let composition = if defer_frame {
            scene
                .last_report()
                .cloned()
                .ok_or("software redraw coalescing has no prior composed frame")?
        } else {
            scene
                .compose(&committed_surfaces, raised_surface, cursor_position)?
                .clone()
        };
        let native_frames = if defer_frame {
            None
        } else {
            native_scanout
                .as_ref()
                .map(|_| scene.frames_for_outputs(output_descriptors))
                .transpose()?
        };
        if let (Some(native_scanout), Some(frames)) =
            (native_scanout.as_deref_mut(), native_frames.as_ref())
        {
            self.initialize_native_scanout(native_scanout, frames)?;
        }
        let tick = self.run_batch(
            batch,
            if defer_frame { None } else { native_scanout },
            native_frames,
            wm_update,
        )?;
        Ok((
            LiveProductionCpuSubmission {
                tick,
                composition,
                composed: !defer_frame,
                compose_elapsed: if defer_frame {
                    Duration::ZERO
                } else {
                    compose_started.elapsed()
                },
            },
            committed_surfaces,
        ))
    }

    pub fn run_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        self.presentation_feedback
            .observe_authority_resources(batch)?;
        if !batch.present_submissions.is_empty() {
            let cpu_background = native_frames
                .as_ref()
                .and_then(|frames| frames.first())
                .map(|frame| frame.frame.clone());
            self.present_scheduler.enqueue_batch(
                batch,
                cpu_background,
                self.presentation_feedback.resources_mut(),
                Instant::now(),
            )?;
            self.layers
                .retain(|surface, _| !batch.removed_surfaces.contains(surface));
            for transaction in &batch.transactions {
                self.layers.insert(transaction.surface, transaction.clone());
            }
            return self.drive_gpu_presentation(native_scanout.as_deref_mut());
        }
        self.run_authority_transactions(
            batch.transaction,
            &batch.transactions,
            &batch.removed_surfaces,
            authority_transaction_count(&batch.transactions),
            native_scanout,
            native_frames,
            wm_update,
        )
    }

    pub fn drive_gpu_presentation(
        &mut self,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transaction = match self
            .present_scheduler
            .poll_gate(self.presentation_feedback.resources_mut(), Instant::now())?
        {
            LiveProductionPresentGate::Idle
            | LiveProductionPresentGate::SubmittedInFlight
            | LiveProductionPresentGate::WaitingAcquire => {
                return self.run_observation_tick();
            }
            LiveProductionPresentGate::Reject(transaction) => {
                self.reject_gpu_presentation(transaction, 0, 0);
                return self.run_observation_tick();
            }
            LiveProductionPresentGate::Ready(transaction) => transaction,
        };
        let Some(native_scanout) = native_scanout.as_deref_mut() else {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
            return self.run_observation_tick();
        };
        if self.native_scanout_in_flight() {
            return self.run_observation_tick();
        }
        let queued = self
            .present_scheduler
            .front()
            .ok_or("ready Present gate has no queued presentation")?;
        let queued_surface = queued.surface;

        let prepared = self
            .production
            .prepare_full_state_present(transaction, &queued.transactions);
        if !prepared.is_ready() {
            self.present_scheduler.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
            return self.run_observation_tick();
        }
        let mixed = self.presentation_feedback.resources().build_mixed_frame(
            transaction,
            queued.cpu_background.clone(),
            queued.target,
            None,
            1.0,
        )?;
        if self.present_scheduler.take_diagnose_first_mixed_export() {
            let (cpu_layers, dmabuf_layers) =
                mixed
                    .layers
                    .iter()
                    .fold((0usize, 0usize), |(cpu, dmabuf), layer| match layer {
                        crate::LiveOwnedMixedCompositionLayer::Cpu { .. } => {
                            (cpu.saturating_add(1), dmabuf)
                        }
                        crate::LiveOwnedMixedCompositionLayer::DmaBuf { .. } => {
                            (cpu, dmabuf.saturating_add(1))
                        }
                    });
            let (status, detail) = native_scanout.diagnose_mixed_frame(0, mixed);
            self.present_scheduler.pop_front();
            let _ = self
                .presentation_feedback
                .resources_mut()
                .reject(transaction);
            let _ = self.presentation_feedback.disconnect();
            return Err(Box::new(crate::LiveNativeMixedDiagnosticComplete {
                status,
                detail,
                cpu_layers,
                dmabuf_layers,
                live_sources: self.presentation_feedback.resources().source_count(),
                live_fences: self.presentation_feedback.resources().fence_count(),
                live_transactions: self.presentation_feedback.resources().presentation_count(),
            }));
        }
        native_scanout.queue_mixed_frame(0, mixed);

        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            1,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                Ok(native_scanout.run_tick(
                    index,
                    &mut output.runtime,
                    compositor_tick_input(&transactions, 0, Vec::new(), None),
                )?)
            },
        );
        let report = production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        use crate::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
        match report
            .rendered_primary_plane_scanout_submit
            .map(|submit| submit.status)
        {
            Some(Status::SubmittedWaitingForPageFlip) => {
                // run_tick polls callbacks before it submits this frame. Any
                // feedback already queued here therefore belongs to an older
                // CPU frame and must not retire this Present transaction.
                native_scanout.discard_presentation_feedback(self.outputs.primary_output());
                self.presentation_feedback
                    .resources_mut()
                    .mark_submitted(transaction)?;
                self.present_scheduler.pop_front();
                self.present_scheduler
                    .mark_submitted(LiveProductionSubmittedPresent {
                        transaction,
                        surface: queued_surface,
                        prepared,
                    });
            }
            Some(Status::AlreadyInFlight | Status::CleanupPending) | None => {}
            Some(_) => {
                self.present_scheduler.pop_front();
                self.reject_gpu_presentation(transaction, 0, 0);
            }
        }
        Ok(report)
    }

    pub fn run_cpu_repaint(
        &mut self,
        scene: &mut LiveProductionCpuScene,
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
        output_descriptors: &[sophia_engine::HeadlessOutput],
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<LiveProductionCpuSubmission, Box<dyn std::error::Error>> {
        let committed = self.production.committed_surfaces().to_vec();
        let compose_started = Instant::now();
        let composition = scene
            .compose(&committed, raised_surface, cursor_position)?
            .clone();
        let frames = scene.frames_for_outputs(output_descriptors)?;
        self.initialize_native_scanout(native_scanout, &frames)?;
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut frames = frames.into_iter();
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, snapshot: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                outputs.run_output(index, snapshot, |runtime| {
                    if let Some(frame) = frames.next() {
                        native_scanout.queue_frame(index, frame);
                    }
                    let input = compositor_tick_input(&transactions, 0, Vec::new(), None);
                    Ok(if runtime.rendered_primary_plane_scanout_in_flight() {
                        runtime.run_tick(input)?
                    } else {
                        native_scanout.run_tick(index, runtime, input)?
                    })
                })
            },
        );
        let tick = production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        Ok(LiveProductionCpuSubmission {
            tick,
            composition,
            composed: true,
            compose_elapsed: compose_started.elapsed(),
        })
    }

    pub fn run_observation_tick(
        &mut self,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output = self
            .outputs
            .values_mut()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        Ok(output
            .runtime
            .run_tick(compositor_tick_input(&transactions, 0, Vec::new(), None))?)
    }

    pub fn reject_gpu_presentation(&mut self, transaction: TransactionId, ust: u64, msc: u64) {
        if let Ok(outcome) = self
            .presentation_feedback
            .reject_skip(transaction, ust, msc)
        {
            self.route_present_feedback(outcome);
        }
    }

    pub fn route_present_feedback(&mut self, outcome: crate::LivePresentFeedbackOutcome) {
        (self.present_feedback_sink)(outcome);
    }

    pub fn shutdown_presentations(&mut self) -> crate::LivePresentationDisconnectReport {
        let queued = self.present_scheduler.drain_transactions();
        for transaction in queued {
            self.reject_gpu_presentation(transaction, 0, 0);
        }
        if let Some(submitted) = self.present_scheduler.take_submitted() {
            self.reject_gpu_presentation(submitted.transaction, 0, 0);
        }

        self.presentation_feedback.disconnect()
    }

    pub fn prepare_authority_transactions(
        &mut self,
        transaction_id: TransactionId,
        transactions: &[SurfaceTransaction],
        removed_surfaces: &[SurfaceId],
    ) -> Result<LiveProductionPreparedAuthorityBatch, Box<dyn std::error::Error>> {
        self.layers
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for transaction in transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        let intake = AuthorityTransactionIntake::new(transaction_id, transactions.to_vec())
            .with_surface_removals(removed_surfaces.to_vec());
        let active_transactions = self.layers.values().cloned().collect::<Vec<_>>();
        if transactions.iter().any(|transaction| {
            !self
                .production
                .committed_surfaces()
                .iter()
                .any(|committed| committed.surface == transaction.surface)
        }) {
            let seeded = seed_missing_committed_surfaces(
                self.production.committed_surfaces(),
                &active_transactions,
            );
            self.production.replace_committed_surfaces(seeded);
        }
        let authority_commits = self
            .production
            .commit_authority_batches(std::slice::from_ref(&intake));
        Ok(LiveProductionPreparedAuthorityBatch {
            authority_commits,
            active_transactions,
        })
    }

    pub fn run_prepared_authority_transactions(
        &mut self,
        prepared: LiveProductionPreparedAuthorityBatch,
        event_count: usize,
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut native_frames = native_frames.unwrap_or_default().into_iter();
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                let input = compositor_tick_input(
                    &prepared.active_transactions,
                    event_count,
                    prepared.authority_commits.clone(),
                    wm_update.clone(),
                );
                Ok(match native_scanout.as_deref_mut() {
                    Some(native_scanout) => {
                        if let Some(frame) = native_frames.next() {
                            native_scanout.queue_frame(index, frame);
                        }
                        if output.runtime.rendered_primary_plane_scanout_in_flight() {
                            output.runtime.run_tick(input)?
                        } else {
                            native_scanout.run_tick(index, &mut output.runtime, input)?
                        }
                    }
                    None => output.runtime.run_tick(input)?,
                })
            },
        );
        production
            .run_outputs(&mut adapter)?
            .into_iter()
            .next()
            .ok_or_else(|| "persistent backend runtime has no outputs".into())
    }

    pub fn run_authority_transactions(
        &mut self,
        transaction_id: TransactionId,
        transactions: &[SurfaceTransaction],
        removed_surfaces: &[SurfaceId],
        event_count: usize,
        native_scanout: Option<&mut LiveProductionNativeScanout>,
        native_frames: Option<Vec<LiveProductionComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let prepared =
            self.prepare_authority_transactions(transaction_id, transactions, removed_surfaces)?;
        self.run_prepared_authority_transactions(
            prepared,
            event_count,
            native_scanout,
            native_frames,
            wm_update,
        )
    }

    pub fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        self.production.committed_surfaces()
    }

    pub fn input_layers(&self) -> Vec<LayerSnapshot> {
        self.layers
            .values()
            .enumerate()
            .map(|(index, transaction)| LayerSnapshot {
                surface: transaction.surface,
                authority_local_id: None,
                namespace: None,
                stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
                geometry: transaction.target_geometry,
                source: transaction.target_buffer,
                damage: transaction.damage.clone(),
                opacity: 1.0,
                crop: None,
                transform: Transform::IDENTITY,
                generation: transaction.previous_committed_generation,
                resize_sync: ResizeSyncCapability::ImplicitOnly,
            })
            .collect()
    }

    pub fn drain_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + timeout;
        while self.native_scanout_in_flight() && Instant::now() < deadline {
            self.retire_native_scanout(native_scanout)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.native_scanout_in_flight() {
            return Err("persistent native scanout remained in flight during teardown".into());
        }
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                native_scanout.release_displayed_output(index, &mut output.runtime)
            },
        );
        let _ = production.run_outputs(&mut adapter)?;
        Ok(())
    }

    pub fn run_native_idle(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<crate::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                if !native_scanout.pending_frame(index) {
                    return Ok(None);
                }
                Ok(Some(native_scanout.run_tick(
                    index,
                    &mut output.runtime,
                    compositor_tick_input(&transactions, 0, Vec::new(), None),
                )?))
            },
        );
        production
            .run_outputs(&mut adapter)?
            .into_iter()
            .flatten()
            .next()
            .ok_or_else(|| "persistent native idle tick had no pending output".into())
    }

    pub fn retire_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let output_count = self.outputs.output_count();
        let production = &self.production;
        let outputs = &mut self.outputs;
        let mut adapter = crate::LiveProductionOutputRuntimeAdapter::new(
            output_count,
            |index, committed: &[CommittedSurfaceState]| -> Result<_, Box<dyn std::error::Error>> {
                let output = outputs
                    .values_mut()
                    .nth(index)
                    .ok_or("production output index was not registered")?;
                output
                    .runtime
                    .assembly_mut()
                    .replace_committed_surfaces(committed.to_vec());
                native_scanout.retire_ready_and_retry_cleanup(index, &mut output.runtime)
            },
        );
        let _ = production.run_outputs(&mut adapter)?;
        if let Some(primary) = self.outputs.primary_output()
            && let Some((ust, msc)) = native_scanout.take_presentation_feedback(primary)
        {
            return self.finalize_gpu_page_flip(ust, msc);
        }
        Ok(None)
    }

    pub fn finalize_gpu_page_flip(
        &mut self,
        ust: u64,
        msc: u64,
    ) -> Result<Option<LiveProductionRetiredPresent>, Box<dyn std::error::Error>> {
        let Some(submitted) = self.present_scheduler.take_submitted() else {
            return Ok(None);
        };
        let (production, outputs, presentation_feedback) = (
            &mut self.production,
            &mut self.outputs,
            &mut self.presentation_feedback,
        );
        let completion = production
            .complete_prepared_retirement(submitted.prepared, || {
                presentation_feedback.complete_flip(submitted.transaction, ust, msc)
            })
            .map_err(|error| format!("page flip prepared retirement failed: {error:?}"))?;
        outputs.project_committed(&completion.committed_surfaces);
        let outcome = completion.evidence;
        self.route_present_feedback(outcome);
        Ok(Some(LiveProductionRetiredPresent {
            transaction: submitted.transaction,
            surface: submitted.surface,
        }))
    }

    pub fn native_scanout_in_flight(&self) -> bool {
        self.outputs.native_scanout_in_flight()
    }

    pub fn native_cleanup_pending(&self) -> bool {
        self.outputs.native_cleanup_pending()
    }

    pub fn native_diagnostic(&self) -> String {
        self.outputs.diagnostic()
    }
}
fn compositor_tick_input(
    transactions: &[SurfaceTransaction],
    x_event_count: usize,
    authority_commits: Vec<TransactionCommit>,
    wm_update: Option<WmTransactionUpdate>,
) -> CompositorBackendTickInput {
    CompositorBackendTickInput {
        x_event_count: u32::try_from(x_event_count).unwrap_or(u32::MAX),
        authority_commits,
        authority_batches: Vec::new(),
        wm_update,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layer_templates: layer_templates_from_surface_transactions(transactions),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    }
}

fn seed_committed_surfaces(transactions: &[SurfaceTransaction]) -> Vec<CommittedSurfaceState> {
    seed_missing_committed_surfaces(&[], transactions)
}

fn seed_missing_committed_surfaces(
    existing: &[CommittedSurfaceState],
    transactions: &[SurfaceTransaction],
) -> Vec<CommittedSurfaceState> {
    let mut surfaces = existing
        .iter()
        .cloned()
        .map(|surface| (surface.surface, surface))
        .collect::<BTreeMap<_, _>>();
    for transaction in transactions {
        surfaces
            .entry(transaction.surface)
            .or_insert(CommittedSurfaceState {
                surface: transaction.surface,
                committed_generation: transaction.previous_committed_generation,
                geometry: transaction.target_geometry,
                buffer: transaction.target_buffer,
                damage: Region::empty(),
            });
    }
    surfaces.into_values().collect()
}

fn layer_templates_from_surface_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<LayerSnapshot> {
    transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| LayerSnapshot {
            surface: transaction.surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
            geometry: transaction.target_geometry,
            source: BufferSource::None,
            damage: transaction.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: transaction.previous_committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        })
        .collect()
}

fn authority_transaction_count(transactions: &[SurfaceTransaction]) -> usize {
    transactions.len()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionVisualDiagnostics {
    pub present_queued: bool,
    pub live_sources: usize,
    pub live_fences: usize,
    pub live_presentations: usize,
    pub acquire_waits: usize,
    pub controlled_rejections: usize,
}

impl LiveProductionVisualRuntime {
    pub fn diagnostics(&self) -> LiveProductionVisualDiagnostics {
        LiveProductionVisualDiagnostics {
            present_queued: self.present_scheduler.has_queued(),
            live_sources: self.presentation_feedback.resources().source_count(),
            live_fences: self.presentation_feedback.resources().fence_count(),
            live_presentations: self.presentation_feedback.resources().presentation_count(),
            acquire_waits: self.present_scheduler.acquire_waits(),
            controlled_rejections: self.present_scheduler.controlled_rejections(),
        }
    }

    pub fn replace_output_projection(
        &mut self,
        index: usize,
        committed: Vec<CommittedSurfaceState>,
    ) -> bool {
        self.outputs.replace_output_projection(index, committed)
    }

    pub fn output_count(&self) -> usize {
        self.outputs.output_count()
    }
    pub fn output_committed(&self, index: usize) -> Option<&[CommittedSurfaceState]> {
        self.outputs.output_committed(index)
    }
}

#[derive(Debug)]
pub struct LiveProductionRetiredPresent {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
}

#[derive(Debug)]
pub struct LiveProductionNativeServiceReport {
    pub tick: Option<LiveBackendRuntimeTickReport>,
    pub retired_present: Option<LiveProductionRetiredPresent>,
    pub retirement_polled: bool,
    pub present_polled: bool,
    pub pending_frame_polled: bool,
}

impl LiveProductionVisualRuntime {
    pub fn service_native(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
    ) -> Result<LiveProductionNativeServiceReport, Box<dyn std::error::Error>> {
        let mut coordinator = ProductionAsyncServiceCoordinator::new();
        let mut tick = None;
        let mut retired_present = None;
        let mut retirement_polled = false;
        let mut present_polled = false;
        let mut pending_frame_polled = false;
        loop {
            let phase = coordinator.next_phase(ProductionAsyncServiceObservation {
                native_in_flight: self.native_scanout_in_flight(),
                cleanup_pending: self.native_cleanup_pending(),
                present_queued: self.diagnostics().present_queued,
                pending_frame: (0..self.output_count())
                    .any(|index| native_scanout.pending_frame(index)),
            });
            match phase {
                Some(ProductionAsyncServicePhase::KmsRetire) => {
                    retirement_polled = true;
                    retired_present = self.retire_native_scanout(native_scanout)?;
                }
                Some(ProductionAsyncServicePhase::SchedulePresent) => {
                    present_polled = true;
                    tick = Some(self.drive_gpu_presentation(Some(native_scanout))?);
                }
                Some(ProductionAsyncServicePhase::SubmitPendingFrame) => {
                    pending_frame_polled = true;
                    tick = Some(self.run_native_idle(native_scanout)?);
                }
                None => break,
            }
        }
        Ok(LiveProductionNativeServiceReport {
            tick,
            retired_present,
            retirement_polled,
            present_polled,
            pending_frame_polled,
        })
    }
}

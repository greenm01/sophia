use super::*;

pub struct LiveBackendRuntimeAssembly<P = QueuedInputPoller> {
    pub(crate) assembly: HeadlessCompositorBackendAssembly<P>,
    pub(crate) renderer_observation: LiveRendererRuntimeObservation,
    pub(crate) output_size: Option<Size>,
    pub(crate) scanout_readiness: LiveScanoutReadinessReport,
    pub(crate) kms_scanout_target: LiveKmsScanoutTargetReport,
    pub(crate) gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    pub(crate) gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub(crate) gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    pub(crate) page_flip_event: LivePageFlipEvent,
    pub(crate) page_flip_callback_intake: LivePageFlipCallbackIntake,
    pub(crate) page_flip_callback_queue: Option<LivePageFlipCallbackQueue>,
    pub(crate) libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_submission:
        Option<BoxedRenderedPrimaryPlaneScanoutSubmission>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_cleanup:
        Option<BoxedRenderedPrimaryPlaneScanoutCleanup>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_runtime_scanout_state: Option<RuntimeScanoutState>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_in_flight_ticks: u64,
    #[cfg(feature = "libdrm-events")]
    pub(crate) pending_runtime_scanout_states: VecDeque<RuntimeScanoutState>,
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly<P> {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly<P> {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
    }

    pub fn with_page_flip_callback_queue(mut self, queue: LivePageFlipCallbackQueue) -> Self {
        self.page_flip_callback_queue = Some(queue);
        self
    }

    pub fn with_libdrm_poller_diagnostics(
        mut self,
        diagnostics: LiveLibdrmPollerDiagnostics,
    ) -> Self {
        self.libdrm_poller_diagnostics = diagnostics;
        self
    }

    #[cfg(feature = "libdrm-events")]
    pub fn with_native_libdrm_poller_diagnostics(
        self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) -> Self {
        self.with_libdrm_poller_diagnostics(diagnostics.into())
    }

    pub fn observe_libdrm_poller_diagnostics(&mut self, diagnostics: LiveLibdrmPollerDiagnostics) {
        self.libdrm_poller_diagnostics = diagnostics;
    }

    #[cfg(feature = "libdrm-events")]
    pub fn observe_native_libdrm_poller_diagnostics(
        &mut self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) {
        self.observe_libdrm_poller_diagnostics(diagnostics.into());
    }

    pub fn libdrm_poller_diagnostics(&self) -> LiveLibdrmPollerDiagnostics {
        self.libdrm_poller_diagnostics
    }

    #[cfg(feature = "libdrm-events")]
    pub fn rendered_primary_plane_scanout_in_flight(&self) -> bool {
        self.rendered_primary_plane_scanout_submission.is_some()
    }

    #[cfg(feature = "libdrm-events")]
    pub fn rendered_primary_plane_scanout_cleanup_pending(&self) -> bool {
        self.rendered_primary_plane_scanout_cleanup.is_some()
    }

    #[cfg(feature = "libdrm-events")]
    pub const fn rendered_primary_plane_scanout_in_flight_ticks(&self) -> u64 {
        self.rendered_primary_plane_scanout_in_flight_ticks
    }

    #[cfg(feature = "libdrm-events")]
    pub fn rendered_primary_plane_scanout_backpressure_report(
        &self,
        threshold_ticks: u64,
    ) -> LiveRenderedPrimaryPlaneScanoutBackpressureReport {
        LiveRenderedPrimaryPlaneScanoutBackpressureReport::from_in_flight_state(
            self.rendered_primary_plane_scanout_in_flight(),
            self.rendered_primary_plane_scanout_in_flight_ticks,
            threshold_ticks,
        )
    }

    #[cfg(feature = "libdrm-events")]
    pub fn rendered_primary_plane_runtime_scanout_state(&self) -> Option<RuntimeScanoutState> {
        self.rendered_primary_plane_runtime_scanout_state
    }

    #[cfg(feature = "libdrm-events")]
    pub fn pending_runtime_scanout_state_count(&self) -> usize {
        self.pending_runtime_scanout_states.len()
    }

    pub fn scanout_readiness_observation(&self) -> LiveScanoutReadinessReport {
        self.scanout_readiness
    }

    pub fn kms_scanout_target_observation(&self) -> LiveKmsScanoutTargetReport {
        self.kms_scanout_target
    }

    pub fn gbm_egl_frame_target_observation(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.gbm_egl_frame_target
    }

    pub fn gbm_egl_frame_target_lifecycle_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.gbm_egl_frame_target_lifecycle
    }

    pub fn gbm_egl_frame_target_allocation_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport> {
        self.gbm_egl_frame_target_allocation
    }

    pub fn observe_gbm_egl_frame_target_size(&mut self, size: Size) -> LiveGbmEglFrameTargetRecord {
        let previous = self.gbm_egl_frame_target;
        let record = LiveGbmEglFrameTargetRecord::new(size);
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::from_size_update(previous, record);
        self.gbm_egl_frame_target = Some(record);
        self.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        if lifecycle.status != LiveGbmEglFrameTargetLifecycleStatus::Retained {
            self.gbm_egl_frame_target_allocation = None;
        }
        self.refresh_kms_scanout_target(LiveRendererPresentationReport {
            status: match self.scanout_readiness.status {
                LiveScanoutReadinessStatus::Ready => LiveRendererPresentationStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable
                | LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LiveRendererPresentationStatus::Unavailable
                }
                LiveScanoutReadinessStatus::Degraded => LiveRendererPresentationStatus::Degraded,
            },
        });
        record
    }

    pub fn retire_gbm_egl_frame_target(&mut self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        let target = self.gbm_egl_frame_target.take()?;
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::retired(target);
        self.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        self.gbm_egl_frame_target_allocation = None;
        self.refresh_kms_scanout_target(LiveRendererPresentationReport {
            status: match self.scanout_readiness.status {
                LiveScanoutReadinessStatus::Ready => LiveRendererPresentationStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable
                | LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LiveRendererPresentationStatus::Unavailable
                }
                LiveScanoutReadinessStatus::Degraded => LiveRendererPresentationStatus::Degraded,
            },
        });
        Some(lifecycle)
    }

    pub fn allocate_gbm_egl_frame_target<A>(
        &mut self,
        allocator: &mut A,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        A: LiveGbmEglFrameTargetAllocator,
    {
        let target = self.gbm_egl_frame_target?;
        let report =
            allocator.allocate_frame_target(LiveGbmEglFrameTargetAllocationRequest { target });
        self.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn allocate_native_gbm_egl_frame_target_with_gbm_device<D>(
        &mut self,
        discovery: &D,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let target = self.gbm_egl_frame_target?;
        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
                discovery.open_render_device(),
                LiveGbmEglFrameTargetAllocationRequest { target },
            );
        self.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    #[cfg(feature = "libdrm-events")]
    pub fn submit_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
        device: &D,
        exporter: &mut E,
    ) -> LiveRenderedPrimaryPlaneScanoutSubmitResult<E::Owner>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
    {
        submit_rendered_primary_plane_scanout_from_target_with(
            self.gbm_egl_frame_target,
            device,
            exporter,
        )
    }

    #[cfg(feature = "libdrm-events")]
    pub fn submit_and_track_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
        device: &D,
        exporter: &mut E,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
    {
        let cleanup_pending = self.rendered_primary_plane_scanout_cleanup_pending();
        track_rendered_primary_plane_scanout_submit_from_target_with(
            self.gbm_egl_frame_target,
            &mut self.rendered_primary_plane_scanout_submission,
            &mut self.rendered_primary_plane_runtime_scanout_state,
            &mut self.rendered_primary_plane_scanout_in_flight_ticks,
            cleanup_pending,
            self.page_flip_callback_intake.last_frame_serial(),
            Some(&mut self.pending_runtime_scanout_states),
            device,
            exporter,
        )
    }

    #[cfg(feature = "libdrm-events")]
    pub fn retire_tracked_rendered_primary_plane_scanout_after_page_flip<D>(
        &mut self,
        device: &D,
        callback: &LivePageFlipCallbackReport,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutRetireReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        let Some(submission) = self.rendered_primary_plane_scanout_submission.take() else {
            return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
                status: LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission,
                runtime_scanout_state: None,
                in_flight: false,
                in_flight_ticks: 0,
                cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
            };
        };

        let retired =
            retire_rendered_primary_plane_scanout_after_page_flip(device, submission, callback);
        let runtime_scanout_state = retired.runtime_scanout_state();
        if let Some(submission) = retired.submission {
            self.rendered_primary_plane_scanout_submission = Some(submission);
        } else {
            self.rendered_primary_plane_scanout_in_flight_ticks = 0;
        }
        if let Some(cleanup) = retired.cleanup {
            self.rendered_primary_plane_scanout_cleanup = Some(cleanup);
        }
        if let Some(runtime_scanout_state) = runtime_scanout_state {
            self.rendered_primary_plane_runtime_scanout_state = Some(runtime_scanout_state);
            self.pending_runtime_scanout_states
                .push_back(runtime_scanout_state);
        }

        LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
            status: retired.status.into(),
            runtime_scanout_state,
            in_flight: self.rendered_primary_plane_scanout_in_flight(),
            in_flight_ticks: self.rendered_primary_plane_scanout_in_flight_ticks,
            cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
        }
    }

    #[cfg(feature = "libdrm-events")]
    pub fn retry_tracked_rendered_primary_plane_scanout_cleanup<D>(
        &mut self,
        device: &D,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        let Some(cleanup) = self.rendered_primary_plane_scanout_cleanup.take() else {
            return LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
                status: LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending,
                destroy: None,
                cleanup_pending: false,
            };
        };

        let retried = retry_rendered_primary_plane_scanout_cleanup(device, cleanup);
        let status = if retried.cleanup.is_some() {
            LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanupFailed
        } else {
            LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
        };
        if let Some(cleanup) = retried.cleanup {
            self.rendered_primary_plane_scanout_cleanup = Some(cleanup);
        }

        LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
            status,
            destroy: Some(retried.destroy),
            cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
        }
    }

    pub fn page_flip_observation(&self) -> LivePageFlipEvent {
        self.page_flip_event
    }

    pub fn observe_presentation_report(&mut self, presentation: LiveRendererPresentationReport) {
        self.scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        self.refresh_kms_scanout_target(presentation);
    }

    pub fn observe_page_flip_outcome(&mut self, outcome: &PageFlipCommitOutcome) {
        self.page_flip_event = LivePageFlipEvent::from_commit_outcome(outcome);
    }

    pub fn observe_atomic_scanout_commit(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report = LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_with<C>(
        &mut self,
        committer: &mut C,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let report = committer.commit_atomic_scanout(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_after_page_flip_with<C>(
        &mut self,
        committer: &mut C,
        callback: LivePageFlipCallback,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let callback_report = self.page_flip_callback_intake.observe(callback);
        let report = committer.commit_atomic_scanout_after_page_flip(&callback_report, outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn observe_page_flip_callback(
        &mut self,
        callback: LivePageFlipCallback,
    ) -> LivePageFlipCallbackReport {
        let report = self.page_flip_callback_intake.observe(callback);
        self.page_flip_event = report.event;
        report
    }

    pub fn run_tick(
        &mut self,
        mut input: CompositorBackendTickInput,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError> {
        let page_flip_callbacks = self
            .page_flip_callback_queue
            .as_ref()
            .map(|queue| {
                queue.drain_ready(
                    &mut self.page_flip_callback_intake,
                    &mut self.page_flip_event,
                )
            })
            .unwrap_or_default();

        #[cfg(feature = "libdrm-events")]
        self.advance_rendered_primary_plane_scanout_age_if_in_flight();
        #[cfg(feature = "libdrm-events")]
        let runtime_scanout_states: Vec<RuntimeScanoutState> =
            self.pending_runtime_scanout_states.drain(..).collect();
        #[cfg(not(feature = "libdrm-events"))]
        let runtime_scanout_states: Vec<RuntimeScanoutState> = Vec::new();
        input
            .scanout_lifecycle_states
            .extend(runtime_scanout_states.iter().copied());

        let engine = self.assembly.run_tick(input)?;

        Ok(LiveBackendRuntimeTickReport {
            engine,
            renderer: self.renderer_observation,
            scanout: self.scanout_readiness,
            kms_scanout_target: self.kms_scanout_target,
            gbm_egl_frame_target: self.gbm_egl_frame_target,
            gbm_egl_frame_target_lifecycle: self.gbm_egl_frame_target_lifecycle,
            gbm_egl_frame_target_allocation: self.gbm_egl_frame_target_allocation,
            page_flip: self.page_flip_event,
            page_flip_callbacks,
            runtime_scanout_states,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_pending: self
                .rendered_primary_plane_scanout_cleanup_pending(),
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_retry: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_in_flight_ticks: self
                .rendered_primary_plane_scanout_in_flight_ticks,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_submit: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_retire: None,
            libdrm_poller: self.libdrm_poller_diagnostics,
        })
    }

    #[cfg(feature = "libdrm-events")]
    pub fn run_tick_with_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
        mut input: CompositorBackendTickInput,
        device: &D,
        exporter: &mut E,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
    {
        let rendered_primary_plane_scanout_cleanup_retry = self
            .rendered_primary_plane_scanout_cleanup_pending()
            .then(|| self.retry_tracked_rendered_primary_plane_scanout_cleanup(device));
        let page_flip_callbacks = self
            .page_flip_callback_queue
            .as_ref()
            .map(|queue| {
                queue.drain_ready(
                    &mut self.page_flip_callback_intake,
                    &mut self.page_flip_event,
                )
            })
            .unwrap_or_default();
        let rendered_primary_plane_scanout_retire =
            page_flip_callbacks.last_accepted.map(|callback| {
                self.retire_tracked_rendered_primary_plane_scanout_after_page_flip(
                    device, &callback,
                )
            });
        self.advance_rendered_primary_plane_scanout_age_if_in_flight();
        let runtime_scanout_states: Vec<RuntimeScanoutState> =
            self.pending_runtime_scanout_states.drain(..).collect();
        input
            .scanout_lifecycle_states
            .extend(runtime_scanout_states.iter().copied());

        let target = self.gbm_egl_frame_target;
        let rendered_primary_plane_scanout_cleanup_pending =
            self.rendered_primary_plane_scanout_cleanup_pending();
        let rendered_primary_plane_scanout_submission =
            &mut self.rendered_primary_plane_scanout_submission;
        let rendered_primary_plane_runtime_scanout_state =
            &mut self.rendered_primary_plane_runtime_scanout_state;
        let rendered_primary_plane_scanout_in_flight_ticks =
            &mut self.rendered_primary_plane_scanout_in_flight_ticks;
        let submitted_after_page_flip_serial = self.page_flip_callback_intake.last_frame_serial();
        let mut rendered_primary_plane_scanout_submit = None;

        let engine = self.assembly.run_tick_with_live_runtime_adapter(
            input,
            |engine: &HeadlessEngine, intake: LiveRuntimeDriverIntake| {
                let inner = LiveRuntimeDriverAdapter::from_authority_batches(engine, intake);
                LiveRenderedPrimaryPlaneRuntimeAdapter {
                    inner,
                    target,
                    rendered_primary_plane_scanout_submission,
                    rendered_primary_plane_runtime_scanout_state,
                    rendered_primary_plane_scanout_in_flight_ticks,
                    cleanup_pending: rendered_primary_plane_scanout_cleanup_pending,
                    submitted_after_page_flip_serial,
                    device,
                    exporter,
                    submit_report: &mut rendered_primary_plane_scanout_submit,
                }
            },
            |adapter| adapter.inner.renderer.committed_surfaces.clone(),
        )?;

        Ok(LiveBackendRuntimeTickReport {
            engine,
            renderer: self.renderer_observation,
            scanout: self.scanout_readiness,
            kms_scanout_target: self.kms_scanout_target,
            gbm_egl_frame_target: self.gbm_egl_frame_target,
            gbm_egl_frame_target_lifecycle: self.gbm_egl_frame_target_lifecycle,
            gbm_egl_frame_target_allocation: self.gbm_egl_frame_target_allocation,
            page_flip: self.page_flip_event,
            page_flip_callbacks,
            runtime_scanout_states,
            rendered_primary_plane_scanout_cleanup_pending: self
                .rendered_primary_plane_scanout_cleanup_pending(),
            rendered_primary_plane_scanout_cleanup_retry,
            rendered_primary_plane_scanout_in_flight_ticks: self
                .rendered_primary_plane_scanout_in_flight_ticks,
            rendered_primary_plane_scanout_submit,
            rendered_primary_plane_scanout_retire,
            libdrm_poller: self.libdrm_poller_diagnostics,
        })
    }

    #[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_with<D, R>(
        &mut self,
        input: CompositorBackendTickInput,
        device: &D,
        discovery: &R,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        R: RenderDeviceDiscoveryBackend,
    {
        let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery);
        self.run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
            input,
            device,
            &mut exporter,
        )
    }

    #[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with<D, R>(
        &mut self,
        input: CompositorBackendTickInput,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<R>,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        R: RenderDeviceDiscoveryBackend,
    {
        self.run_tick_with_rendered_primary_plane_scanout_with(input, device, exporter)
    }
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    fn refresh_kms_scanout_target(&mut self, presentation: LiveRendererPresentationReport) {
        self.kms_scanout_target = LiveKmsScanoutTargetReport::from_output_target_and_presentation(
            self.output_size,
            self.gbm_egl_frame_target,
            presentation,
        );
        self.page_flip_event =
            LivePageFlipEvent::from_kms_scanout_target_status(self.kms_scanout_target.status);
    }

    #[cfg(feature = "libdrm-events")]
    fn advance_rendered_primary_plane_scanout_age_if_in_flight(&mut self) {
        if self.rendered_primary_plane_scanout_submission.is_some() {
            self.rendered_primary_plane_scanout_in_flight_ticks = self
                .rendered_primary_plane_scanout_in_flight_ticks
                .saturating_add(1);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeTickReport {
    pub engine: CompositorBackendTickReport,
    pub renderer: LiveRendererRuntimeObservation,
    pub scanout: LiveScanoutReadinessReport,
    pub kms_scanout_target: LiveKmsScanoutTargetReport,
    pub gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    pub gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    pub page_flip: LivePageFlipEvent,
    pub page_flip_callbacks: LivePageFlipCallbackQueueReport,
    pub runtime_scanout_states: Vec<RuntimeScanoutState>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_cleanup_pending: bool,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_cleanup_retry:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_in_flight_ticks: u64,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_submit:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_retire:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutRetireReport>,
    pub libdrm_poller: LiveLibdrmPollerDiagnostics,
}

use crate::prelude::*;
use crate::render::should_render;
use crate::{
    ChromeActionDecision, CpuFallbackRenderer, EngineError, FrameClock, FramePlanRequest,
    FrameRenderer, HeadlessOutput, LastCommittedLayout, RenderFrameReport, ReplayReport,
    ReplayStep, SessionEvent, SessionLayerSource, SessionTickReport, SessionTickRequest,
    SessionUpdate, SurfaceTransactionCommitReadiness, SurfaceVisualStateTable, WmTransactionUpdate,
    handle_session_event, request_wm_over_stream, validate_chrome_action,
};

pub trait EngineBackend {
    fn output(&self) -> HeadlessOutput;

    fn plan_frame(
        &self,
        request: FramePlanRequest,
        layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError>;

    fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError>;
}

#[derive(Clone, Debug, Default)]
pub struct HeadlessEngine {
    output: HeadlessOutput,
}

impl HeadlessEngine {
    pub fn new(output: HeadlessOutput) -> Self {
        Self { output }
    }

    pub fn output(&self) -> HeadlessOutput {
        self.output
    }

    #[instrument(skip_all, fields(
        output = request.output.raw(),
        frame_serial = request.frame_serial,
        layer_count = layers.len()
    ))]
    pub fn plan_frame(
        &self,
        request: FramePlanRequest,
        mut layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError> {
        self.validate_output(request.output)?;

        layers.sort_by_key(|layer| layer.stack_rank);

        let mut commands = Vec::new();
        let mut damage = Region::empty();
        let mut skipped_layers = 0usize;
        let mut empty_targets = 0usize;

        for layer in &layers {
            if !layer.surface.is_valid() {
                warn!(
                    output = request.output.raw(),
                    frame_serial = request.frame_serial,
                    "rejected frame plan with invalid surface"
                );
                return Err(EngineError::InvalidSurface);
            }

            if !should_render(layer) {
                skipped_layers += 1;
                continue;
            }

            let target = layer.crop.map_or_else(
                || Region::single(layer.geometry),
                |crop| Region::single(crop),
            );

            if target.is_empty() {
                empty_targets += 1;
                continue;
            }

            damage.extend(&layer.damage);
            commands.push(RenderCommand {
                kind: RenderCommandKind::Blit,
                source: Some(layer.surface),
                output: request.output,
                target,
                clip: layer.crop.map(Region::single),
                transform: layer.transform,
                alpha: layer.opacity,
            });
        }
        let rendered_layers = commands.len();
        trace!(
            output = request.output.raw(),
            frame_serial = request.frame_serial,
            layer_count = layers.len(),
            rendered_layers,
            skipped_layers,
            empty_targets,
            "frame planning layer filter summary"
        );
        debug!(
            output = request.output.raw(),
            frame_serial = request.frame_serial,
            layer_count = layers.len(),
            render_commands = commands.len(),
            damage_rects = damage.rects.len(),
            "planned frame"
        );

        Ok(FrameSnapshot {
            output: request.output,
            output_size: self.output.size,
            output_scale: self.output.scale,
            frame_serial: request.frame_serial,
            layers,
            commands,
            damage,
        })
    }

    #[instrument(skip_all, fields(
        output = frame.output.raw(),
        frame_serial = frame.frame_serial,
        command_count = frame.commands.len()
    ))]
    pub fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError> {
        self.validate_output(frame.output)?;

        if frame.output_size != self.output.size || frame.output_scale != self.output.scale {
            warn!(
                output = frame.output.raw(),
                frame_serial = frame.frame_serial,
                "rejected frame replay with mismatched output shape"
            );
            return Err(EngineError::InvalidFrame);
        }

        let surfaces = frame
            .layers
            .iter()
            .map(|layer| layer.surface)
            .collect::<BTreeSet<_>>();
        let mut steps = Vec::with_capacity(frame.commands.len());

        for (command_index, command) in frame.commands.iter().enumerate() {
            if command.output != frame.output {
                warn!(
                    output = frame.output.raw(),
                    frame_serial = frame.frame_serial,
                    command_index,
                    command_output = command.output.raw(),
                    "rejected frame replay with command for different output"
                );
                return Err(EngineError::InvalidOutput);
            }

            if let Some(source) = command.source {
                if !source.is_valid() || !surfaces.contains(&source) {
                    warn!(
                        output = frame.output.raw(),
                        frame_serial = frame.frame_serial,
                        command_index,
                        has_source = command.source.is_some(),
                        "rejected frame replay with invalid command source"
                    );
                    return Err(EngineError::InvalidSurface);
                }
            }

            steps.push(ReplayStep {
                command_index,
                kind: command.kind,
                source: command.source,
                target: command.target.clone(),
                alpha: command.alpha,
            });
        }
        debug!(
            output = frame.output.raw(),
            frame_serial = frame.frame_serial,
            command_count = frame.commands.len(),
            replay_steps = steps.len(),
            damage_rects = frame.damage.rects.len(),
            "replayed frame"
        );

        Ok(ReplayReport {
            output: frame.output,
            output_size: frame.output_size,
            output_scale: frame.output_scale,
            frame_serial: frame.frame_serial,
            steps,
            damage: frame.damage.clone(),
        })
    }

    pub fn render_frame_with(
        &self,
        renderer: &impl FrameRenderer,
        frame: &FrameSnapshot,
    ) -> Result<RenderFrameReport, EngineError> {
        let replay = self.replay_frame(frame)?;
        renderer.render_frame(frame, replay)
    }

    pub fn render_frame(&self, frame: &FrameSnapshot) -> Result<RenderFrameReport, EngineError> {
        self.render_frame_with(&CpuFallbackRenderer, frame)
    }

    #[instrument(skip_all, fields(
        transaction = transaction.transaction.raw(),
        placements = transaction.render_positions.len(),
        layer_count = layers.len()
    ))]
    pub fn apply_layout_transaction(
        &self,
        transaction: &LayoutTransaction,
        mut layers: Vec<LayerSnapshot>,
    ) -> Result<Vec<LayerSnapshot>, EngineError> {
        let layer_indexes = layers
            .iter()
            .enumerate()
            .map(|(index, layer)| (layer.surface, index))
            .collect::<BTreeMap<_, _>>();

        for placement in &transaction.render_positions {
            if !placement.surface.is_valid() {
                warn!(
                    transaction = transaction.transaction.raw(),
                    "rejected layout transaction with invalid placement surface"
                );
                return Err(EngineError::InvalidSurface);
            }
            let Some(index) = layer_indexes.get(&placement.surface).copied() else {
                warn!(
                    transaction = transaction.transaction.raw(),
                    surface_index = placement.surface.index(),
                    surface_generation = placement.surface.generation(),
                    "rejected layout transaction for unknown surface"
                );
                return Err(EngineError::InvalidSurface);
            };
            let layer = &mut layers[index];
            let old_geometry = layer.geometry;

            layer.geometry = placement.geometry;
            layer.stack_rank = u32::try_from(placement.z_index.max(0))
                .expect("non-negative z-index should fit u32");
            layer.crop = placement.crop;
            layer.transform = placement.transform;
            layer.damage = moved_damage(old_geometry, placement.geometry);
            layer.generation = layer.generation.saturating_add(1);
        }

        Ok(layers)
    }

    #[instrument(skip_all, fields(
        transaction = transaction.transaction.raw(),
        placements = transaction.render_positions.len(),
        layer_count = layers.len()
    ))]
    pub fn commit_layout_transaction(
        &self,
        transaction: &LayoutTransaction,
        layers: &mut Vec<LayerSnapshot>,
    ) -> TransactionCommit {
        let applied_surfaces = transaction
            .render_positions
            .iter()
            .map(|placement| placement.surface)
            .collect::<Vec<_>>();

        match self.apply_layout_transaction(transaction, layers.clone()) {
            Ok(committed) => {
                *layers = committed;
                debug!(
                    transaction = transaction.transaction.raw(),
                    applied_surfaces = applied_surfaces.len(),
                    outcome = ?TransactionOutcome::Committed,
                    "committed layout transaction"
                );
                TransactionCommit {
                    transaction: transaction.transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces,
                }
            }
            Err(EngineError::InvalidSurface) => {
                warn!(
                    transaction = transaction.transaction.raw(),
                    outcome = ?TransactionOutcome::RejectedInvalidSurface,
                    "rejected layout transaction"
                );
                TransactionCommit {
                    transaction: transaction.transaction,
                    outcome: TransactionOutcome::RejectedInvalidSurface,
                    applied_surfaces: Vec::new(),
                }
            }
            Err(_) => {
                warn!(
                    transaction = transaction.transaction.raw(),
                    outcome = ?TransactionOutcome::RejectedStaleSurface,
                    "rejected layout transaction"
                );
                TransactionCommit {
                    transaction: transaction.transaction,
                    outcome: TransactionOutcome::RejectedStaleSurface,
                    applied_surfaces: Vec::new(),
                }
            }
        }
    }

    pub fn committed_state_from_layer(&self, layer: &LayerSnapshot) -> CommittedSurfaceState {
        CommittedSurfaceState::from_layer_snapshot(layer)
    }

    pub fn project_committed_surface_state(
        &self,
        committed: &CommittedSurfaceState,
        template: &LayerSnapshot,
    ) -> Result<LayerSnapshot, EngineError> {
        if !committed.surface.is_valid() || !template.surface.is_valid() {
            return Err(EngineError::InvalidSurface);
        }
        if committed.surface != template.surface {
            return Err(EngineError::InvalidSurface);
        }

        let mut layer = template.clone();
        layer.geometry = committed.geometry;
        layer.source = committed.buffer;
        layer.damage = committed.damage.clone();
        layer.generation = committed.committed_generation;
        Ok(layer)
    }

    pub fn project_committed_surface_states(
        &self,
        committed: &[CommittedSurfaceState],
        templates: &[LayerSnapshot],
    ) -> Result<Vec<LayerSnapshot>, EngineError> {
        let templates_by_surface = templates
            .iter()
            .map(|template| (template.surface, template))
            .collect::<BTreeMap<_, _>>();

        committed
            .iter()
            .map(|state| {
                let Some(template) = templates_by_surface.get(&state.surface) else {
                    return Err(EngineError::InvalidSurface);
                };
                self.project_committed_surface_state(state, template)
            })
            .collect()
    }

    #[instrument(skip_all, fields(
        transaction = transaction.raw(),
        transaction_count = transactions.len(),
        committed_count = committed.len()
    ))]
    pub fn commit_surface_transactions(
        &self,
        transaction: TransactionId,
        transactions: &[SurfaceTransaction],
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> TransactionCommit {
        let applied_surfaces = transactions
            .iter()
            .map(|surface_transaction| surface_transaction.surface)
            .collect::<Vec<_>>();

        let visual_state = SurfaceVisualStateTable::from_committed_states(committed.clone());

        for surface_transaction in transactions {
            if surface_transaction.transaction != transaction {
                warn!(
                    expected_transaction = transaction.raw(),
                    actual_transaction = surface_transaction.transaction.raw(),
                    "rejected authority transaction batch with mismatched transaction ID"
                );
                return TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::RejectedStaleSurface,
                    applied_surfaces: Vec::new(),
                };
            }

            match visual_state.transaction_commit_readiness(surface_transaction) {
                SurfaceTransactionCommitReadiness::Ready => {}
                SurfaceTransactionCommitReadiness::NotReady(
                    SurfaceTransactionReadiness::Pending | SurfaceTransactionReadiness::TimedOut,
                ) => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        readiness = ?surface_transaction.readiness,
                        "preserving committed surface state because authority transaction is not ready"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::TimedOut,
                        applied_surfaces: Vec::new(),
                    };
                }
                SurfaceTransactionCommitReadiness::NotReady(
                    SurfaceTransactionReadiness::Failed,
                ) => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        "rejected failed authority transaction"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedStaleSurface,
                        applied_surfaces: Vec::new(),
                    };
                }
                SurfaceTransactionCommitReadiness::InvalidSurface
                | SurfaceTransactionCommitReadiness::EmptyGeometry
                | SurfaceTransactionCommitReadiness::MissingBuffer
                | SurfaceTransactionCommitReadiness::NotReady(SurfaceTransactionReadiness::Ready) =>
                {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        readiness = ?visual_state.transaction_commit_readiness(surface_transaction),
                        "rejected malformed authority transaction"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedInvalidSurface,
                        applied_surfaces: Vec::new(),
                    };
                }
                SurfaceTransactionCommitReadiness::StaleGeneration { current, expected } => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        current_generation = current,
                        previous_committed_generation = expected,
                        "rejected stale authority transaction"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedStaleSurface,
                        applied_surfaces: Vec::new(),
                    };
                }
            }
        }

        let mut next_committed = committed.clone();
        for surface_transaction in transactions {
            let next_state = CommittedSurfaceState {
                surface: surface_transaction.surface,
                committed_generation: surface_transaction
                    .previous_committed_generation
                    .saturating_add(1),
                geometry: surface_transaction.target_geometry,
                buffer: surface_transaction.target_buffer,
                damage: surface_transaction.damage.clone(),
            };

            if let Some(index) = next_committed
                .iter()
                .position(|state| state.surface == surface_transaction.surface)
            {
                next_committed[index] = next_state;
            } else {
                next_committed.push(next_state);
            }
        }

        *committed = next_committed;
        debug!(
            transaction = transaction.raw(),
            applied_surfaces = applied_surfaces.len(),
            outcome = ?TransactionOutcome::Committed,
            "committed authority surface transactions"
        );

        TransactionCommit {
            transaction,
            outcome: TransactionOutcome::Committed,
            applied_surfaces,
        }
    }

    pub fn preserve_layout_on_wm_absent(
        &self,
        transaction: TransactionId,
        _layers: &[LayerSnapshot],
    ) -> TransactionCommit {
        warn!(
            transaction = transaction.raw(),
            outcome = ?TransactionOutcome::TimedOut,
            "preserving layout because WM transaction is absent"
        );
        TransactionCommit {
            transaction,
            outcome: TransactionOutcome::TimedOut,
            applied_surfaces: Vec::new(),
        }
    }

    pub fn request_and_commit_wm_transaction<S>(
        &self,
        stream: &mut S,
        request: &WmRequestPacket,
        layers: &mut Vec<LayerSnapshot>,
    ) -> WmTransactionUpdate
    where
        S: Read + Write,
    {
        debug!(
            transaction = request.transaction.raw(),
            request_kind = wm_request_kind_name(&request.kind),
            node_count = wm_request_node_count(&request.kind),
            layer_count = layers.len(),
            "requesting WM transaction"
        );
        match request_wm_over_stream(stream, request) {
            Ok(response) => {
                debug!(
                    transaction = request.transaction.raw(),
                    response_commands = response.commands.len(),
                    response_timeout_msec = response.timeout_msec,
                    "received WM transaction response"
                );
                let transaction = response.into_layout_transaction();
                WmTransactionUpdate {
                    commit: self.commit_layout_transaction(&transaction, layers),
                    ipc_error: None,
                }
            }
            Err(error) => {
                warn!(
                    transaction = request.transaction.raw(),
                    error = %error,
                    "WM transaction IPC failed; preserving layout"
                );
                WmTransactionUpdate {
                    commit: self.preserve_layout_on_wm_absent(request.transaction, layers),
                    ipc_error: Some(error),
                }
            }
        }
    }

    pub fn request_and_cache_wm_transaction<S>(
        &self,
        stream: &mut S,
        request: &WmRequestPacket,
        layers: &mut Vec<LayerSnapshot>,
        last_committed: &mut LastCommittedLayout,
    ) -> WmTransactionUpdate
    where
        S: Read + Write,
    {
        let update = self.request_and_commit_wm_transaction(stream, request, layers);
        match update.commit.outcome {
            TransactionOutcome::Committed => {
                last_committed.replace(layers);
                debug!(
                    transaction = request.transaction.raw(),
                    cached_layers = last_committed.layers().len(),
                    "updated last committed layout cache"
                );
            }
            TransactionOutcome::TimedOut if !last_committed.is_empty() => {
                last_committed.restore_into(layers);
                warn!(
                    transaction = request.transaction.raw(),
                    restored_layers = layers.len(),
                    "restored last committed layout after WM timeout"
                );
            }
            _ => {
                debug!(
                    transaction = request.transaction.raw(),
                    outcome = ?update.commit.outcome,
                    cached_layers = last_committed.layers().len(),
                    "left last committed layout cache unchanged"
                );
            }
        }
        update
    }

    pub fn validate_chrome_action(
        &self,
        request: &ChromeActionRequest,
        nodes: &[LayoutNodeSnapshot],
    ) -> ChromeActionDecision {
        validate_chrome_action(request, nodes)
    }

    pub fn handle_session_event(
        &self,
        event: SessionEvent,
        nodes: &[LayoutNodeSnapshot],
    ) -> SessionUpdate {
        handle_session_event(event, nodes)
    }

    pub fn run_session_tick(
        &self,
        request: SessionTickRequest,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        let (layers, restored_last_committed) = match request.layers {
            SessionLayerSource::Fresh(layers) => {
                debug!(
                    output = request.output.raw(),
                    frame_serial = request.frame_serial,
                    layer_count = layers.len(),
                    "running session tick from fresh layers"
                );
                last_committed.replace(&layers);
                (layers, false)
            }
            SessionLayerSource::RestoreLastCommitted => {
                let mut layers = Vec::new();
                last_committed.restore_into(&mut layers);
                warn!(
                    output = request.output.raw(),
                    frame_serial = request.frame_serial,
                    restored_layers = layers.len(),
                    "running session tick from last committed layout"
                );
                (layers, true)
            }
        };
        let frame = self.plan_frame(
            FramePlanRequest {
                output: request.output,
                frame_serial: request.frame_serial,
            },
            layers,
        )?;
        let replay = self.replay_frame(&frame)?;
        debug!(
            output = request.output.raw(),
            frame_serial = request.frame_serial,
            restored_last_committed,
            render_commands = frame.commands.len(),
            replay_steps = replay.steps.len(),
            "completed session tick"
        );

        Ok(SessionTickReport {
            frame,
            replay,
            restored_last_committed,
        })
    }

    pub fn run_clocked_session_tick(
        &self,
        clock: &mut impl FrameClock,
        layers: SessionLayerSource,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        let tick = clock.next_frame(self.output.id);
        trace!(
            output = tick.output.raw(),
            frame_serial = tick.frame_serial,
            target_msec = tick.target_msec,
            "frame clock produced session tick"
        );

        self.run_session_tick(
            SessionTickRequest {
                output: tick.output,
                frame_serial: tick.frame_serial,
                layers,
            },
            last_committed,
        )
    }

    fn validate_output(&self, output: OutputId) -> Result<(), EngineError> {
        if output.is_valid() && output == self.output.id {
            Ok(())
        } else {
            warn!(
                output = output.raw(),
                expected_output = self.output.id.raw(),
                "rejected engine operation with invalid output"
            );
            Err(EngineError::InvalidOutput)
        }
    }
}

impl EngineBackend for HeadlessEngine {
    fn output(&self) -> HeadlessOutput {
        HeadlessEngine::output(self)
    }

    fn plan_frame(
        &self,
        request: FramePlanRequest,
        layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError> {
        HeadlessEngine::plan_frame(self, request, layers)
    }

    fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError> {
        HeadlessEngine::replay_frame(self, frame)
    }
}

fn moved_damage(old_geometry: Rect, new_geometry: Rect) -> Region {
    let mut damage = Region::single(old_geometry);
    damage.extend(&Region::single(new_geometry));
    damage
}

fn wm_request_kind_name(kind: &WmRequestKind) -> &'static str {
    match kind {
        WmRequestKind::ManageSurface(_) => "manage_surface",
        WmRequestKind::RelayoutWorkspace(_) => "relayout_workspace",
        WmRequestKind::SurfaceRemoved { .. } => "surface_removed",
    }
}

fn wm_request_node_count(kind: &WmRequestKind) -> usize {
    match kind {
        WmRequestKind::ManageSurface(_) => 1,
        WmRequestKind::RelayoutWorkspace(relayout) => relayout.nodes.len(),
        WmRequestKind::SurfaceRemoved { .. } => 0,
    }
}

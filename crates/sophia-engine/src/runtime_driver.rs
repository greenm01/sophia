use crate::prelude::*;
use crate::{
    EngineError, HeadlessEngine, LastCommittedLayout, MetadataChromeUpdate,
    NotificationChromeUpdate, RenderFrameReport, SessionLayerSource, SessionTickReport,
    SessionTickRequest, SlowClientVisualDecision, WmRuntimeAction, WmTransactionUpdate,
};

pub fn runtime_observation_from_wm_transaction_update(
    update: &WmTransactionUpdate,
) -> SessionRuntimeObservation {
    match update.runtime_action() {
        WmRuntimeAction::KeepRunning => SessionRuntimeObservation::WmLayoutReady,
        WmRuntimeAction::RestartWm { .. } => SessionRuntimeObservation::WmRestartRequested,
    }
}

pub fn runtime_observation_from_authority_transaction_commit(
    commit: &TransactionCommit,
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::AuthorityTransactionObserved {
        outcome: commit.outcome,
        applied_surface_count: u32::try_from(commit.applied_surfaces.len()).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_slow_client_visual_decisions(
    decisions: &[SlowClientVisualDecision],
) -> SessionRuntimeObservation {
    let preserved_count = decisions
        .iter()
        .filter(|decision| matches!(decision, SlowClientVisualDecision::PreserveCommitted { .. }))
        .count();
    let degraded_count = decisions
        .iter()
        .filter(|decision| matches!(decision, SlowClientVisualDecision::DegradeToPending { .. }))
        .count();
    let timeout_count = preserved_count.saturating_add(degraded_count);

    SessionRuntimeObservation::SlowClientVisualDecisionsObserved {
        timeout_count: u32::try_from(timeout_count).unwrap_or(u32::MAX),
        preserved_count: u32::try_from(preserved_count).unwrap_or(u32::MAX),
        degraded_count: u32::try_from(degraded_count).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_session_tick_report(
    report: &SessionTickReport,
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::FrameRendered {
        frame_serial: report.frame.frame_serial,
    }
}

pub fn runtime_observation_from_render_frame_report(
    report: &RenderFrameReport,
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::FrameRendered {
        frame_serial: report.replay.frame_serial,
    }
}

pub fn runtime_observation_from_portal_commands(
    commands: &[PortalCommand],
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::PortalCommandsReady {
        count: u32::try_from(commands.len()).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_notification_chrome_updates<'a>(
    updates: impl IntoIterator<Item = &'a NotificationChromeUpdate>,
) -> SessionRuntimeObservation {
    let count = updates
        .into_iter()
        .filter(|update| {
            matches!(
                update,
                NotificationChromeUpdate::Presented { .. }
                    | NotificationChromeUpdate::Dismissed { .. }
            )
        })
        .count();

    SessionRuntimeObservation::ChromeCommandsReady {
        count: u32::try_from(count).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_metadata_chrome_updates<'a>(
    updates: impl IntoIterator<Item = &'a MetadataChromeUpdate>,
) -> SessionRuntimeObservation {
    let count = updates
        .into_iter()
        .filter(|update| {
            matches!(
                update,
                MetadataChromeUpdate::Upserted { .. } | MetadataChromeUpdate::Removed { .. }
            )
        })
        .count();

    SessionRuntimeObservation::ChromeCommandsReady {
        count: u32::try_from(count).unwrap_or(u32::MAX),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessSessionDriverTick {
    pub output: OutputId,
    pub frame_serial: u64,
    pub x_event_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessSessionDriverReport {
    pub runtime_state: SessionRuntimeState,
    pub runtime_commands: Vec<SessionRuntimeCommand>,
    pub session_tick: Option<SessionTickReport>,
    pub cached_layers: usize,
}

pub trait RuntimeDriverAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError>;

    fn request_wm_layout(&mut self) -> Result<SessionRuntimeObservation, EngineError>;

    fn schedule_frame(
        &mut self,
        frame_serial: u64,
    ) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(SessionRuntimeObservation::FrameScheduled { frame_serial })
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError>;

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError>;

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessRuntimeAdapter {
    pub x_event_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
}

impl HeadlessRuntimeAdapter {
    pub fn new(tick: HeadlessSessionDriverTick) -> Self {
        Self {
            x_event_count: tick.x_event_count,
            layers: tick.layers,
            wm_update: tick.wm_update,
            portal_commands: tick.portal_commands,
            chrome_command_count: tick.chrome_command_count,
        }
    }
}

impl RuntimeDriverAdapter for HeadlessRuntimeAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(SessionRuntimeObservation::XEventsPolled {
            count: self.x_event_count,
        })
    }

    fn request_wm_layout(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self
            .wm_update
            .as_ref()
            .map(runtime_observation_from_wm_transaction_update)
            .unwrap_or(SessionRuntimeObservation::WmLayoutReady))
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        engine.run_session_tick(
            SessionTickRequest {
                output,
                frame_serial,
                layers: SessionLayerSource::Fresh(self.layers.clone()),
            },
            last_committed,
        )
    }

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(runtime_observation_from_portal_commands(
            &self.portal_commands,
        ))
    }

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(SessionRuntimeObservation::ChromeCommandsReady {
            count: self.chrome_command_count,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveXRuntimeAdapter {
    pub pending_event_count: u32,
}

impl LiveXRuntimeAdapter {
    pub fn from_polled_event_count(count: u32) -> Self {
        Self {
            pending_event_count: count,
        }
    }

    pub fn poll_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::XEventsPolled {
            count: self.pending_event_count,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveWmRuntimeAdapter {
    pub update: Option<WmTransactionUpdate>,
}

impl LiveWmRuntimeAdapter {
    pub fn from_transaction_update(update: WmTransactionUpdate) -> Self {
        Self {
            update: Some(update),
        }
    }

    pub fn layout_observation(&self) -> SessionRuntimeObservation {
        self.update
            .as_ref()
            .map(runtime_observation_from_wm_transaction_update)
            .unwrap_or(SessionRuntimeObservation::WmLayoutReady)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveBrokerRuntimeAdapter;

impl LiveBrokerRuntimeAdapter {
    pub fn from_health_packet(packet: &BrokerHealthPacket) -> SessionRuntimeObservation {
        Self::health_observation(packet)
    }

    pub fn health_observation(packet: &BrokerHealthPacket) -> SessionRuntimeObservation {
        SessionRuntimeObservation::BrokerHealthChanged {
            broker: packet.broker,
            state: packet.state,
            generation: packet.generation,
            status_message_len: packet.message.as_deref().map(str::len).unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LivePortalRuntimeAdapter {
    pub commands: Vec<PortalCommand>,
}

impl LivePortalRuntimeAdapter {
    pub fn from_commands(commands: Vec<PortalCommand>) -> Self {
        Self { commands }
    }

    pub fn drain_observation(&self) -> SessionRuntimeObservation {
        runtime_observation_from_portal_commands(&self.commands)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveChromeRuntimeAdapter {
    pub command_count: u32,
}

impl LiveChromeRuntimeAdapter {
    pub fn from_command_count(count: u32) -> Self {
        Self {
            command_count: count,
        }
    }

    pub fn from_notification_updates<'a>(
        updates: impl IntoIterator<Item = &'a NotificationChromeUpdate>,
    ) -> Self {
        let SessionRuntimeObservation::ChromeCommandsReady { count } =
            runtime_observation_from_notification_chrome_updates(updates)
        else {
            unreachable!("notification chrome updates always map to chrome command counts");
        };

        Self::from_command_count(count)
    }

    pub fn from_metadata_updates<'a>(
        updates: impl IntoIterator<Item = &'a MetadataChromeUpdate>,
    ) -> Self {
        let SessionRuntimeObservation::ChromeCommandsReady { count } =
            runtime_observation_from_metadata_chrome_updates(updates)
        else {
            unreachable!("metadata chrome updates always map to chrome command counts");
        };

        Self::from_command_count(count)
    }

    pub fn present_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::ChromeCommandsReady {
            count: self.command_count,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRendererRuntimeAdapter {
    pub layers: Vec<LayerSnapshot>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
}

impl LiveRendererRuntimeAdapter {
    pub fn from_layers(layers: Vec<LayerSnapshot>) -> Self {
        Self {
            layers,
            committed_surfaces: Vec::new(),
        }
    }

    pub fn from_committed_surface_states(
        committed_surfaces: Vec<CommittedSurfaceState>,
        layer_templates: Vec<LayerSnapshot>,
    ) -> Self {
        Self {
            layers: layer_templates,
            committed_surfaces,
        }
    }

    pub fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        let layers = if self.committed_surfaces.is_empty() {
            self.layers.clone()
        } else {
            engine.project_committed_surface_states(&self.committed_surfaces, &self.layers)?
        };

        engine.run_session_tick(
            SessionTickRequest {
                output,
                frame_serial,
                layers: SessionLayerSource::Fresh(layers),
            },
            last_committed,
        )
    }

    pub fn rendered_observation(report: &SessionTickReport) -> SessionRuntimeObservation {
        runtime_observation_from_session_tick_report(report)
    }

    pub fn from_render_frame_report(report: &RenderFrameReport) -> SessionRuntimeObservation {
        runtime_observation_from_render_frame_report(report)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverIntake {
    pub x_event_count: u32,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverAdapter {
    pub x: LiveXRuntimeAdapter,
    pub wm: LiveWmRuntimeAdapter,
    pub portal: LivePortalRuntimeAdapter,
    pub chrome: LiveChromeRuntimeAdapter,
    pub renderer: LiveRendererRuntimeAdapter,
}

impl LiveRuntimeDriverAdapter {
    pub fn from_intake(intake: LiveRuntimeDriverIntake) -> Self {
        Self {
            x: LiveXRuntimeAdapter::from_polled_event_count(intake.x_event_count),
            wm: LiveWmRuntimeAdapter {
                update: intake.wm_update,
            },
            portal: LivePortalRuntimeAdapter::from_commands(intake.portal_commands),
            chrome: LiveChromeRuntimeAdapter::from_command_count(intake.chrome_command_count),
            renderer: LiveRendererRuntimeAdapter::from_committed_surface_states(
                intake.committed_surfaces,
                intake.layers,
            ),
        }
    }
}

impl RuntimeDriverAdapter for LiveRuntimeDriverAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.x.poll_observation())
    }

    fn request_wm_layout(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.wm.layout_observation())
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        self.renderer
            .render_frame(engine, output, frame_serial, last_committed)
    }

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.portal.drain_observation())
    }

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.chrome.present_observation())
    }
}

#[derive(Clone, Debug, Default)]
pub struct HeadlessSessionDriver {
    engine: HeadlessEngine,
    runtime: SessionRuntimeLoop,
    last_committed: LastCommittedLayout,
}

impl HeadlessSessionDriver {
    pub fn new(engine: HeadlessEngine) -> Self {
        Self {
            engine,
            runtime: SessionRuntimeLoop::default(),
            last_committed: LastCommittedLayout::default(),
        }
    }

    pub fn runtime_state(&self) -> &SessionRuntimeState {
        self.runtime.state()
    }

    pub fn last_committed(&self) -> &LastCommittedLayout {
        &self.last_committed
    }

    pub fn run_tick(
        &mut self,
        request: HeadlessSessionDriverTick,
    ) -> Result<HeadlessSessionDriverReport, EngineError> {
        let output = request.output;
        let frame_serial = request.frame_serial;
        let mut adapter = HeadlessRuntimeAdapter::new(request);
        self.run_with_adapter(output, frame_serial, &mut adapter)
    }

    pub fn run_with_adapter(
        &mut self,
        output: OutputId,
        frame_serial: u64,
        adapter: &mut impl RuntimeDriverAdapter,
    ) -> Result<HeadlessSessionDriverReport, EngineError> {
        let mut executor = HeadlessSessionCommandExecutor {
            engine: &self.engine,
            runtime: &mut self.runtime,
            last_committed: &mut self.last_committed,
            output,
            frame_serial,
            adapter,
            runtime_commands: Vec::new(),
            pending_commands: Vec::new(),
            session_tick: None,
        };
        executor.run()
    }
}

struct HeadlessSessionCommandExecutor<'a> {
    engine: &'a HeadlessEngine,
    runtime: &'a mut SessionRuntimeLoop,
    last_committed: &'a mut LastCommittedLayout,
    output: OutputId,
    frame_serial: u64,
    adapter: &'a mut dyn RuntimeDriverAdapter,
    runtime_commands: Vec<SessionRuntimeCommand>,
    pending_commands: Vec<SessionRuntimeCommand>,
    session_tick: Option<SessionTickReport>,
}

impl HeadlessSessionCommandExecutor<'_> {
    fn run(&mut self) -> Result<HeadlessSessionDriverReport, EngineError> {
        self.observe([SessionRuntimeObservation::TickStarted])?;

        while let Some(command) = self.pending_commands.pop() {
            match command {
                SessionRuntimeCommand::None => {}
                SessionRuntimeCommand::PollXEvents => {
                    let observation = self.adapter.poll_x_events()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::RequestWmLayout => {
                    let observation = self.adapter.request_wm_layout()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::ScheduleFrame => {
                    let observation = self.adapter.schedule_frame(self.frame_serial)?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::RenderFrame { frame_serial } => {
                    let report = self.adapter.render_frame(
                        self.engine,
                        self.output,
                        frame_serial,
                        self.last_committed,
                    )?;
                    let observation = runtime_observation_from_session_tick_report(&report);
                    self.session_tick = Some(report);
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::DrainPortalCommands => {
                    let observation = self.adapter.drain_portal_commands()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::PresentChrome => {
                    let observation = self.adapter.present_chrome()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::RestartWindowManager => break,
            }
        }

        Ok(HeadlessSessionDriverReport {
            runtime_state: self.runtime.state().clone(),
            runtime_commands: self.runtime_commands.clone(),
            session_tick: self.session_tick.clone(),
            cached_layers: self.last_committed.layers().len(),
        })
    }

    fn observe(
        &mut self,
        observations: impl IntoIterator<Item = SessionRuntimeObservation>,
    ) -> Result<(), EngineError> {
        let report = self
            .runtime
            .step_observations(observations)
            .map_err(EngineError::RuntimeObservation)?;
        self.runtime_commands
            .extend(report.commands.iter().copied());
        self.pending_commands
            .extend(report.commands.into_iter().rev());
        Ok(())
    }
}

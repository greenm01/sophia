use crate::prelude::*;
use crate::{
    AuthorityTransactionIntake, CpuFallbackRenderer, DeterministicFrameClock, DrmKmsMode,
    DrmKmsOutputDescriptor, DrmKmsOutputRegistry, EngineError, FrameClock, FrameClockTick,
    HeadlessEngine, HeadlessOutput, HeadlessSessionDriver, HeadlessSessionDriverReport,
    ImportCapableRenderer, LibinputEventSource, LibinputPhysicalInputAdapter, LibinputPollReport,
    LiveRuntimeDriverAdapter, LiveRuntimeDriverIntake, NonBlockingInputPoller, QueuedInputPoller,
    RenderFrameReport, RuntimeDriverAdapter, WmTransactionUpdate,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RendererSelection {
    #[default]
    CpuFallback,
    ImportCapable {
        import_xpixmap: bool,
        import_dmabuf: bool,
    },
}

impl RendererSelection {
    pub fn render_frame(
        self,
        engine: &HeadlessEngine,
        frame: &FrameSnapshot,
    ) -> Result<RenderFrameReport, EngineError> {
        match self {
            Self::CpuFallback => engine.render_frame_with(&CpuFallbackRenderer, frame),
            Self::ImportCapable {
                import_xpixmap,
                import_dmabuf,
            } => engine.render_frame_with(
                &ImportCapableRenderer::new(import_xpixmap, import_dmabuf),
                frame,
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompositorBackendTickInput {
    pub x_event_count: u32,
    pub authority_batches: Vec<AuthorityTransactionIntake>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
    pub layer_templates: Vec<LayerSnapshot>,
    pub scanout_submit_state: Option<RuntimeScanoutState>,
    pub scanout_lifecycle_states: Vec<RuntimeScanoutState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositorBackendTickReport {
    pub tick: FrameClockTick,
    pub input_poll: LibinputPollReport,
    pub authority_inbox: AuthorityTransactionInboxReport,
    pub runtime: HeadlessSessionDriverReport,
    pub render: Option<RenderFrameReport>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthorityTransactionInboxReport {
    pub drained: usize,
    pub disconnected: bool,
    pub max_reached: bool,
}

#[derive(Debug)]
pub struct AuthorityTransactionInbox {
    receiver: Receiver<AuthorityTransactionIntake>,
    max_drain_per_tick: usize,
}

impl AuthorityTransactionInbox {
    pub fn new(receiver: Receiver<AuthorityTransactionIntake>, max_drain_per_tick: usize) -> Self {
        Self {
            receiver,
            max_drain_per_tick,
        }
    }

    pub fn drain_ready(
        &self,
        out: &mut Vec<AuthorityTransactionIntake>,
    ) -> AuthorityTransactionInboxReport {
        let mut report = AuthorityTransactionInboxReport::default();

        for _ in 0..self.max_drain_per_tick {
            match self.receiver.try_recv() {
                Ok(batch) => {
                    out.push(batch);
                    report.drained = report.drained.saturating_add(1);
                }
                Err(TryRecvError::Empty) => return report,
                Err(TryRecvError::Disconnected) => {
                    report.disconnected = true;
                    return report;
                }
            }
        }

        report.max_reached = true;
        report
    }
}

#[derive(Debug)]
pub enum CompositorBackendAssemblyError {
    InputPoll(io::Error),
    Engine(EngineError),
}

impl fmt::Display for CompositorBackendAssemblyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputPoll(error) => write!(f, "input poll failed: {error}"),
            Self::Engine(error) => write!(f, "engine backend tick failed: {error}"),
        }
    }
}

impl std::error::Error for CompositorBackendAssemblyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InputPoll(error) => Some(error),
            Self::Engine(error) => Some(error),
        }
    }
}

impl From<EngineError> for CompositorBackendAssemblyError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

pub type QueuedHeadlessCompositorBackendAssembly =
    HeadlessCompositorBackendAssembly<QueuedInputPoller>;

pub struct HeadlessCompositorBackendAssembly<P = QueuedInputPoller> {
    engine: HeadlessEngine,
    driver: HeadlessSessionDriver,
    clock: DeterministicFrameClock,
    outputs: DrmKmsOutputRegistry,
    input: LibinputPhysicalInputAdapter<P>,
    authority_inbox: Option<AuthorityTransactionInbox>,
    renderer: RendererSelection,
    committed_surfaces: Vec<CommittedSurfaceState>,
}

impl HeadlessCompositorBackendAssembly<QueuedInputPoller> {
    pub fn new(output: HeadlessOutput) -> Self {
        let mut outputs = DrmKmsOutputRegistry::new();
        outputs.upsert(output_descriptor_from_headless_output(output));

        Self::from_parts(
            output,
            outputs,
            DeterministicFrameClock::default(),
            LibinputPhysicalInputAdapter::new(
                QueuedInputPoller::default(),
                LibinputEventSource::new(),
            ),
            RendererSelection::default(),
        )
    }
}

impl<P> HeadlessCompositorBackendAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn from_parts(
        output: HeadlessOutput,
        outputs: DrmKmsOutputRegistry,
        clock: DeterministicFrameClock,
        input: LibinputPhysicalInputAdapter<P>,
        renderer: RendererSelection,
    ) -> Self {
        let engine = HeadlessEngine::new(output);
        let driver = HeadlessSessionDriver::new(engine.clone());

        Self {
            engine,
            driver,
            clock,
            outputs,
            input,
            authority_inbox: None,
            renderer,
            committed_surfaces: Vec::new(),
        }
    }

    pub fn with_authority_inbox(mut self, inbox: AuthorityTransactionInbox) -> Self {
        self.authority_inbox = Some(inbox);
        self
    }

    pub fn with_committed_surfaces(
        mut self,
        committed_surfaces: Vec<CommittedSurfaceState>,
    ) -> Self {
        self.committed_surfaces = committed_surfaces;
        self
    }

    pub fn engine(&self) -> &HeadlessEngine {
        &self.engine
    }

    pub fn driver(&self) -> &HeadlessSessionDriver {
        &self.driver
    }

    pub fn outputs(&self) -> &DrmKmsOutputRegistry {
        &self.outputs
    }

    pub fn input(&self) -> &LibinputPhysicalInputAdapter<P> {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut LibinputPhysicalInputAdapter<P> {
        &mut self.input
    }

    pub fn renderer(&self) -> RendererSelection {
        self.renderer
    }

    pub fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        &self.committed_surfaces
    }

    pub fn run_tick(
        &mut self,
        input: CompositorBackendTickInput,
    ) -> Result<CompositorBackendTickReport, CompositorBackendAssemblyError> {
        self.run_tick_with_live_runtime_adapter(
            input,
            LiveRuntimeDriverAdapter::from_authority_batches,
            |adapter| adapter.renderer.committed_surfaces.clone(),
        )
    }

    pub fn run_tick_with_live_runtime_adapter<A>(
        &mut self,
        input: CompositorBackendTickInput,
        build_adapter: impl FnOnce(&HeadlessEngine, LiveRuntimeDriverIntake) -> A,
        committed_surfaces: impl Fn(&A) -> Vec<CommittedSurfaceState>,
    ) -> Result<CompositorBackendTickReport, CompositorBackendAssemblyError>
    where
        A: RuntimeDriverAdapter,
    {
        let input_poll = self
            .input
            .poll_once()
            .map_err(CompositorBackendAssemblyError::InputPoll)?;
        let tick = self.clock.next_frame(self.engine.output().id);
        let mut authority_batches = input.authority_batches;
        let authority_inbox = self
            .authority_inbox
            .as_ref()
            .map(|inbox| inbox.drain_ready(&mut authority_batches))
            .unwrap_or_default();
        let mut adapter = build_adapter(
            &self.engine,
            LiveRuntimeDriverIntake {
                x_event_count: input.x_event_count,
                authority_commits: Vec::new(),
                authority_batches,
                wm_update: input.wm_update,
                portal_commands: input.portal_commands,
                chrome_command_count: input.chrome_command_count,
                layers: input.layer_templates,
                committed_surfaces: self.committed_surfaces.clone(),
                scanout_submit_state: input.scanout_submit_state,
                scanout_lifecycle_states: input.scanout_lifecycle_states,
            },
        );

        self.committed_surfaces = committed_surfaces(&adapter);
        let runtime = self
            .driver
            .run_with_adapter(tick.output, tick.frame_serial, &mut adapter)?;
        self.committed_surfaces = committed_surfaces(&adapter);
        let render = runtime
            .session_tick
            .as_ref()
            .map(|session_tick| {
                self.renderer
                    .render_frame(&self.engine, &session_tick.frame)
            })
            .transpose()?;

        debug!(
            output = tick.output.raw(),
            frame_serial = tick.frame_serial,
            input_polled = input_poll.polled,
            input_accepted = input_poll.accepted,
            input_rejected = input_poll.rejected.len(),
            authority_batches_drained = authority_inbox.drained,
            authority_inbox_disconnected = authority_inbox.disconnected,
            committed_surfaces = self.committed_surfaces.len(),
            rendered = render.is_some(),
            "ran deterministic compositor backend tick"
        );

        Ok(CompositorBackendTickReport {
            tick,
            input_poll,
            authority_inbox,
            runtime,
            render,
        })
    }
}

fn output_descriptor_from_headless_output(output: HeadlessOutput) -> DrmKmsOutputDescriptor {
    DrmKmsOutputDescriptor {
        output: output.id,
        connector_id: u32::try_from(output.id.raw()).unwrap_or(u32::MAX),
        crtc_id: 0,
        mode: DrmKmsMode {
            size: output.size,
            refresh_millihz: 60_000,
        },
        scale: output.scale,
    }
}

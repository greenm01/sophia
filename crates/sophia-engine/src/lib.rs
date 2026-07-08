use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

use sophia_protocol::{
    BufferSource, ChromeActionKind, ChromeActionRequest, ChromeDescriptor, FrameSnapshot,
    InputEventKind, InputEventPacket, InputRoute, InputRouteOutcome, IpcCodecError, LayerSnapshot,
    LayoutNodeSnapshot, LayoutTransaction, OutputId, Rect, Region, RenderCommand,
    RenderCommandKind, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, Size, SurfaceId,
    TransactionCommit, TransactionId, TransactionOutcome, WmRequestKind, WmRequestPacket,
    WmResponsePacket, WorkspaceId, XWindowId, decode_wm_response_frame, encode_wm_request_frame,
};
use sophia_runtime::{
    RestartPolicy, SophiaErrorExt, SophiaErrorKind, SupervisedProcessKind, SupervisorCommand,
    SupervisorEvent, SupervisorState, update_supervisor,
};
use tracing::instrument;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    InvalidOutput,
    InvalidSurface,
    InvalidFrame,
    WmIpc(WmIpcError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput => f.write_str("invalid output ID"),
            Self::InvalidSurface => f.write_str("invalid surface ID"),
            Self::InvalidFrame => f.write_str("invalid frame snapshot"),
            Self::WmIpc(error) => write!(f, "WM IPC failed: {error}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl SophiaErrorExt for EngineError {
    fn kind(&self) -> SophiaErrorKind {
        match self {
            Self::InvalidOutput => SophiaErrorKind::InvalidOutput,
            Self::InvalidSurface => SophiaErrorKind::InvalidSurface,
            Self::InvalidFrame => SophiaErrorKind::InvalidFrame,
            Self::WmIpc(_) => SophiaErrorKind::ExternalProcess,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmIpcError {
    Codec(IpcCodecError),
    Io(String),
    TransactionMismatch {
        expected: TransactionId,
        actual: TransactionId,
    },
}

impl fmt::Display for WmIpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "codec error: {error:?}"),
            Self::Io(error) => f.write_str(error),
            Self::TransactionMismatch { expected, actual } => write!(
                f,
                "transaction mismatch, expected {}, got {}",
                expected.raw(),
                actual.raw()
            ),
        }
    }
}

impl std::error::Error for WmIpcError {}

#[derive(Clone, Debug, PartialEq)]
pub struct WmTransactionUpdate {
    pub commit: TransactionCommit,
    pub ipc_error: Option<WmIpcError>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRuntimeAction {
    KeepRunning,
    RestartWm { reason: WmRestartReason },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRestartReason {
    IpcFailure(WmIpcError),
}

impl WmTransactionUpdate {
    pub fn runtime_action(&self) -> WmRuntimeAction {
        match &self.ipc_error {
            Some(error) => WmRuntimeAction::RestartWm {
                reason: WmRestartReason::IpcFailure(error.clone()),
            },
            None => WmRuntimeAction::KeepRunning,
        }
    }
}

pub fn update_wm_supervisor_from_runtime_action(
    state: SupervisorState,
    action: WmRuntimeAction,
    policy: RestartPolicy,
) -> (SupervisorState, SupervisorCommand) {
    debug_assert_eq!(state.process, SupervisedProcessKind::WindowManager);

    match action {
        WmRuntimeAction::KeepRunning => (state, SupervisorCommand::None),
        WmRuntimeAction::RestartWm { .. } => {
            update_supervisor(state, SupervisorEvent::RestartRequested, policy)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueuedRoutedInput {
    pub event: InputEventPacket,
    pub route: InputRoute,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutedInputFlush {
    pub reason: RoutedInputFlushReason,
    pub inputs: Vec<QueuedRoutedInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputFlushReason {
    FrameBoundary,
    StateChangingInput,
    TargetCrossing,
    DragStateChanged,
    GrabChanged,
    FocusChanged,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoutedInputQueueAction {
    BufferedMotion,
    Flushed(RoutedInputFlush),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoutedInputCoalescer {
    pending_motion: Option<QueuedRoutedInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RoutedInputRouteKey {
    target_surface: Option<SurfaceId>,
    target_window: XWindowId,
}

impl RoutedInputCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: InputEventPacket, route: InputRoute) -> RoutedInputQueueAction {
        let input = QueuedRoutedInput { event, route };

        if let Some(key) = coalescible_motion_key(&input) {
            return self.push_motion(input, key);
        }

        self.flush_with(RoutedInputFlushReason::StateChangingInput, Some(input))
    }

    pub fn flush_frame(&mut self) -> Option<RoutedInputFlush> {
        self.take_pending(RoutedInputFlushReason::FrameBoundary)
    }

    pub fn flush_barrier(&mut self, reason: RoutedInputFlushReason) -> Option<RoutedInputFlush> {
        self.take_pending(reason)
    }

    pub fn has_pending_motion(&self) -> bool {
        self.pending_motion.is_some()
    }

    fn push_motion(
        &mut self,
        input: QueuedRoutedInput,
        key: RoutedInputRouteKey,
    ) -> RoutedInputQueueAction {
        match self
            .pending_motion
            .as_ref()
            .and_then(coalescible_motion_key)
        {
            Some(pending_key) if pending_key == key => {
                self.pending_motion = Some(input);
                RoutedInputQueueAction::BufferedMotion
            }
            Some(_) => self.flush_with(RoutedInputFlushReason::TargetCrossing, Some(input)),
            None => {
                self.pending_motion = Some(input);
                RoutedInputQueueAction::BufferedMotion
            }
        }
    }

    fn flush_with(
        &mut self,
        reason: RoutedInputFlushReason,
        current: Option<QueuedRoutedInput>,
    ) -> RoutedInputQueueAction {
        let mut inputs = Vec::new();
        if let Some(pending) = self.pending_motion.take() {
            inputs.push(pending);
        }
        if let Some(current) = current {
            inputs.push(current);
        }

        RoutedInputQueueAction::Flushed(RoutedInputFlush { reason, inputs })
    }

    fn take_pending(&mut self, reason: RoutedInputFlushReason) -> Option<RoutedInputFlush> {
        self.pending_motion.take().map(|pending| RoutedInputFlush {
            reason,
            inputs: vec![pending],
        })
    }
}

fn coalescible_motion_key(input: &QueuedRoutedInput) -> Option<RoutedInputRouteKey> {
    if input.event.kind != InputEventKind::PointerMotion {
        return None;
    }
    if input.route.outcome != InputRouteOutcome::Routed {
        return None;
    }

    let target_window = input
        .route
        .target_window
        .filter(|window| window.is_valid())?;

    Some(RoutedInputRouteKey {
        target_surface: input.route.target_surface,
        target_window,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmSocketTransportConfig {
    pub response_timeout: Duration,
}

impl Default for WmSocketTransportConfig {
    fn default() -> Self {
        Self {
            response_timeout: Duration::from_millis(250),
        }
    }
}

#[cfg(unix)]
pub struct WmSocketTransport {
    stream: UnixStream,
    config: WmSocketTransportConfig,
}

#[cfg(unix)]
impl WmSocketTransport {
    pub fn new(stream: UnixStream, config: WmSocketTransportConfig) -> Self {
        Self { stream, config }
    }

    pub fn request(&mut self, request: &WmRequestPacket) -> Result<WmResponsePacket, WmIpcError> {
        self.stream
            .set_read_timeout(Some(self.config.response_timeout))
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        request_wm_over_stream(&mut self.stream, request)
    }
}

pub fn request_wm_over_stream<S>(
    stream: &mut S,
    request: &WmRequestPacket,
) -> Result<WmResponsePacket, WmIpcError>
where
    S: Read + Write,
{
    let frame = encode_wm_request_frame(request).map_err(WmIpcError::Codec)?;
    stream
        .write_all(&frame)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    let response = read_wm_response_frame(stream)?;
    if response.transaction != request.transaction {
        return Err(WmIpcError::TransactionMismatch {
            expected: request.transaction,
            actual: response.transaction,
        });
    }

    Ok(response)
}

pub fn read_wm_response_frame<R>(reader: &mut R) -> Result<WmResponsePacket, WmIpcError>
where
    R: Read,
{
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(WmIpcError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    reader
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    decode_wm_response_frame(&frame).map_err(WmIpcError::Codec)
}

#[derive(Clone, Copy, Debug)]
pub struct FramePlanRequest {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadlessOutput {
    pub id: OutputId,
    pub size: Size,
    pub scale: u32,
}

impl HeadlessOutput {
    pub const fn deterministic() -> Self {
        Self {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1280,
                height: 720,
            },
            scale: 1,
        }
    }
}

impl Default for HeadlessOutput {
    fn default() -> Self {
        Self::deterministic()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayStep {
    pub command_index: usize,
    pub kind: RenderCommandKind,
    pub source: Option<SurfaceId>,
    pub target: Region,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayReport {
    pub output: OutputId,
    pub output_size: Size,
    pub output_scale: u32,
    pub frame_serial: u64,
    pub steps: Vec<ReplayStep>,
    pub damage: Region,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromeActionDecision {
    RequestPoliteClose { surface: SurfaceId },
    Rejected(ChromeActionRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeActionRejectReason {
    UnknownSurface,
    StaleGeneration,
    NotClosable,
    UnsupportedAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    ChromeAction(ChromeActionRequest),
    SurfaceRemoved {
        transaction: TransactionId,
        surface: SurfaceId,
        workspace: WorkspaceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionUpdate {
    pub chrome_decision: Option<ChromeActionDecision>,
    pub commands: Vec<SessionCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionCommand {
    RequestPoliteClose { surface: SurfaceId },
    SendWmRequest(WmRequestPacket),
}

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

#[derive(Clone, Debug, Default)]
pub struct ChromeBroker {
    descriptors: BTreeMap<SurfaceId, ChromeDescriptor>,
}

impl ChromeBroker {
    pub fn upsert(&mut self, descriptor: ChromeDescriptor) {
        self.descriptors.insert(descriptor.surface, descriptor);
    }

    pub fn get(&self, surface: SurfaceId) -> Option<&ChromeDescriptor> {
        self.descriptors.get(&surface)
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> Option<ChromeDescriptor> {
        self.descriptors.remove(&surface)
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
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

        for layer in &layers {
            if !layer.surface.is_valid() {
                return Err(EngineError::InvalidSurface);
            }

            if !should_render(layer) {
                continue;
            }

            let target = layer.crop.map_or_else(
                || Region::single(layer.geometry),
                |crop| Region::single(crop),
            );

            if target.is_empty() {
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
                return Err(EngineError::InvalidOutput);
            }

            if let Some(source) = command.source {
                if !source.is_valid() || !surfaces.contains(&source) {
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

        Ok(ReplayReport {
            output: frame.output,
            output_size: frame.output_size,
            output_scale: frame.output_scale,
            frame_serial: frame.frame_serial,
            steps,
            damage: frame.damage.clone(),
        })
    }

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
                return Err(EngineError::InvalidSurface);
            }
            let Some(index) = layer_indexes.get(&placement.surface).copied() else {
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
                TransactionCommit {
                    transaction: transaction.transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces,
                }
            }
            Err(EngineError::InvalidSurface) => TransactionCommit {
                transaction: transaction.transaction,
                outcome: TransactionOutcome::RejectedInvalidSurface,
                applied_surfaces: Vec::new(),
            },
            Err(_) => TransactionCommit {
                transaction: transaction.transaction,
                outcome: TransactionOutcome::RejectedStaleSurface,
                applied_surfaces: Vec::new(),
            },
        }
    }

    pub fn preserve_layout_on_wm_absent(
        &self,
        transaction: TransactionId,
        _layers: &[LayerSnapshot],
    ) -> TransactionCommit {
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
        match request_wm_over_stream(stream, request) {
            Ok(response) => {
                let transaction = response.into_layout_transaction();
                WmTransactionUpdate {
                    commit: self.commit_layout_transaction(&transaction, layers),
                    ipc_error: None,
                }
            }
            Err(error) => WmTransactionUpdate {
                commit: self.preserve_layout_on_wm_absent(request.transaction, layers),
                ipc_error: Some(error),
            },
        }
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

    fn validate_output(&self, output: OutputId) -> Result<(), EngineError> {
        if output.is_valid() && output == self.output.id {
            Ok(())
        } else {
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

fn should_render(layer: &LayerSnapshot) -> bool {
    layer.opacity > 0.0 && !layer.geometry.is_empty() && layer.source != BufferSource::None
}

fn moved_damage(old_geometry: Rect, new_geometry: Rect) -> Region {
    let mut damage = Region::single(old_geometry);
    damage.extend(&Region::single(new_geometry));
    damage
}

pub fn validate_chrome_action(
    request: &ChromeActionRequest,
    nodes: &[LayoutNodeSnapshot],
) -> ChromeActionDecision {
    let Some(node) = nodes.iter().find(|node| node.surface == request.surface) else {
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::UnknownSurface);
    };

    if node.generation != request.generation {
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::StaleGeneration);
    }

    match request.kind {
        ChromeActionKind::CloseSurfaceRequested => {
            if node.capabilities.closable {
                ChromeActionDecision::RequestPoliteClose {
                    surface: request.surface,
                }
            } else {
                ChromeActionDecision::Rejected(ChromeActionRejectReason::NotClosable)
            }
        }
    }
}

pub fn handle_session_event(event: SessionEvent, nodes: &[LayoutNodeSnapshot]) -> SessionUpdate {
    match event {
        SessionEvent::ChromeAction(request) => {
            let decision = validate_chrome_action(&request, nodes);
            let commands = match decision {
                ChromeActionDecision::RequestPoliteClose { surface } => {
                    vec![SessionCommand::RequestPoliteClose { surface }]
                }
                ChromeActionDecision::Rejected(_) => Vec::new(),
            };

            SessionUpdate {
                chrome_decision: Some(decision),
                commands,
            }
        }
        SessionEvent::SurfaceRemoved {
            transaction,
            surface,
            workspace,
        } => SessionUpdate {
            chrome_decision: None,
            commands: vec![SessionCommand::SendWmRequest(WmRequestPacket {
                transaction,
                kind: WmRequestKind::SurfaceRemoved { surface, workspace },
            })],
        },
    }
}

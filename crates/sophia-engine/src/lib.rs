use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

use sophia_portal::{
    MAX_NOTIFICATION_ACTION_LEN, MAX_NOTIFICATION_ACTIONS, MAX_NOTIFICATION_BODY_LEN,
    MAX_NOTIFICATION_SUMMARY_LEN, NotificationRequest, NotificationUrgency, PortalCommand,
};
use sophia_protocol::{
    AttentionState, BufferSource, ChromeActionKind, ChromeActionRequest, ChromeDescriptor,
    DisplayLabel, FrameSnapshot, IconTokenId, InputEventKind, InputEventPacket, InputRoute,
    InputRouteOutcome, IpcCodecError, LayerSnapshot, LayoutNodeSnapshot, LayoutTransaction,
    OutputId, PortalTransferId, Rect, Region, RenderCommand, RenderCommandKind,
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, Size, SurfaceId, TransactionCommit,
    TransactionId, TransactionOutcome, TrustLevel, WmRequestKind, WmRequestPacket,
    WmResponsePacket, WorkspaceId, XWindowId, decode_wm_response_frame, encode_wm_request_frame,
};
use sophia_runtime::{
    RestartPolicy, SophiaErrorExt, SophiaErrorKind, SupervisedProcessKind, SupervisorCommand,
    SupervisorEvent, SupervisorState, update_supervisor,
};
use tracing::{debug, instrument, trace, warn};

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LastCommittedLayout {
    layers: Vec<LayerSnapshot>,
}

impl LastCommittedLayout {
    pub fn new(layers: Vec<LayerSnapshot>) -> Self {
        Self { layers }
    }

    pub fn layers(&self) -> &[LayerSnapshot] {
        &self.layers
    }

    pub fn replace(&mut self, layers: &[LayerSnapshot]) {
        self.layers.clear();
        self.layers.extend_from_slice(layers);
    }

    pub fn restore_into(&self, layers: &mut Vec<LayerSnapshot>) {
        layers.clear();
        layers.extend_from_slice(&self.layers);
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
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
        WmRuntimeAction::KeepRunning => {
            debug!(
                process = ?state.process,
                running = state.running,
                restart_attempts = state.restart_attempts,
                "WM runtime action keeps supervisor state"
            );
            (state, SupervisorCommand::None)
        }
        WmRuntimeAction::RestartWm { .. } => {
            warn!(
                process = ?state.process,
                running = state.running,
                restart_attempts = state.restart_attempts,
                "WM runtime action requests supervisor restart"
            );
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
    debug!(
        transaction = request.transaction.raw(),
        request_bytes = frame.len(),
        "sending WM request frame"
    );
    stream
        .write_all(&frame)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    let response = read_wm_response_frame(stream)?;
    if response.transaction != request.transaction {
        warn!(
            expected_transaction = request.transaction.raw(),
            actual_transaction = response.transaction.raw(),
            "rejected WM response with mismatched transaction"
        );
        return Err(WmIpcError::TransactionMismatch {
            expected: request.transaction,
            actual: response.transaction,
        });
    }
    debug!(
        transaction = response.transaction.raw(),
        response_commands = response.commands.len(),
        "received WM response frame"
    );

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
        warn!(
            payload_len,
            max_payload_len = SOPHIA_IPC_MAX_PAYLOAD_LEN,
            "rejected oversized WM response frame"
        );
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
pub struct FrameClockTick {
    pub output: OutputId,
    pub frame_serial: u64,
    pub target_msec: u64,
}

pub trait FrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicFrameClock {
    next_serial: u64,
    frame_interval_msec: u64,
}

impl DeterministicFrameClock {
    pub const fn new(start_serial: u64, frame_interval_msec: u64) -> Self {
        Self {
            next_serial: start_serial,
            frame_interval_msec,
        }
    }

    pub const fn next_serial(&self) -> u64 {
        self.next_serial
    }

    pub const fn frame_interval_msec(&self) -> u64 {
        self.frame_interval_msec
    }
}

impl Default for DeterministicFrameClock {
    fn default() -> Self {
        Self::new(1, 16)
    }
}

impl FrameClock for DeterministicFrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick {
        let frame_serial = self.next_serial;
        self.next_serial = self.next_serial.saturating_add(1);

        FrameClockTick {
            output,
            frame_serial,
            target_msec: frame_serial.saturating_mul(self.frame_interval_msec),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmKmsMode {
    pub size: Size,
    pub refresh_millihz: u32,
}

impl DrmKmsMode {
    pub const fn new(width: i32, height: i32, refresh_millihz: u32) -> Self {
        Self {
            size: Size { width, height },
            refresh_millihz,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrmKmsOutputDescriptor {
    pub output: OutputId,
    pub connector_id: u32,
    pub crtc_id: u32,
    pub mode: DrmKmsMode,
    pub scale: u32,
}

impl DrmKmsOutputDescriptor {
    pub const fn as_engine_output(self) -> HeadlessOutput {
        HeadlessOutput {
            id: self.output,
            size: self.mode.size,
            scale: self.scale,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DrmKmsOutputRegistry {
    outputs: BTreeMap<OutputId, DrmKmsOutputDescriptor>,
}

impl DrmKmsOutputRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, output: DrmKmsOutputDescriptor) {
        self.outputs.insert(output.output, output);
    }

    pub fn remove(&mut self, output: OutputId) -> Option<DrmKmsOutputDescriptor> {
        self.outputs.remove(&output)
    }

    pub fn get(&self, output: OutputId) -> Option<&DrmKmsOutputDescriptor> {
        self.outputs.get(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = &DrmKmsOutputDescriptor> {
        self.outputs.values()
    }

    pub fn primary_engine_output(&self) -> Option<HeadlessOutput> {
        self.outputs
            .values()
            .next()
            .map(|output| output.as_engine_output())
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferImportPath {
    CpuReadback,
    XPixmap,
    DmaBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferImportReport {
    pub surface: SurfaceId,
    pub source: BufferSource,
    pub requested: BufferImportPath,
    pub used: BufferImportPath,
    pub used_fallback: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderFrameReport {
    pub replay: ReplayReport,
    pub imports: Vec<BufferImportReport>,
}

pub trait FrameRenderer {
    fn render_frame(
        &self,
        frame: &FrameSnapshot,
        replay: ReplayReport,
    ) -> Result<RenderFrameReport, EngineError>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuFallbackRenderer;

impl FrameRenderer for CpuFallbackRenderer {
    fn render_frame(
        &self,
        frame: &FrameSnapshot,
        replay: ReplayReport,
    ) -> Result<RenderFrameReport, EngineError> {
        let imports = collect_buffer_imports(frame);
        trace!(
            output = frame.output.raw(),
            frame_serial = frame.frame_serial,
            import_count = imports.len(),
            "rendered frame with CPU fallback renderer"
        );

        Ok(RenderFrameReport { replay, imports })
    }
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

#[derive(Clone, Debug, PartialEq)]
pub enum SessionLayerSource {
    Fresh(Vec<LayerSnapshot>),
    RestoreLastCommitted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionTickRequest {
    pub output: OutputId,
    pub frame_serial: u64,
    pub layers: SessionLayerSource,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionTickReport {
    pub frame: FrameSnapshot,
    pub replay: ReplayReport,
    pub restored_last_committed: bool,
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

pub const MAX_CHROME_LABEL_LEN: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanitizedChromeMetadata {
    pub surface: SurfaceId,
    pub label: Option<String>,
    pub label_redacted: bool,
    pub icon: Option<IconTokenId>,
    pub trust_level: TrustLevel,
    pub attention: AttentionState,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataChromeUpdate {
    Upserted { surface: SurfaceId },
    Removed { surface: SurfaceId },
    Rejected(MetadataChromeRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataChromeRejectReason {
    InvalidSurface,
    InvalidLabel,
    StaleGeneration,
}

impl ChromeBroker {
    pub fn upsert(&mut self, descriptor: ChromeDescriptor) {
        debug!(
            surface_index = descriptor.surface.index(),
            surface_generation = descriptor.surface.generation(),
            descriptor_generation = descriptor.generation,
            has_label = descriptor.label.is_some(),
            has_icon = descriptor.icon.is_some(),
            trust_level = ?descriptor.trust_level,
            attention = ?descriptor.attention,
            "upserting chrome descriptor"
        );
        self.descriptors.insert(descriptor.surface, descriptor);
    }

    pub fn apply_metadata(&mut self, metadata: SanitizedChromeMetadata) -> MetadataChromeUpdate {
        let surface = metadata.surface;
        let generation = metadata.generation;
        let Ok(descriptor) = chrome_descriptor_from_metadata(metadata) else {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected sanitized chrome metadata with invalid label"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel);
        };

        if !descriptor.surface.is_valid() {
            warn!(
                surface_index = descriptor.surface.index(),
                surface_generation = descriptor.surface.generation(),
                metadata_generation = descriptor.generation,
                "rejected sanitized chrome metadata with invalid surface"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidSurface);
        }

        if self
            .get(descriptor.surface)
            .is_some_and(|existing| existing.generation > descriptor.generation)
        {
            warn!(
                surface_index = descriptor.surface.index(),
                surface_generation = descriptor.surface.generation(),
                metadata_generation = descriptor.generation,
                "rejected stale sanitized chrome metadata"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration);
        }

        let surface = descriptor.surface;
        self.upsert(descriptor);
        MetadataChromeUpdate::Upserted { surface }
    }

    pub fn remove_metadata(&mut self, surface: SurfaceId, generation: u64) -> MetadataChromeUpdate {
        if !surface.is_valid() {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected chrome descriptor removal with invalid surface"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidSurface);
        }

        if self
            .get(surface)
            .is_some_and(|existing| existing.generation > generation)
        {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected stale chrome descriptor removal"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration);
        }

        self.remove_surface(surface);
        debug!(
            surface_index = surface.index(),
            surface_generation = surface.generation(),
            metadata_generation = generation,
            "removed chrome descriptor metadata"
        );
        MetadataChromeUpdate::Removed { surface }
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

pub const MAX_CHROME_NOTIFICATIONS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeNotification {
    pub transfer: PortalTransferId,
    pub summary: String,
    pub body: Option<String>,
    pub urgency: NotificationUrgency,
    pub actions: Vec<String>,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationChromeCommand {
    Present { transfer: PortalTransferId },
    Dismiss { transfer: PortalTransferId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationChromeUpdate {
    Staged { transfer: PortalTransferId },
    Presented { transfer: PortalTransferId },
    Dismissed { transfer: PortalTransferId },
    Ignored,
    Rejected(NotificationChromeRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationChromeRejectReason {
    InvalidTransfer,
    InvalidText,
    TooManyActions,
    TooManyVisibleNotifications,
    UnknownTransfer,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationChromePresenter {
    pending: BTreeMap<PortalTransferId, ChromeNotification>,
    visible: BTreeMap<PortalTransferId, ChromeNotification>,
}

impl NotificationChromePresenter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stage_request(&mut self, request: &NotificationRequest) -> NotificationChromeUpdate {
        if !request.transfer.is_valid() {
            warn!(
                transfer = request.transfer.raw(),
                "rejected notification chrome request with invalid transfer"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::InvalidTransfer,
            );
        }

        if !valid_notification_chrome_text(&request.summary, MAX_NOTIFICATION_SUMMARY_LEN)
            || request.body.as_ref().is_some_and(|body| {
                !valid_notification_chrome_text(body, MAX_NOTIFICATION_BODY_LEN)
            })
            || request
                .actions
                .iter()
                .any(|action| !valid_notification_chrome_text(action, MAX_NOTIFICATION_ACTION_LEN))
        {
            warn!(
                transfer = request.transfer.raw(),
                generation = request.generation,
                action_count = request.actions.len(),
                "rejected notification chrome request with invalid text"
            );
            return NotificationChromeUpdate::Rejected(NotificationChromeRejectReason::InvalidText);
        }

        if request.actions.len() > MAX_NOTIFICATION_ACTIONS {
            warn!(
                transfer = request.transfer.raw(),
                generation = request.generation,
                action_count = request.actions.len(),
                "rejected notification chrome request with too many actions"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::TooManyActions,
            );
        }

        let notification = ChromeNotification {
            transfer: request.transfer,
            summary: request.summary.clone(),
            body: request.body.clone(),
            urgency: request.urgency,
            actions: request.actions.clone(),
            generation: request.generation,
        };

        self.pending.insert(request.transfer, notification);
        debug!(
            transfer = request.transfer.raw(),
            generation = request.generation,
            urgency = ?request.urgency,
            action_count = request.actions.len(),
            pending_count = self.pending.len(),
            "staged notification chrome request"
        );
        NotificationChromeUpdate::Staged {
            transfer: request.transfer,
        }
    }

    pub fn apply_portal_command(&mut self, command: &PortalCommand) -> NotificationChromeUpdate {
        let Some(command) = notification_chrome_command_from_portal(command) else {
            return NotificationChromeUpdate::Ignored;
        };

        self.apply_command(command)
    }

    pub fn apply_command(
        &mut self,
        command: NotificationChromeCommand,
    ) -> NotificationChromeUpdate {
        match command {
            NotificationChromeCommand::Present { transfer } => self.present(transfer),
            NotificationChromeCommand::Dismiss { transfer } => self.dismiss(transfer),
        }
    }

    pub fn pending(&self, transfer: PortalTransferId) -> Option<&ChromeNotification> {
        self.pending.get(&transfer)
    }

    pub fn visible(&self, transfer: PortalTransferId) -> Option<&ChromeNotification> {
        self.visible.get(&transfer)
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    fn present(&mut self, transfer: PortalTransferId) -> NotificationChromeUpdate {
        let Some(notification) = self.pending.remove(&transfer) else {
            warn!(
                transfer = transfer.raw(),
                "rejected notification chrome present for unknown transfer"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::UnknownTransfer,
            );
        };

        if self.visible.len() >= MAX_CHROME_NOTIFICATIONS && !self.visible.contains_key(&transfer) {
            self.pending.insert(transfer, notification);
            warn!(
                transfer = transfer.raw(),
                visible_count = self.visible.len(),
                max_visible = MAX_CHROME_NOTIFICATIONS,
                "rejected notification chrome present because visible set is full"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::TooManyVisibleNotifications,
            );
        }

        self.visible.insert(transfer, notification);
        debug!(
            transfer = transfer.raw(),
            pending_count = self.pending.len(),
            visible_count = self.visible.len(),
            "presented notification chrome"
        );
        NotificationChromeUpdate::Presented { transfer }
    }

    fn dismiss(&mut self, transfer: PortalTransferId) -> NotificationChromeUpdate {
        let removed_pending = self.pending.remove(&transfer).is_some();
        let removed_visible = self.visible.remove(&transfer).is_some();

        if removed_pending || removed_visible {
            debug!(
                transfer = transfer.raw(),
                removed_pending,
                removed_visible,
                pending_count = self.pending.len(),
                visible_count = self.visible.len(),
                "dismissed notification chrome"
            );
            NotificationChromeUpdate::Dismissed { transfer }
        } else {
            warn!(
                transfer = transfer.raw(),
                "rejected notification chrome dismiss for unknown transfer"
            );
            NotificationChromeUpdate::Rejected(NotificationChromeRejectReason::UnknownTransfer)
        }
    }
}

pub fn notification_chrome_command_from_portal(
    command: &PortalCommand,
) -> Option<NotificationChromeCommand> {
    match command {
        PortalCommand::DeliverNotification { transfer } => {
            Some(NotificationChromeCommand::Present {
                transfer: *transfer,
            })
        }
        PortalCommand::DropNotification { transfer } => Some(NotificationChromeCommand::Dismiss {
            transfer: *transfer,
        }),
        _ => None,
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

fn should_render(layer: &LayerSnapshot) -> bool {
    layer.opacity > 0.0 && !layer.geometry.is_empty() && layer.source != BufferSource::None
}

fn collect_buffer_imports(frame: &FrameSnapshot) -> Vec<BufferImportReport> {
    let layers_by_surface = frame
        .layers
        .iter()
        .map(|layer| (layer.surface, layer))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut imports = Vec::new();

    for command in &frame.commands {
        let Some(surface) = command.source else {
            continue;
        };
        if !seen.insert(surface) {
            continue;
        }
        if let Some(layer) = layers_by_surface.get(&surface) {
            if let Some(import) = buffer_import_report(layer) {
                imports.push(import);
            }
        }
    }

    imports
}

fn buffer_import_report(layer: &LayerSnapshot) -> Option<BufferImportReport> {
    let requested = match layer.source {
        BufferSource::None => return None,
        BufferSource::CpuBuffer { .. } => BufferImportPath::CpuReadback,
        BufferSource::XPixmap { .. } => BufferImportPath::XPixmap,
        BufferSource::DmaBuf { .. } => BufferImportPath::DmaBuf,
    };
    let used = BufferImportPath::CpuReadback;

    Some(BufferImportReport {
        surface: layer.surface,
        source: layer.source,
        requested,
        used,
        used_fallback: requested != used,
    })
}

fn chrome_descriptor_from_metadata(
    metadata: SanitizedChromeMetadata,
) -> Result<ChromeDescriptor, MetadataChromeRejectReason> {
    let label = metadata
        .label
        .map(|text| {
            if valid_chrome_label(&text) {
                Ok(DisplayLabel {
                    text,
                    redacted: metadata.label_redacted,
                })
            } else {
                Err(MetadataChromeRejectReason::InvalidLabel)
            }
        })
        .transpose()?;

    Ok(ChromeDescriptor {
        surface: metadata.surface,
        label,
        icon: metadata.icon,
        trust_level: metadata.trust_level,
        attention: metadata.attention,
        generation: metadata.generation,
    })
}

fn valid_chrome_label(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_CHROME_LABEL_LEN && !text.chars().any(char::is_control)
}

fn valid_notification_chrome_text(text: &str, max_len: usize) -> bool {
    !text.is_empty() && text.len() <= max_len && !text.chars().any(char::is_control)
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

pub fn validate_chrome_action(
    request: &ChromeActionRequest,
    nodes: &[LayoutNodeSnapshot],
) -> ChromeActionDecision {
    let Some(node) = nodes.iter().find(|node| node.surface == request.surface) else {
        warn!(
            surface_index = request.surface.index(),
            surface_generation = request.surface.generation(),
            request_generation = request.generation,
            action = ?request.kind,
            "rejected chrome action for unknown surface"
        );
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::UnknownSurface);
    };

    if node.generation != request.generation {
        warn!(
            surface_index = request.surface.index(),
            surface_generation = request.surface.generation(),
            request_generation = request.generation,
            current_generation = node.generation,
            action = ?request.kind,
            "rejected stale chrome action"
        );
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::StaleGeneration);
    }

    match request.kind {
        ChromeActionKind::CloseSurfaceRequested => {
            if node.capabilities.closable {
                debug!(
                    surface_index = request.surface.index(),
                    surface_generation = request.surface.generation(),
                    request_generation = request.generation,
                    action = ?request.kind,
                    "accepted chrome action"
                );
                ChromeActionDecision::RequestPoliteClose {
                    surface: request.surface,
                }
            } else {
                warn!(
                    surface_index = request.surface.index(),
                    surface_generation = request.surface.generation(),
                    request_generation = request.generation,
                    action = ?request.kind,
                    "rejected chrome action for non-closable surface"
                );
                ChromeActionDecision::Rejected(ChromeActionRejectReason::NotClosable)
            }
        }
    }
}

pub fn handle_session_event(event: SessionEvent, nodes: &[LayoutNodeSnapshot]) -> SessionUpdate {
    match event {
        SessionEvent::ChromeAction(request) => {
            let decision = validate_chrome_action(&request, nodes);
            let commands = match &decision {
                ChromeActionDecision::RequestPoliteClose { surface } => {
                    vec![SessionCommand::RequestPoliteClose { surface: *surface }]
                }
                ChromeActionDecision::Rejected(_) => Vec::new(),
            };
            debug!(
                surface_index = request.surface.index(),
                surface_generation = request.surface.generation(),
                action = ?request.kind,
                decision = ?decision,
                command_count = commands.len(),
                "handled chrome session event"
            );

            SessionUpdate {
                chrome_decision: Some(decision),
                commands,
            }
        }
        SessionEvent::SurfaceRemoved {
            transaction,
            surface,
            workspace,
        } => {
            debug!(
                transaction = transaction.raw(),
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                workspace = workspace.raw(),
                "handled surface removed session event"
            );
            SessionUpdate {
                chrome_decision: None,
                commands: vec![SessionCommand::SendWmRequest(WmRequestPacket {
                    transaction,
                    kind: WmRequestKind::SurfaceRemoved { surface, workspace },
                })],
            }
        }
    }
}

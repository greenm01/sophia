use crate::geometry::{Point, Rect, Region, Size, Transform};
use crate::ids::{
    DeviceId, IconTokenId, NamespaceId, OutputId, PortalTransferId, SeatId, SurfaceId,
    TransactionId, WorkspaceId, XWindowId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XWindowMirror {
    pub window: XWindowId,
    pub parent: Option<XWindowId>,
    pub children: Vec<XWindowId>,
    pub toplevel: Option<XWindowId>,
    pub client: Option<XWindowId>,
    pub mapped: bool,
    pub stack_rank: u32,
    pub geometry: Rect,
    pub namespace: Option<NamespaceId>,
    pub stale_metadata: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceSnapshot {
    pub surface: SurfaceId,
    pub window: XWindowId,
    pub toplevel: Option<XWindowId>,
    pub client: Option<XWindowId>,
    pub namespace: Option<NamespaceId>,
    pub mapped: bool,
    pub stack_rank: u32,
    pub geometry: Rect,
    pub source: BufferSource,
    pub damage: Region,
    pub generation: u64,
    pub resize_sync: ResizeSyncCapability,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerSnapshot {
    pub surface: SurfaceId,
    pub window: Option<XWindowId>,
    pub namespace: Option<NamespaceId>,
    pub stack_rank: u32,
    pub geometry: Rect,
    pub source: BufferSource,
    pub damage: Region,
    pub opacity: f32,
    pub crop: Option<Rect>,
    pub transform: Transform,
    pub generation: u64,
    pub resize_sync: ResizeSyncCapability,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResizeSyncCapability {
    #[default]
    ImplicitOnly,
    ExplicitSync,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferSource {
    None,
    XPixmap { pixmap: u32 },
    DmaBuf { handle: u64 },
    CpuBuffer { handle: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DamageFrame {
    pub output: OutputId,
    pub frame_serial: u64,
    pub buffer_age: u32,
    pub root_generation: u64,
    pub affected_surfaces: Vec<SurfaceId>,
    pub damage: Region,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameSnapshot {
    pub output: OutputId,
    pub output_size: Size,
    pub output_scale: u32,
    pub frame_serial: u64,
    pub layers: Vec<LayerSnapshot>,
    pub commands: Vec<RenderCommand>,
    pub damage: Region,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderCommand {
    pub kind: RenderCommandKind,
    pub source: Option<SurfaceId>,
    pub output: OutputId,
    pub target: Region,
    pub clip: Option<Region>,
    pub transform: Transform,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderCommandKind {
    Blit,
    Clear,
    Composite,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositorSurface {
    pub surface: SurfaceId,
    pub layer_generation: u64,
    pub geometry: Rect,
    pub active_buffer: BufferSource,
    pub output: Option<OutputId>,
    pub visible: bool,
    pub damage: Region,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutNodeSnapshot {
    pub surface: SurfaceId,
    pub workspace: WorkspaceId,
    pub kind: LayoutNodeKind,
    pub capabilities: LayoutNodeCapabilities,
    pub state: LayoutNodeState,
    pub constraints: SurfaceConstraints,
    pub geometry: Rect,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutNodeKind {
    Toplevel,
    Dialog,
    Utility,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutNodeCapabilities {
    pub movable: bool,
    pub resizable: bool,
    pub focusable: bool,
    pub closable: bool,
    pub fullscreenable: bool,
}

impl LayoutNodeCapabilities {
    pub const STANDARD_TOPLEVEL: Self = Self {
        movable: true,
        resizable: true,
        focusable: true,
        closable: true,
        fullscreenable: true,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutNodeState {
    pub focused: bool,
    pub urgent: bool,
    pub fullscreen: bool,
    pub floating: bool,
    pub visible: bool,
}

impl LayoutNodeState {
    pub const NORMAL: Self = Self {
        focused: false,
        urgent: false,
        fullscreen: false,
        floating: false,
        visible: true,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceConstraints {
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeDescriptor {
    pub surface: SurfaceId,
    pub label: Option<DisplayLabel>,
    pub icon: Option<IconTokenId>,
    pub trust_level: TrustLevel,
    pub attention: AttentionState,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayLabel {
    pub text: String,
    pub redacted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustLevel {
    Unknown,
    Trusted,
    Untrusted,
    Isolated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttentionState {
    None,
    Notice,
    Critical,
}

pub const SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerKind {
    Portal,
    Metadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerHealthState {
    Starting,
    Ready,
    Degraded,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrokerHealthError {
    MessageTooLong { len: usize, max: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokerHealthPacket {
    pub broker: BrokerKind,
    pub state: BrokerHealthState,
    pub generation: u64,
    pub message: Option<String>,
}

impl BrokerHealthPacket {
    pub fn new(
        broker: BrokerKind,
        state: BrokerHealthState,
        generation: u64,
        message: Option<String>,
    ) -> Result<Self, BrokerHealthError> {
        let packet = Self {
            broker,
            state,
            generation,
            message,
        };
        packet.validate()?;
        Ok(packet)
    }

    pub fn validate(&self) -> Result<(), BrokerHealthError> {
        if let Some(message) = &self.message {
            if message.len() > SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN {
                return Err(BrokerHealthError::MessageTooLong {
                    len: message.len(),
                    max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeActionRequest {
    pub surface: SurfaceId,
    pub generation: u64,
    pub kind: ChromeActionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeActionKind {
    CloseSurfaceRequested,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputEventPacket {
    pub serial: u64,
    pub seat: SeatId,
    pub device: DeviceId,
    pub time_msec: u64,
    pub kind: InputEventKind,
    pub global_position: Option<Point>,
    pub target_surface: Option<SurfaceId>,
    pub target_window: Option<XWindowId>,
    pub local_position: Option<Point>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEventKind {
    PointerMotion,
    PointerButton { button: u32, pressed: bool },
    Key { keycode: u32, pressed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputRoute {
    pub input_serial: u64,
    pub target_surface: Option<SurfaceId>,
    pub target_window: Option<XWindowId>,
    pub global_position: Point,
    pub local_position: Option<Point>,
    pub transform: Transform,
    pub outcome: InputRouteOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputRouteOutcome {
    Routed,
    NoTarget,
    StaleTarget,
    Denied,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XLibreRoutedInputRequest {
    pub serial: u64,
    pub seat: SeatId,
    pub device: DeviceId,
    pub time_msec: u64,
    pub target_window: XWindowId,
    pub local_position: Point,
    pub kind: InputEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XLibreRoutedInputDecision {
    pub serial: u64,
    pub target_window: XWindowId,
    pub outcome: XLibreRoutedInputOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XLibreRoutedInputOutcome {
    Accepted,
    RejectedStaleTarget,
    RejectedDeniedNamespace,
    RejectedActiveGrab,
    RejectedFocusPolicy,
    RejectedUnsupportedEvent,
}

pub const XLIBRE_ROUTED_INPUT_EXTENSION_NAME: &str = "SOPHIA-ROUTED-INPUT";
pub const XLIBRE_ROUTED_INPUT_MAJOR_VERSION: u16 = 0;
pub const XLIBRE_ROUTED_INPUT_MINOR_VERSION: u16 = 1;
pub const XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE: u8 = 1;
pub const XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH: u16 = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XLibreRoutedInputWireRequest {
    pub serial_hi: u32,
    pub serial_lo: u32,
    pub target_xid: u32,
    pub seat: u32,
    pub device: u32,
    pub time_msec: u32,
    pub local_x_24_8: i32,
    pub local_y_24_8: i32,
    pub event_code: u16,
    pub detail: u16,
    pub flags: u32,
}

impl XLibreRoutedInputRequest {
    pub fn to_wire_request(&self) -> XLibreRoutedInputWireRequest {
        let (event_code, detail, flags) = encode_routed_input_kind(self.kind);

        XLibreRoutedInputWireRequest {
            serial_hi: (self.serial >> 32) as u32,
            serial_lo: self.serial as u32,
            target_xid: self.target_window.xid(),
            seat: self.seat.raw() as u32,
            device: self.device.raw() as u32,
            time_msec: self.time_msec as u32,
            local_x_24_8: fixed_24_8(self.local_position.x),
            local_y_24_8: fixed_24_8(self.local_position.y),
            event_code,
            detail,
            flags,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XLibreRoutedInputWireError {
    UnsupportedEventCode,
    InvalidTarget,
    InvalidSeat,
    InvalidDevice,
}

impl XLibreRoutedInputWireRequest {
    pub fn to_request(self) -> Result<XLibreRoutedInputRequest, XLibreRoutedInputWireError> {
        let target_window = XWindowId::new(self.target_xid, 1);
        let seat = SeatId::from_raw(u64::from(self.seat));
        let device = DeviceId::from_raw(u64::from(self.device));

        if !target_window.is_valid() {
            return Err(XLibreRoutedInputWireError::InvalidTarget);
        }
        if !seat.is_valid() {
            return Err(XLibreRoutedInputWireError::InvalidSeat);
        }
        if !device.is_valid() {
            return Err(XLibreRoutedInputWireError::InvalidDevice);
        }

        Ok(XLibreRoutedInputRequest {
            serial: (u64::from(self.serial_hi) << 32) | u64::from(self.serial_lo),
            seat,
            device,
            time_msec: u64::from(self.time_msec),
            target_window,
            local_position: Point {
                x: f64::from(self.local_x_24_8) / 256.0,
                y: f64::from(self.local_y_24_8) / 256.0,
            },
            kind: decode_routed_input_kind(self.event_code, self.detail, self.flags)?,
        })
    }
}

fn encode_routed_input_kind(kind: InputEventKind) -> (u16, u16, u32) {
    match kind {
        InputEventKind::PointerMotion => (1, 0, 0),
        InputEventKind::PointerButton { button, pressed } => (2, button as u16, u32::from(pressed)),
        InputEventKind::Key { keycode, pressed } => (3, keycode as u16, u32::from(pressed)),
    }
}

fn decode_routed_input_kind(
    event_code: u16,
    detail: u16,
    flags: u32,
) -> Result<InputEventKind, XLibreRoutedInputWireError> {
    let pressed = (flags & 1) != 0;

    match event_code {
        1 => Ok(InputEventKind::PointerMotion),
        2 => Ok(InputEventKind::PointerButton {
            button: u32::from(detail),
            pressed,
        }),
        3 => Ok(InputEventKind::Key {
            keycode: u32::from(detail),
            pressed,
        }),
        _ => Err(XLibreRoutedInputWireError::UnsupportedEventCode),
    }
}

fn fixed_24_8(value: f64) -> i32 {
    (value * 256.0).round() as i32
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTransaction {
    pub transaction: TransactionId,
    pub requested_sizes: Vec<SurfaceSizeRequest>,
    pub focus: Option<SurfaceId>,
    pub render_positions: Vec<SurfacePlacement>,
    pub timeout_msec: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmRequestPacket {
    pub transaction: TransactionId,
    pub kind: WmRequestKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRequestKind {
    ManageSurface(WmManageSurface),
    RelayoutWorkspace(WmRelayoutWorkspace),
    SurfaceRemoved {
        surface: SurfaceId,
        workspace: WorkspaceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmManageSurface {
    pub node: LayoutNodeSnapshot,
    pub output: OutputId,
    pub workspace: WorkspaceId,
    pub bounds: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmRelayoutWorkspace {
    pub output: OutputId,
    pub workspace: WorkspaceId,
    pub bounds: Rect,
    pub nodes: Vec<LayoutNodeSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WmResponsePacket {
    pub transaction: TransactionId,
    pub commands: Vec<WmCommand>,
    pub timeout_msec: u32,
}

impl WmResponsePacket {
    pub fn into_layout_transaction(self) -> LayoutTransaction {
        let mut requested_sizes = Vec::new();
        let mut focus = None;
        let mut render_positions = Vec::new();

        for command in self.commands {
            match command {
                WmCommand::ConfigureSurface(request) => requested_sizes.push(request),
                WmCommand::FocusSurface(surface) => focus = Some(surface),
                WmCommand::AssignWorkspace { .. } => {}
                WmCommand::RenderSurface(placement) => render_positions.push(placement),
            }
        }

        LayoutTransaction {
            transaction: self.transaction,
            requested_sizes,
            focus,
            render_positions,
            timeout_msec: self.timeout_msec,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum WmCommand {
    ConfigureSurface(SurfaceSizeRequest),
    FocusSurface(SurfaceId),
    AssignWorkspace {
        surface: SurfaceId,
        workspace: WorkspaceId,
    },
    RenderSurface(SurfacePlacement),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionCommit {
    pub transaction: TransactionId,
    pub outcome: TransactionOutcome,
    pub applied_surfaces: Vec<SurfaceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionOutcome {
    Committed,
    RejectedStaleSurface,
    RejectedInvalidSurface,
    TimedOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceSizeRequest {
    pub surface: SurfaceId,
    pub size: Size,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePlacement {
    pub surface: SurfaceId,
    pub geometry: Rect,
    pub z_index: i32,
    pub crop: Option<Rect>,
    pub transform: Transform,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortalTransfer {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub kind: PortalTransferKind,
    pub mime_type: Option<String>,
    pub byte_size: u64,
    pub decision: PortalDecision,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortalTransferKind {
    Clipboard,
    DragAndDrop,
    FileHandoff,
    Screenshot,
    Notification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortalDecision {
    Pending,
    Allowed,
    Denied,
    Revoked,
}

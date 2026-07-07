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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_snapshot_is_cloneable_frame_data() {
        let surface = SurfaceId::new(0, 1);
        let snapshot = LayerSnapshot {
            surface,
            window: Some(XWindowId::new(42, 1)),
            namespace: Some(NamespaceId::from_raw(1)),
            stack_rank: 0,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            source: BufferSource::XPixmap { pixmap: 99 },
            damage: Region::single(Rect {
                x: 10,
                y: 20,
                width: 10,
                height: 10,
            }),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: 3,
        };

        let cloned = snapshot.clone();

        assert_eq!(cloned.surface, surface);
        assert_eq!(cloned.damage.rects.len(), 1);
    }

    #[test]
    fn layout_node_snapshot_carries_only_opaque_policy_data() {
        let node = LayoutNodeSnapshot {
            surface: SurfaceId::new(7, 1),
            workspace: WorkspaceId::from_raw(2),
            kind: LayoutNodeKind::Toplevel,
            capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
            state: LayoutNodeState::NORMAL,
            constraints: SurfaceConstraints {
                min_size: Some(Size {
                    width: 320,
                    height: 200,
                }),
                max_size: None,
            },
            geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            generation: 3,
        };

        assert_eq!(node.surface, SurfaceId::new(7, 1));
        assert_eq!(node.workspace, WorkspaceId::from_raw(2));
        assert!(node.capabilities.resizable);
        assert!(node.state.visible);
    }

    #[test]
    fn chrome_descriptor_carries_redacted_metadata_separately() {
        let chrome = ChromeDescriptor {
            surface: SurfaceId::new(9, 1),
            label: Some(DisplayLabel {
                text: "Private Window".to_owned(),
                redacted: true,
            }),
            icon: Some(IconTokenId::from_raw(4)),
            trust_level: TrustLevel::Untrusted,
            attention: AttentionState::Notice,
            generation: 1,
        };

        assert_eq!(chrome.surface, SurfaceId::new(9, 1));
        assert_eq!(
            chrome.label.as_ref().map(|label| label.redacted),
            Some(true)
        );
        assert_eq!(chrome.icon, Some(IconTokenId::from_raw(4)));
    }

    #[test]
    fn wm_manage_request_contains_only_blind_policy_data() {
        let surface = SurfaceId::new(2, 1);
        let workspace = WorkspaceId::from_raw(1);
        let request = WmRequestPacket {
            transaction: TransactionId::from_raw(5),
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                node: LayoutNodeSnapshot {
                    surface,
                    workspace,
                    kind: LayoutNodeKind::Toplevel,
                    capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
                    state: LayoutNodeState::NORMAL,
                    constraints: SurfaceConstraints {
                        min_size: None,
                        max_size: None,
                    },
                    geometry: Rect {
                        x: 0,
                        y: 0,
                        width: 320,
                        height: 200,
                    },
                    generation: 1,
                },
                output: OutputId::from_raw(1),
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
            }),
        };

        assert_eq!(request.transaction, TransactionId::from_raw(5));
        let WmRequestKind::ManageSurface(manage) = request.kind else {
            panic!("expected manage request");
        };
        assert_eq!(manage.node.surface, surface);
        assert_eq!(manage.workspace, workspace);
    }

    #[test]
    fn wm_response_converts_to_layout_transaction() {
        let surface = SurfaceId::new(2, 1);
        let workspace = WorkspaceId::from_raw(1);
        let response = WmResponsePacket {
            transaction: TransactionId::from_raw(5),
            commands: vec![
                WmCommand::AssignWorkspace { surface, workspace },
                WmCommand::ConfigureSurface(SurfaceSizeRequest {
                    surface,
                    size: Size {
                        width: 640,
                        height: 480,
                    },
                }),
                WmCommand::FocusSurface(surface),
                WmCommand::RenderSurface(SurfacePlacement {
                    surface,
                    geometry: Rect {
                        x: 10,
                        y: 20,
                        width: 640,
                        height: 480,
                    },
                    z_index: 3,
                    crop: None,
                    transform: Transform::IDENTITY,
                }),
            ],
            timeout_msec: 250,
        };

        let transaction = response.into_layout_transaction();

        assert_eq!(transaction.transaction, TransactionId::from_raw(5));
        assert_eq!(transaction.requested_sizes.len(), 1);
        assert_eq!(transaction.focus, Some(surface));
        assert_eq!(transaction.render_positions.len(), 1);
        assert_eq!(transaction.render_positions[0].z_index, 3);
        assert_eq!(transaction.timeout_msec, 250);
    }

    #[test]
    fn xlibre_routed_input_request_is_targeted_but_not_direct_delivery() {
        let request = XLibreRoutedInputRequest {
            serial: 99,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: 1_000,
            target_window: XWindowId::new(0x42, 1),
            local_position: Point { x: 12.5, y: 9.0 },
            kind: InputEventKind::PointerButton {
                button: 1,
                pressed: true,
            },
        };

        assert_eq!(request.serial, 99);
        assert_eq!(request.target_window, XWindowId::new(0x42, 1));
        assert_eq!(request.local_position.x, 12.5);
        assert_eq!(request.device, DeviceId::from_raw(2));
        assert_eq!(
            request.kind,
            InputEventKind::PointerButton {
                button: 1,
                pressed: true,
            }
        );
    }

    #[test]
    fn xlibre_routed_input_decision_carries_server_side_rejection() {
        let decision = XLibreRoutedInputDecision {
            serial: 100,
            target_window: XWindowId::new(0x55, 3),
            outcome: XLibreRoutedInputOutcome::RejectedDeniedNamespace,
        };

        assert_eq!(decision.serial, 100);
        assert_eq!(
            decision.outcome,
            XLibreRoutedInputOutcome::RejectedDeniedNamespace
        );
    }
}

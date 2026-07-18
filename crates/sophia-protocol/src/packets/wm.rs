use super::LayoutNodeSnapshot;
use crate::geometry::{Rect, Size, Transform};
use crate::ids::{OutputId, SurfaceId, TransactionId, WmActionId, WorkspaceId};

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTransaction {
    pub transaction: TransactionId,
    pub requested_sizes: Vec<SurfaceSizeRequest>,
    pub focus: Option<SurfaceId>,
    pub render_positions: Vec<SurfacePlacement>,
    pub timeout_msec: u32,
}

pub const WM_API_VERSION: u16 = 2;
pub const WM_MAX_BINDINGS: usize = 256;
pub const WM_DEFAULT_WORKSPACES: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmCapabilities {
    pub bits: u64,
}

impl WmCapabilities {
    pub const BINDINGS: u64 = 1 << 0;
    pub const WORKSPACES: u64 = 1 << 1;
    pub const SESSION_ACTIONS: u64 = 1 << 2;
    pub const SUPPORTED: u64 = Self::BINDINGS | Self::WORKSPACES | Self::SESSION_ACTIONS;

    pub const fn all_supported() -> Self {
        Self {
            bits: Self::SUPPORTED,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WmModifierMask {
    pub bits: u32,
}

impl WmModifierMask {
    pub const SHIFT: u32 = 1 << 0;
    pub const CONTROL: u32 = 1 << 1;
    pub const ALT: u32 = 1 << 2;
    pub const SUPER: u32 = 1 << 3;
    pub const SUPPORTED: u32 = Self::SHIFT | Self::CONTROL | Self::ALT | Self::SUPER;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmBindingRegistration {
    pub action: WmActionId,
    pub keycode: u32,
    pub modifiers: WmModifierMask,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmHello {
    pub api_version: u16,
    pub capabilities: WmCapabilities,
    pub bindings: Vec<WmBindingRegistration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmOutputWorkspace {
    pub output: OutputId,
    pub workspace: WorkspaceId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmSessionAction {
    LaunchTerminal,
    LaunchApplicationMenu,
    LaunchFirefox,
    CloseFocused,
    Logout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmSessionDescriptor {
    pub api_version: u16,
    pub workspaces: Vec<WorkspaceId>,
    pub active_workspaces: Vec<WmOutputWorkspace>,
    pub session_actions: Vec<WmSessionAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmActionActivation {
    pub action: WmActionId,
    pub output: OutputId,
    pub workspace: WorkspaceId,
    pub focused_surface: Option<SurfaceId>,
    pub nodes: Vec<LayoutNodeSnapshot>,
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
    ActionActivated(WmActionActivation),
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
                WmCommand::ActivateWorkspace { .. } | WmCommand::RequestSessionAction { .. } => {}
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
    ActivateWorkspace {
        output: OutputId,
        workspace: WorkspaceId,
    },
    RequestSessionAction {
        action: WmSessionAction,
        target: Option<SurfaceId>,
    },
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

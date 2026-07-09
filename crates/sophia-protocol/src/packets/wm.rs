use super::LayoutNodeSnapshot;
use crate::geometry::{Rect, Size, Transform};
use crate::ids::{OutputId, SurfaceId, TransactionId, WorkspaceId};

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

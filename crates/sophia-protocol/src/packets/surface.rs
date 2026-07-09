use super::{AuthorityKind, AuthorityLocalId};
use crate::geometry::{Rect, Region, Size, Transform};
use crate::ids::{NamespaceId, OutputId, SurfaceId, TransactionId, WorkspaceId, XWindowId};

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

impl SurfaceSnapshot {
    pub fn to_authority_surface(&self, authority: AuthorityKind) -> AuthoritySurface {
        AuthoritySurface {
            authority,
            local_id: AuthorityLocalId::from(self.window),
            surface: self.surface,
            namespace: self.namespace,
            mapped: self.mapped,
            geometry: self.geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: self.generation,
        }
    }

    pub fn to_surface_transaction(
        &self,
        transaction: TransactionId,
        authority: AuthorityKind,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> SurfaceTransaction {
        SurfaceTransaction {
            transaction,
            authority,
            surface: self.surface,
            namespace: self.namespace,
            target_geometry: self.geometry,
            target_buffer: self.source,
            damage: self.damage.clone(),
            readiness,
            timeout_msec,
            previous_committed_generation,
        }
    }
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

impl LayerSnapshot {
    pub fn to_surface_transaction(
        &self,
        transaction: TransactionId,
        authority: AuthorityKind,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> SurfaceTransaction {
        SurfaceTransaction {
            transaction,
            authority,
            surface: self.surface,
            namespace: self.namespace,
            target_geometry: self.geometry,
            target_buffer: self.source,
            damage: self.damage.clone(),
            readiness,
            timeout_msec,
            previous_committed_generation,
        }
    }
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
pub struct AuthoritySurface {
    pub authority: AuthorityKind,
    pub local_id: AuthorityLocalId,
    pub surface: SurfaceId,
    pub namespace: Option<NamespaceId>,
    pub mapped: bool,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceTransaction {
    pub transaction: TransactionId,
    pub authority: AuthorityKind,
    pub surface: SurfaceId,
    pub namespace: Option<NamespaceId>,
    pub target_geometry: Rect,
    pub target_buffer: BufferSource,
    pub damage: Region,
    pub readiness: SurfaceTransactionReadiness,
    pub timeout_msec: u32,
    pub previous_committed_generation: u64,
}

impl SurfaceTransaction {
    pub fn from_layer_snapshot(
        transaction: TransactionId,
        authority: AuthorityKind,
        layer: &LayerSnapshot,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> Self {
        layer.to_surface_transaction(
            transaction,
            authority,
            readiness,
            timeout_msec,
            previous_committed_generation,
        )
    }

    pub fn from_surface_snapshot(
        transaction: TransactionId,
        authority: AuthorityKind,
        surface: &SurfaceSnapshot,
        readiness: SurfaceTransactionReadiness,
        timeout_msec: u32,
        previous_committed_generation: u64,
    ) -> Self {
        surface.to_surface_transaction(
            transaction,
            authority,
            readiness,
            timeout_msec,
            previous_committed_generation,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceTransactionReadiness {
    Pending,
    Ready,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedSurfaceState {
    pub surface: SurfaceId,
    pub committed_generation: u64,
    pub geometry: Rect,
    pub buffer: BufferSource,
    pub damage: Region,
}

impl CommittedSurfaceState {
    pub fn from_layer_snapshot(layer: &LayerSnapshot) -> Self {
        Self {
            surface: layer.surface,
            committed_generation: layer.generation,
            geometry: layer.geometry,
            buffer: layer.source,
            damage: layer.damage.clone(),
        }
    }
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

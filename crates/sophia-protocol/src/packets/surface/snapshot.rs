use super::*;

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

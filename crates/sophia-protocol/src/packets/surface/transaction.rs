use super::*;

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

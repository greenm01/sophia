use std::collections::BTreeMap;

use sophia_protocol::{LayerSnapshot, Size, SurfaceId, TransactionId};
use sophia_x_authority::XAuthorityObservedTransactionBatch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResizeRollbackRequest {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub size: Size,
}

/// Owns the last committed authority sizes and the fence used after a timed
/// out resize. Late pixels for the abandoned size are rejected until the
/// authority confirms the compensating configure at the committed size.
#[derive(Debug)]
pub struct ResizeRollbackCoordinator {
    committed_sizes: BTreeMap<SurfaceId, Size>,
    rollback_sizes: BTreeMap<SurfaceId, Size>,
    rejected_sizes: BTreeMap<SurfaceId, Size>,
    next_transaction: u64,
}

impl Default for ResizeRollbackCoordinator {
    fn default() -> Self {
        Self {
            committed_sizes: BTreeMap::new(),
            rollback_sizes: BTreeMap::new(),
            rejected_sizes: BTreeMap::new(),
            next_transaction: 1 << 63,
        }
    }
}

impl ResizeRollbackCoordinator {
    pub fn committed_size(&self, surface: SurfaceId) -> Option<Size> {
        self.committed_sizes.get(&surface).copied()
    }

    pub fn record_committed(&mut self, surface: SurfaceId, size: Size) {
        self.committed_sizes.insert(surface, size);
        if self.rejected_sizes.get(&surface) == Some(&size) {
            self.rejected_sizes.remove(&surface);
        }
    }

    pub fn request_allowed(&self, surface: SurfaceId, size: Size) -> bool {
        self.rejected_sizes.get(&surface) != Some(&size)
    }

    pub fn accept_observation(&mut self, surface: SurfaceId, size: Size) -> bool {
        let Some(expected) = self.rollback_sizes.get(&surface).copied() else {
            return true;
        };
        if size != expected {
            return false;
        }
        self.rollback_sizes.remove(&surface);
        self.rejected_sizes.remove(&surface);
        true
    }

    pub fn begin_rollback(
        &mut self,
        requests: impl IntoIterator<Item = (SurfaceId, Size)>,
    ) -> Result<Vec<ResizeRollbackRequest>, &'static str> {
        let sizes = requests
            .into_iter()
            .map(|(surface, rejected)| {
                self.rejected_sizes.insert(surface, rejected);
                self.committed_size(surface)
                    .map(|size| (surface, size))
                    .ok_or("live WM rollback surface has no committed authority size")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("live WM rollback transaction ID exhausted")?;
        Ok(sizes
            .into_iter()
            .map(|(surface, size)| {
                self.rollback_sizes.insert(surface, size);
                ResizeRollbackRequest {
                    transaction,
                    surface,
                    size,
                }
            })
            .collect())
    }

    pub fn remove(&mut self, surface: SurfaceId) {
        self.committed_sizes.remove(&surface);
        self.rollback_sizes.remove(&surface);
        self.rejected_sizes.remove(&surface);
    }

    pub fn rollback_pending(&self, surface: SurfaceId) -> bool {
        self.rollback_sizes.contains_key(&surface)
    }

    pub fn rollback_surfaces(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.rollback_sizes.keys().copied()
    }
}

/// Projects authority pixels onto the current layout without dropping any
/// generation-bearing transaction or associated buffer update.
pub fn project_authority_batch_onto_layout(
    mut batch: XAuthorityObservedTransactionBatch,
    layers: &BTreeMap<SurfaceId, LayerSnapshot>,
) -> XAuthorityObservedTransactionBatch {
    for transaction in &mut batch.transactions {
        if let Some(layer) = layers.get(&transaction.surface) {
            transaction.target_geometry = layer.geometry;
        }
    }
    batch
}

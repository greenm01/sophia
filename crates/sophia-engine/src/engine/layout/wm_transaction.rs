use super::*;

impl HeadlessEngine {
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
}

fn moved_damage(old_geometry: Rect, new_geometry: Rect) -> Region {
    let mut damage = Region::single(old_geometry);
    damage.extend(&Region::single(new_geometry));
    damage
}

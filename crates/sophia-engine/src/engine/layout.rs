use crate::prelude::*;
use crate::{
    EngineError, HeadlessEngine, SurfaceTransactionCommitReadiness, SurfaceVisualStateTable,
};

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

    pub fn committed_state_from_layer(&self, layer: &LayerSnapshot) -> CommittedSurfaceState {
        CommittedSurfaceState::from_layer_snapshot(layer)
    }

    pub fn project_committed_surface_state(
        &self,
        committed: &CommittedSurfaceState,
        template: &LayerSnapshot,
    ) -> Result<LayerSnapshot, EngineError> {
        if !committed.surface.is_valid() || !template.surface.is_valid() {
            return Err(EngineError::InvalidSurface);
        }
        if committed.surface != template.surface {
            return Err(EngineError::InvalidSurface);
        }

        let mut layer = template.clone();
        layer.geometry = committed.geometry;
        layer.source = committed.buffer;
        layer.damage = committed.damage.clone();
        layer.generation = committed.committed_generation;
        Ok(layer)
    }

    pub fn project_committed_surface_states(
        &self,
        committed: &[CommittedSurfaceState],
        templates: &[LayerSnapshot],
    ) -> Result<Vec<LayerSnapshot>, EngineError> {
        let templates_by_surface = templates
            .iter()
            .map(|template| (template.surface, template))
            .collect::<BTreeMap<_, _>>();

        committed
            .iter()
            .map(|state| {
                let Some(template) = templates_by_surface.get(&state.surface) else {
                    return Err(EngineError::InvalidSurface);
                };
                self.project_committed_surface_state(state, template)
            })
            .collect()
    }

    #[instrument(skip_all, fields(
        transaction = transaction.raw(),
        transaction_count = transactions.len(),
        committed_count = committed.len()
    ))]
    pub fn commit_surface_transactions(
        &self,
        transaction: TransactionId,
        transactions: &[SurfaceTransaction],
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> TransactionCommit {
        let applied_surfaces = transactions
            .iter()
            .map(|surface_transaction| surface_transaction.surface)
            .collect::<Vec<_>>();

        let visual_state = SurfaceVisualStateTable::from_committed_states(committed.clone());

        for surface_transaction in transactions {
            if surface_transaction.transaction != transaction {
                warn!(
                    expected_transaction = transaction.raw(),
                    actual_transaction = surface_transaction.transaction.raw(),
                    "rejected authority transaction batch with mismatched transaction ID"
                );
                return TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::RejectedStaleSurface,
                    applied_surfaces: Vec::new(),
                };
            }

            match visual_state.transaction_commit_readiness(surface_transaction) {
                SurfaceTransactionCommitReadiness::Ready => {}
                SurfaceTransactionCommitReadiness::NotReady(
                    SurfaceTransactionReadiness::Pending | SurfaceTransactionReadiness::TimedOut,
                ) => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        readiness = ?surface_transaction.readiness,
                        "preserving committed surface state because authority transaction is not ready"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::TimedOut,
                        applied_surfaces: Vec::new(),
                    };
                }
                SurfaceTransactionCommitReadiness::NotReady(
                    SurfaceTransactionReadiness::Failed,
                ) => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        "rejected failed authority transaction"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedStaleSurface,
                        applied_surfaces: Vec::new(),
                    };
                }
                SurfaceTransactionCommitReadiness::InvalidSurface
                | SurfaceTransactionCommitReadiness::EmptyGeometry
                | SurfaceTransactionCommitReadiness::MissingBuffer
                | SurfaceTransactionCommitReadiness::NotReady(SurfaceTransactionReadiness::Ready) =>
                {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        readiness = ?visual_state.transaction_commit_readiness(surface_transaction),
                        "rejected malformed authority transaction"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedInvalidSurface,
                        applied_surfaces: Vec::new(),
                    };
                }
                SurfaceTransactionCommitReadiness::StaleGeneration { current, expected } => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        current_generation = current,
                        previous_committed_generation = expected,
                        "rejected stale authority transaction"
                    );
                    return TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedStaleSurface,
                        applied_surfaces: Vec::new(),
                    };
                }
            }
        }

        let mut next_committed = committed.clone();
        for surface_transaction in transactions {
            let next_state = CommittedSurfaceState {
                surface: surface_transaction.surface,
                committed_generation: surface_transaction
                    .previous_committed_generation
                    .saturating_add(1),
                geometry: surface_transaction.target_geometry,
                buffer: surface_transaction.target_buffer,
                damage: surface_transaction.damage.clone(),
            };

            if let Some(index) = next_committed
                .iter()
                .position(|state| state.surface == surface_transaction.surface)
            {
                next_committed[index] = next_state;
            } else {
                next_committed.push(next_state);
            }
        }

        *committed = next_committed;
        debug!(
            transaction = transaction.raw(),
            applied_surfaces = applied_surfaces.len(),
            outcome = ?TransactionOutcome::Committed,
            "committed authority surface transactions"
        );

        TransactionCommit {
            transaction,
            outcome: TransactionOutcome::Committed,
            applied_surfaces,
        }
    }
}

fn moved_damage(old_geometry: Rect, new_geometry: Rect) -> Region {
    let mut damage = Region::single(old_geometry);
    damage.extend(&Region::single(new_geometry));
    damage
}

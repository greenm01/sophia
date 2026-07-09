use super::*;

impl HeadlessEngine {
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

use super::*;
use crate::PreparedSurfaceCommit;

impl HeadlessEngine {
    #[instrument(skip_all, fields(
        transaction = transaction.raw(),
        transaction_count = transactions.len(),
        committed_count = committed.len()
    ))]
    pub fn prepare_surface_transactions(
        &self,
        transaction: TransactionId,
        transactions: &[SurfaceTransaction],
        committed: &[CommittedSurfaceState],
    ) -> PreparedSurfaceCommit {
        let applied_surfaces = transactions
            .iter()
            .map(|surface_transaction| surface_transaction.surface)
            .collect::<Vec<_>>();

        let baseline = committed.to_vec();
        let visual_state = SurfaceVisualStateTable::from_committed_states(baseline.clone());

        for surface_transaction in transactions {
            if surface_transaction.transaction != transaction {
                warn!(
                    expected_transaction = transaction.raw(),
                    actual_transaction = surface_transaction.transaction.raw(),
                    "rejected authority transaction batch with mismatched transaction ID"
                );
                let commit = TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::RejectedStaleSurface,
                    applied_surfaces: Vec::new(),
                };
                return PreparedSurfaceCommit::new(commit, baseline.clone(), baseline);
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
                    let commit = TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::TimedOut,
                        applied_surfaces: Vec::new(),
                    };
                    return PreparedSurfaceCommit::new(commit, baseline.clone(), baseline);
                }
                SurfaceTransactionCommitReadiness::NotReady(
                    SurfaceTransactionReadiness::Failed,
                ) => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        "rejected failed authority transaction"
                    );
                    let commit = TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedStaleSurface,
                        applied_surfaces: Vec::new(),
                    };
                    return PreparedSurfaceCommit::new(commit, baseline.clone(), baseline);
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
                    let commit = TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedInvalidSurface,
                        applied_surfaces: Vec::new(),
                    };
                    return PreparedSurfaceCommit::new(commit, baseline.clone(), baseline);
                }
                SurfaceTransactionCommitReadiness::StaleGeneration { current, expected } => {
                    warn!(
                        transaction = transaction.raw(),
                        surface_index = surface_transaction.surface.index(),
                        current_generation = current,
                        previous_committed_generation = expected,
                        "rejected stale authority transaction"
                    );
                    let commit = TransactionCommit {
                        transaction,
                        outcome: TransactionOutcome::RejectedStaleSurface,
                        applied_surfaces: Vec::new(),
                    };
                    return PreparedSurfaceCommit::new(commit, baseline.clone(), baseline);
                }
            }
        }

        let mut next_committed = baseline.clone();
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

        debug!(
            transaction = transaction.raw(),
            applied_surfaces = applied_surfaces.len(),
            outcome = ?TransactionOutcome::Committed,
            "prepared authority surface transactions"
        );

        let commit = TransactionCommit {
            transaction,
            outcome: TransactionOutcome::Committed,
            applied_surfaces,
        };
        PreparedSurfaceCommit::new(commit, baseline, next_committed)
    }

    /// Applies a previously prepared visual commit if the baseline of every
    /// touched surface is still authoritative. Unrelated surface commits are
    /// retained. Presentation backends call this only after the matching page
    /// flip becomes visible.
    pub fn apply_prepared_surface_commit(
        &self,
        prepared: PreparedSurfaceCommit,
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> TransactionCommit {
        if !prepared.is_ready() {
            return prepared.commit;
        }
        let touched_baseline_is_current = prepared.commit.applied_surfaces.iter().all(|surface| {
            let baseline = prepared
                .baseline
                .iter()
                .find(|state| state.surface == *surface);
            let current = committed.iter().find(|state| state.surface == *surface);
            baseline == current
        });
        if !touched_baseline_is_current {
            warn!(
                transaction = prepared.transaction().raw(),
                "discarded prepared surface commit because its baseline became stale"
            );
            return TransactionCommit {
                transaction: prepared.transaction(),
                outcome: TransactionOutcome::RejectedStaleSurface,
                applied_surfaces: Vec::new(),
            };
        }

        let PreparedSurfaceCommit {
            commit, candidate, ..
        } = prepared;
        for surface in &commit.applied_surfaces {
            let candidate = candidate
                .iter()
                .find(|state| state.surface == *surface)
                .expect("a prepared applied surface must have candidate state")
                .clone();
            if let Some(index) = committed.iter().position(|state| state.surface == *surface) {
                committed[index] = candidate;
            } else {
                committed.push(candidate);
            }
        }
        debug!(
            transaction = commit.transaction.raw(),
            committed_count = committed.len(),
            "applied prepared authority surface transaction after presentation"
        );
        commit
    }

    pub fn commit_surface_transactions(
        &self,
        transaction: TransactionId,
        transactions: &[SurfaceTransaction],
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> TransactionCommit {
        let prepared = self.prepare_surface_transactions(transaction, transactions, committed);
        self.apply_prepared_surface_commit(prepared, committed)
    }
}

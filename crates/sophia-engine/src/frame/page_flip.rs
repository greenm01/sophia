use crate::prelude::*;
use crate::{HeadlessEngine, SurfaceTransactionCommitReadiness, SurfaceVisualStateTable};

use super::clock::FrameClockTick;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageFlipTransactionBatch {
    pub output: OutputId,
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageFlipCommitOutcome {
    Idle,
    WaitingForOutput {
        expected: OutputId,
        actual: OutputId,
        transaction: TransactionId,
    },
    WaitingForTransactionReadiness {
        transaction: TransactionId,
        pending_surfaces: Vec<SurfaceId>,
    },
    Committed {
        frame_serial: u64,
        commit: TransactionCommit,
    },
    Rejected {
        frame_serial: u64,
        commit: TransactionCommit,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PageFlipCommitGate {
    staged: Option<PageFlipTransactionBatch>,
}

impl PageFlipCommitGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stage(
        &mut self,
        output: OutputId,
        transaction: TransactionId,
        transactions: Vec<SurfaceTransaction>,
    ) {
        self.staged = Some(PageFlipTransactionBatch {
            output,
            transaction,
            transactions,
        });
    }

    pub fn staged(&self) -> Option<&PageFlipTransactionBatch> {
        self.staged.as_ref()
    }

    pub fn clear(&mut self) -> Option<PageFlipTransactionBatch> {
        self.staged.take()
    }

    pub fn commit_on_page_flip(
        &mut self,
        engine: &HeadlessEngine,
        tick: FrameClockTick,
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> PageFlipCommitOutcome {
        let Some(batch) = self.staged.as_ref() else {
            return PageFlipCommitOutcome::Idle;
        };

        if batch.output != tick.output {
            return PageFlipCommitOutcome::WaitingForOutput {
                expected: batch.output,
                actual: tick.output,
                transaction: batch.transaction,
            };
        }

        let visual_state = SurfaceVisualStateTable::from_committed_states(committed.clone());
        let pending_surfaces = batch
            .transactions
            .iter()
            .filter_map(|transaction| {
                match visual_state.transaction_commit_readiness(transaction) {
                    SurfaceTransactionCommitReadiness::NotReady(
                        SurfaceTransactionReadiness::Pending,
                    ) => Some(transaction.surface),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        if !pending_surfaces.is_empty() {
            return PageFlipCommitOutcome::WaitingForTransactionReadiness {
                transaction: batch.transaction,
                pending_surfaces,
            };
        }

        let Some(batch) = self.staged.take() else {
            return PageFlipCommitOutcome::Idle;
        };
        let commit =
            engine.commit_surface_transactions(batch.transaction, &batch.transactions, committed);

        if commit.outcome == TransactionOutcome::Committed {
            PageFlipCommitOutcome::Committed {
                frame_serial: tick.frame_serial,
                commit,
            }
        } else {
            PageFlipCommitOutcome::Rejected {
                frame_serial: tick.frame_serial,
                commit,
            }
        }
    }
}

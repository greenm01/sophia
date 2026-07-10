use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveAtomicScanoutCommitReport {
    pub status: LiveAtomicScanoutCommitStatus,
    pub page_flip: LivePageFlipEvent,
}

impl LiveAtomicScanoutCommitReport {
    pub fn from_page_flip_outcome(outcome: &PageFlipCommitOutcome) -> Self {
        Self {
            status: match outcome {
                PageFlipCommitOutcome::Idle => LiveAtomicScanoutCommitStatus::Idle,
                PageFlipCommitOutcome::WaitingForOutput { .. } => {
                    LiveAtomicScanoutCommitStatus::WaitingForOutput
                }
                PageFlipCommitOutcome::WaitingForTransactionReadiness { .. } => {
                    LiveAtomicScanoutCommitStatus::WaitingForTransactionReadiness
                }
                PageFlipCommitOutcome::Committed { .. } => LiveAtomicScanoutCommitStatus::Committed,
                PageFlipCommitOutcome::Rejected { commit, .. }
                    if commit.outcome == TransactionOutcome::TimedOut =>
                {
                    LiveAtomicScanoutCommitStatus::TimedOut
                }
                PageFlipCommitOutcome::Rejected { .. } => LiveAtomicScanoutCommitStatus::Rejected,
            },
            page_flip: LivePageFlipEvent::from_commit_outcome(outcome),
        }
    }

    pub fn from_page_flip_callback_and_outcome(
        callback: &LivePageFlipCallbackReport,
        outcome: &PageFlipCommitOutcome,
    ) -> Self {
        match callback.decision {
            LivePageFlipCallbackDecision::Accepted => {
                if let Some(outcome_frame_serial) = page_flip_outcome_frame_serial(outcome) {
                    if callback.event.frame_serial != Some(outcome_frame_serial) {
                        return Self {
                            status: LiveAtomicScanoutCommitStatus::Rejected,
                            page_flip: LivePageFlipEvent {
                                status: LivePageFlipEventStatus::Rejected,
                                frame_serial: callback.event.frame_serial,
                            },
                        };
                    }
                }

                Self::from_page_flip_outcome(outcome)
            }
            LivePageFlipCallbackDecision::RejectedUnexpectedOutput => Self {
                status: LiveAtomicScanoutCommitStatus::WaitingForOutput,
                page_flip: callback.event,
            },
            LivePageFlipCallbackDecision::RejectedStaleFrameSerial => Self {
                status: LiveAtomicScanoutCommitStatus::Rejected,
                page_flip: callback.event,
            },
        }
    }
}

fn page_flip_outcome_frame_serial(outcome: &PageFlipCommitOutcome) -> Option<u64> {
    match outcome {
        PageFlipCommitOutcome::Committed { frame_serial, .. }
        | PageFlipCommitOutcome::Rejected { frame_serial, .. } => Some(*frame_serial),
        PageFlipCommitOutcome::Idle
        | PageFlipCommitOutcome::WaitingForOutput { .. }
        | PageFlipCommitOutcome::WaitingForTransactionReadiness { .. } => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveAtomicScanoutCommitStatus {
    Idle,
    WaitingForOutput,
    WaitingForTransactionReadiness,
    Committed,
    TimedOut,
    Rejected,
}

pub trait LiveAtomicScanoutCommitter {
    fn commit_atomic_scanout(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport;

    fn commit_atomic_scanout_after_page_flip(
        &mut self,
        callback: &LivePageFlipCallbackReport,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeAtomicScanoutCommitter {
    committed: usize,
}

impl FakeAtomicScanoutCommitter {
    pub const fn committed_count(&self) -> usize {
        self.committed
    }
}

impl LiveAtomicScanoutCommitter for FakeAtomicScanoutCommitter {
    fn commit_atomic_scanout(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report = LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome);
        if report.status == LiveAtomicScanoutCommitStatus::Committed {
            self.committed = self.committed.saturating_add(1);
        }
        report
    }

    fn commit_atomic_scanout_after_page_flip(
        &mut self,
        callback: &LivePageFlipCallbackReport,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report =
            LiveAtomicScanoutCommitReport::from_page_flip_callback_and_outcome(callback, outcome);
        if report.status == LiveAtomicScanoutCommitStatus::Committed {
            self.committed = self.committed.saturating_add(1);
        }
        report
    }
}

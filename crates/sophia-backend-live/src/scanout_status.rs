use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveScanoutReadinessReport {
    pub status: LiveScanoutReadinessStatus,
}

impl LiveScanoutReadinessReport {
    pub(crate) fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        Self::from_output_and_presentation(backend.selected_output().is_some(), presentation)
    }

    pub(crate) fn from_output_and_presentation(
        output_available: bool,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        if !output_available {
            return Self {
                status: LiveScanoutReadinessStatus::OutputUnavailable,
            };
        }

        Self {
            status: match presentation.status {
                LiveRendererPresentationStatus::Ready => LiveScanoutReadinessStatus::Ready,
                LiveRendererPresentationStatus::Unavailable => {
                    LiveScanoutReadinessStatus::PresentationUnavailable
                }
                LiveRendererPresentationStatus::Degraded => LiveScanoutReadinessStatus::Degraded,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveScanoutReadinessStatus {
    Ready,
    OutputUnavailable,
    PresentationUnavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveKmsScanoutTargetReport {
    pub status: LiveKmsScanoutTargetStatus,
    pub size: Option<Size>,
}

impl LiveKmsScanoutTargetReport {
    pub(crate) fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        Self::from_output_target_and_presentation(
            backend.selected_output().map(|output| output.size),
            backend.selected_gbm_egl_frame_target(),
            presentation,
        )
    }

    pub(crate) fn from_output_target_and_presentation(
        output_size: Option<Size>,
        frame_target: Option<LiveGbmEglFrameTargetRecord>,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        let Some(output_size) = output_size else {
            return Self {
                status: LiveKmsScanoutTargetStatus::OutputUnavailable,
                size: None,
            };
        };

        let Some(frame_target) = frame_target else {
            return Self {
                status: LiveKmsScanoutTargetStatus::FrameTargetUnavailable,
                size: Some(output_size),
            };
        };

        if frame_target.status != LiveGbmEglFrameTargetStatus::Ready {
            return Self {
                status: LiveKmsScanoutTargetStatus::InvalidFrameTarget,
                size: Some(frame_target.size),
            };
        }

        if frame_target.size != output_size {
            return Self {
                status: LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
                size: Some(frame_target.size),
            };
        }

        Self {
            status: match presentation.status {
                LiveRendererPresentationStatus::Ready => LiveKmsScanoutTargetStatus::Ready,
                LiveRendererPresentationStatus::Unavailable => {
                    LiveKmsScanoutTargetStatus::PresentationUnavailable
                }
                LiveRendererPresentationStatus::Degraded => LiveKmsScanoutTargetStatus::Degraded,
            },
            size: Some(frame_target.size),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKmsScanoutTargetStatus {
    Ready,
    OutputUnavailable,
    FrameTargetUnavailable,
    InvalidFrameTarget,
    FrameTargetSizeMismatch,
    PresentationUnavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipEvent {
    pub status: LivePageFlipEventStatus,
    pub frame_serial: Option<u64>,
}

impl LivePageFlipEvent {
    pub const fn from_scanout_status(status: LiveScanoutReadinessStatus) -> Self {
        Self {
            status: match status {
                LiveScanoutReadinessStatus::Ready => LivePageFlipEventStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable => {
                    LivePageFlipEventStatus::OutputUnavailable
                }
                LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LivePageFlipEventStatus::PresentationUnavailable
                }
                LiveScanoutReadinessStatus::Degraded => LivePageFlipEventStatus::Degraded,
            },
            frame_serial: None,
        }
    }

    pub const fn from_kms_scanout_target_status(status: LiveKmsScanoutTargetStatus) -> Self {
        Self {
            status: match status {
                LiveKmsScanoutTargetStatus::Ready => LivePageFlipEventStatus::Ready,
                LiveKmsScanoutTargetStatus::OutputUnavailable => {
                    LivePageFlipEventStatus::OutputUnavailable
                }
                LiveKmsScanoutTargetStatus::FrameTargetUnavailable => {
                    LivePageFlipEventStatus::FrameTargetUnavailable
                }
                LiveKmsScanoutTargetStatus::InvalidFrameTarget => {
                    LivePageFlipEventStatus::InvalidFrameTarget
                }
                LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch => {
                    LivePageFlipEventStatus::FrameTargetSizeMismatch
                }
                LiveKmsScanoutTargetStatus::PresentationUnavailable => {
                    LivePageFlipEventStatus::PresentationUnavailable
                }
                LiveKmsScanoutTargetStatus::Degraded => LivePageFlipEventStatus::Degraded,
            },
            frame_serial: None,
        }
    }

    pub fn from_commit_outcome(outcome: &PageFlipCommitOutcome) -> Self {
        match outcome {
            PageFlipCommitOutcome::Idle => Self {
                status: LivePageFlipEventStatus::Idle,
                frame_serial: None,
            },
            PageFlipCommitOutcome::WaitingForOutput { .. } => Self {
                status: LivePageFlipEventStatus::WaitingForOutput,
                frame_serial: None,
            },
            PageFlipCommitOutcome::WaitingForTransactionReadiness { .. } => Self {
                status: LivePageFlipEventStatus::WaitingForTransactionReadiness,
                frame_serial: None,
            },
            PageFlipCommitOutcome::Committed { frame_serial, .. } => Self {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(*frame_serial),
            },
            PageFlipCommitOutcome::Rejected { frame_serial, .. } => Self {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(*frame_serial),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePageFlipEventStatus {
    Ready,
    Idle,
    WaitingForOutput,
    WaitingForTransactionReadiness,
    Presented,
    Rejected,
    OutputUnavailable,
    FrameTargetUnavailable,
    InvalidFrameTarget,
    FrameTargetSizeMismatch,
    PresentationUnavailable,
    Degraded,
}

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

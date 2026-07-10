use crate::prelude::*;

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

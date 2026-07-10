#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutSubmitResult<Owner> {
    pub status: LiveRenderedPrimaryPlaneScanoutSubmitStatus,
    pub scanout_target: LiveKmsScanoutTargetStatus,
    pub target: Option<LiveGbmEglFrameTargetStatus>,
    pub export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub submission: Option<LiveRenderedPrimaryPlaneScanoutSubmission<Owner>>,
    pub cleanup: Option<LiveRenderedPrimaryPlaneScanoutCleanup<Owner>>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutSubmitResult<Owner> {
    pub fn runtime_scanout_state(&self) -> RuntimeScanoutState {
        runtime_scanout_state_from_rendered_primary_plane_submit_status(self.status)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderedPrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    ScanoutTargetNotReady,
    FrameTargetUnavailable,
    ScanoutExportFailed,
    PrimaryPlaneSubmitFailed,
}

#[cfg(feature = "libdrm-events")]
pub fn runtime_scanout_state_from_rendered_primary_plane_submit_status(
    status: LiveRenderedPrimaryPlaneScanoutSubmitStatus,
) -> RuntimeScanoutState {
    match status {
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip => {
            RuntimeScanoutState::Submitted
        }
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
        | LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable
        | LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
        | LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed => {
            RuntimeScanoutState::Rejected
        }
    }
}

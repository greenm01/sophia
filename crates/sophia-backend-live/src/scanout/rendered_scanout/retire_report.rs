#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutRetireResult<Owner> {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LiveRenderedPrimaryPlaneScanoutSubmission<Owner>>,
    pub cleanup: Option<LiveRenderedPrimaryPlaneScanoutCleanup<Owner>>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutRetireResult<Owner> {
    pub fn runtime_scanout_state(&self) -> Option<RuntimeScanoutState> {
        runtime_scanout_state_from_rendered_primary_plane_retire_status(self.status)
    }
}

#[cfg(feature = "libdrm-events")]
pub fn runtime_scanout_state_from_rendered_primary_plane_retire_status(
    status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
) -> Option<RuntimeScanoutState> {
    match status {
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip => {
            Some(RuntimeScanoutState::Retired)
        }
        LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip => None,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed => {
            Some(RuntimeScanoutState::Rejected)
        }
    }
}

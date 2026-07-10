use crate::prelude::*;

use super::reduced_status;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
    pub status: LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub runtime_scanout_state: Option<RuntimeScanoutState>,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
    pub cleanup_pending: bool,
}

impl LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
    pub fn reduced_log_line(&self) -> String {
        format!(
            "sophia_runtime_rendered_scanout_retire schema=1 status={:?} destroy={} runtime_scanout_state={} in_flight={} in_flight_ticks={} cleanup_pending={}",
            self.status,
            reduced_status(self.destroy),
            reduced_status(self.runtime_scanout_state),
            self.in_flight,
            self.in_flight_ticks,
            self.cleanup_pending,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus {
    NoSubmission,
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

impl From<LibdrmNativePrimaryPlaneScanoutRetireStatus>
    for LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus
{
    fn from(status: LibdrmNativePrimaryPlaneScanoutRetireStatus) -> Self {
        match status {
            LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip => {
                Self::RetiredAfterPageFlip
            }
            LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip => {
                Self::WaitingForAcceptedPageFlip
            }
            LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed => {
                Self::ResourceRetireFailed
            }
        }
    }
}

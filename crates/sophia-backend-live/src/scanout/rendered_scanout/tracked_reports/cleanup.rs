use crate::prelude::*;

use super::reduced_status;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
    pub status: LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub cleanup_pending: bool,
}

impl LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
    pub fn reduced_log_line(&self) -> String {
        format!(
            "sophia_runtime_rendered_scanout_cleanup schema=1 status={:?} destroy={} cleanup_pending={}",
            self.status,
            reduced_status(self.destroy),
            self.cleanup_pending,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus {
    NoCleanupPending,
    CleanedUp,
    CleanupFailed,
}

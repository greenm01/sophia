#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
    pub status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus,
    pub scanout_target: LiveKmsScanoutTargetStatus,
    pub target: Option<LiveGbmEglFrameTargetStatus>,
    pub export: Option<LiveRendererScanoutBufferExportStatus>,
    pub scanout_buffer: Option<LiveRendererScanoutBufferStatus>,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub commit_submit: Option<LibdrmNativeAtomicCommitSubmitStatus>,
    pub runtime_scanout_state: Option<RuntimeScanoutState>,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
}

#[cfg(feature = "libdrm-events")]
impl LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
    pub fn reduced_log_line(&self) -> String {
        fn status<T: std::fmt::Debug>(status: Option<T>) -> String {
            status
                .map(|status| format!("{status:?}"))
                .unwrap_or_else(|| "none".to_owned())
        }

        let (commit_page_flip_event, commit_nonblocking, commit_allow_modeset, commit_test_only) =
            self.commit_flags
                .map(|flags| {
                    (
                        flags.page_flip_event.to_string(),
                        flags.nonblocking.to_string(),
                        flags.allow_modeset.to_string(),
                        flags.test_only.to_string(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        "none".to_owned(),
                        "none".to_owned(),
                        "none".to_owned(),
                        "none".to_owned(),
                    )
                });

        format!(
            "sophia_runtime_rendered_scanout_submit schema=1 status={:?} scanout_target={:?} target={} export={} scanout_buffer={} properties={} resources={} request={} submit={} request_scope={} commit_page_flip_event={} commit_nonblocking={} commit_allow_modeset={} commit_test_only={} commit_submit={} runtime_scanout_state={} in_flight={} in_flight_ticks={}",
            self.status,
            self.scanout_target,
            status(self.target),
            status(self.export),
            status(self.scanout_buffer),
            status(self.properties),
            status(self.resources),
            status(self.request),
            status(self.submit),
            status(self.request_scope),
            commit_page_flip_event,
            commit_nonblocking,
            commit_allow_modeset,
            commit_test_only,
            status(self.commit_submit),
            status(self.runtime_scanout_state),
            self.in_flight,
            self.in_flight_ticks,
        )
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    ScanoutTargetNotReady,
    FrameTargetUnavailable,
    ScanoutExportFailed,
    PrimaryPlaneSubmitFailed,
    AlreadyInFlight,
    CleanupPending,
}

#[cfg(feature = "libdrm-events")]
impl From<LiveRenderedPrimaryPlaneScanoutSubmitStatus>
    for LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus
{
    fn from(status: LiveRenderedPrimaryPlaneScanoutSubmitStatus) -> Self {
        match status {
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip => {
                Self::SubmittedWaitingForPageFlip
            }
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady => {
                Self::ScanoutTargetNotReady
            }
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable => {
                Self::FrameTargetUnavailable
            }
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed => {
                Self::ScanoutExportFailed
            }
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed => {
                Self::PrimaryPlaneSubmitFailed
            }
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
    pub status: LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub runtime_scanout_state: Option<RuntimeScanoutState>,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
    pub cleanup_pending: bool,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus {
    NoSubmission,
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

#[cfg(feature = "libdrm-events")]
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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
    pub status: LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub cleanup_pending: bool,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus {
    NoCleanupPending,
    CleanedUp,
    CleanupFailed,
}

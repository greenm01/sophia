#[cfg(feature = "libdrm-events")]
use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::any::Any;

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

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutSubmission<Owner> {
    pub(crate) scanout_buffer: Owner,
    pub(crate) primary_plane: LibdrmNativePrimaryPlaneScanoutSubmission,
    pub(crate) submitted_after_page_flip_serial: Option<u64>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutSubmission<Owner> {
    pub fn into_scanout_buffer(self) -> Owner {
        self.scanout_buffer
    }

    pub fn map_scanout_buffer<Next>(
        self,
        map: impl FnOnce(Owner) -> Next,
    ) -> LiveRenderedPrimaryPlaneScanoutSubmission<Next> {
        LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: map(self.scanout_buffer),
            primary_plane: self.primary_plane,
            submitted_after_page_flip_serial: self.submitted_after_page_flip_serial,
        }
    }

    pub(crate) fn with_submitted_after_page_flip_serial(
        mut self,
        submitted_after_page_flip_serial: Option<u64>,
    ) -> Self {
        self.submitted_after_page_flip_serial = submitted_after_page_flip_serial;
        self
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) type BoxedRenderedPrimaryPlaneScanoutSubmission =
    LiveRenderedPrimaryPlaneScanoutSubmission<Box<dyn Any>>;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutCleanup<Owner> {
    pub(crate) scanout_buffer: Owner,
    pub(crate) primary_plane: LibdrmNativePrimaryPlaneResourceCleanup,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutCleanup<Owner> {
    pub fn into_scanout_buffer(self) -> Owner {
        self.scanout_buffer
    }

    pub fn map_scanout_buffer<Next>(
        self,
        map: impl FnOnce(Owner) -> Next,
    ) -> LiveRenderedPrimaryPlaneScanoutCleanup<Next> {
        LiveRenderedPrimaryPlaneScanoutCleanup {
            scanout_buffer: map(self.scanout_buffer),
            primary_plane: self.primary_plane,
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) type BoxedRenderedPrimaryPlaneScanoutCleanup =
    LiveRenderedPrimaryPlaneScanoutCleanup<Box<dyn Any>>;

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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
    pub status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus,
    pub scanout_target: LiveKmsScanoutTargetStatus,
    pub target: Option<LiveGbmEglFrameTargetStatus>,
    pub export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub runtime_scanout_state: Option<RuntimeScanoutState>,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRenderedPrimaryPlaneScanoutBackpressureReport {
    pub status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
    pub threshold_ticks: u64,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderedPrimaryPlaneScanoutBackpressureStatus {
    Idle,
    WaitingForPageFlip,
    StalledWaitingForPageFlip,
}

#[cfg(feature = "libdrm-events")]
impl LiveRenderedPrimaryPlaneScanoutBackpressureReport {
    pub const fn from_in_flight_state(
        in_flight: bool,
        in_flight_ticks: u64,
        threshold_ticks: u64,
    ) -> Self {
        let status = if !in_flight {
            LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle
        } else if threshold_ticks > 0 && in_flight_ticks >= threshold_ticks {
            LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip
        } else {
            LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip
        };

        Self {
            status,
            in_flight,
            in_flight_ticks,
            threshold_ticks,
        }
    }
}

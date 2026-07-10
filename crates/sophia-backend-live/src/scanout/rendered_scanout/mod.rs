use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
use std::{any::Any, collections::VecDeque};

mod exporter;

pub use exporter::*;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutSubmitResult<Owner> {
    pub status: LiveRenderedPrimaryPlaneScanoutSubmitStatus,
    pub scanout_target: LiveKmsScanoutTargetStatus,
    pub target: Option<LiveGbmEglFrameTargetStatus>,
    pub export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub submission: Option<LiveRenderedPrimaryPlaneScanoutSubmission<Owner>>,
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
pub(crate) fn submit_rendered_primary_plane_scanout_from_scanout_target_with<D, E>(
    scanout_target: LiveKmsScanoutTargetStatus,
    target: Option<LiveGbmEglFrameTargetRecord>,
    device: &D,
    exporter: &mut E,
) -> LiveRenderedPrimaryPlaneScanoutSubmitResult<E::Owner>
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
    E: LiveRenderedScanoutBufferExporter,
{
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            commit_flags: None,
            submission: None,
        };
    }

    let Some(target) = target else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable,
            scanout_target,
            target: None,
            export: None,
            submit: None,
            commit_flags: None,
            submission: None,
        };
    };

    let selection = select_native_primary_plane_target(device);
    let scanout_target =
        reduced_scanout_target_status_from_native_selection(scanout_target, target, &selection);
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            target: Some(target.status),
            export: None,
            submit: None,
            commit_flags: None,
            submission: None,
        };
    }

    let export = exporter.export_rendered_scanout_buffer(target);
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            submit: None,
            commit_flags: None,
            submission: None,
        };
    }

    let (Some(descriptor), Some(owner)) = (export.descriptor, export.owner) else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            submit: None,
            commit_flags: None,
            submission: None,
        };
    };

    let mut submit =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            device,
            selection,
            descriptor,
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );
    if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            submit: Some(submit.status),
            commit_flags: submit.commit_flags,
            submission: None,
        };
    }

    let Some(primary_plane) = submit.submission.take() else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            submit: Some(submit.status),
            commit_flags: submit.commit_flags,
            submission: None,
        };
    };

    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        scanout_target,
        target: Some(target.status),
        export: Some(export.status),
        submit: Some(submit.status),
        commit_flags: submit.commit_flags,
        submission: Some(LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: owner,
            primary_plane,
            submitted_after_page_flip_serial: None,
        }),
    }
}

#[cfg(feature = "libdrm-events")]
fn reduced_scanout_target_status_from_native_selection(
    current_status: LiveKmsScanoutTargetStatus,
    target: LiveGbmEglFrameTargetRecord,
    selection: &LibdrmNativePrimaryPlaneSelectionResult,
) -> LiveKmsScanoutTargetStatus {
    if current_status != LiveKmsScanoutTargetStatus::Ready {
        return current_status;
    }

    if target.status != LiveGbmEglFrameTargetStatus::Ready {
        return LiveKmsScanoutTargetStatus::InvalidFrameTarget;
    }

    let Some(selected) = selection.selection else {
        return LiveKmsScanoutTargetStatus::OutputUnavailable;
    };

    if selected.size() != target.size {
        return LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch;
    }

    LiveKmsScanoutTargetStatus::Ready
}

#[cfg(feature = "libdrm-events")]
pub(crate) fn track_rendered_primary_plane_scanout_submit_from_target_with<D, E>(
    scanout_target: LiveKmsScanoutTargetStatus,
    target: Option<LiveGbmEglFrameTargetRecord>,
    rendered_primary_plane_scanout_submission: &mut Option<
        BoxedRenderedPrimaryPlaneScanoutSubmission,
    >,
    rendered_primary_plane_runtime_scanout_state: &mut Option<RuntimeScanoutState>,
    rendered_primary_plane_scanout_in_flight_ticks: &mut u64,
    cleanup_pending: bool,
    submitted_after_page_flip_serial: Option<u64>,
    pending_runtime_scanout_states: Option<&mut VecDeque<RuntimeScanoutState>>,
    device: &D,
    exporter: &mut E,
) -> LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
    E: LiveRenderedScanoutBufferExporter,
    E::Owner: 'static,
{
    if rendered_primary_plane_scanout_submission.is_some() {
        return LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight,
            scanout_target,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            commit_flags: None,
            runtime_scanout_state: Some(RuntimeScanoutState::Deferred),
            in_flight: true,
            in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
        };
    }

    if cleanup_pending {
        return LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending,
            scanout_target,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            commit_flags: None,
            runtime_scanout_state: Some(RuntimeScanoutState::Deferred),
            in_flight: false,
            in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
        };
    }

    let mut result = submit_rendered_primary_plane_scanout_from_scanout_target_with(
        scanout_target,
        target,
        device,
        exporter,
    );
    let runtime_scanout_state = Some(result.runtime_scanout_state());

    if let Some(submission) = result.submission.take() {
        *rendered_primary_plane_scanout_submission = Some(
            submission
                .with_submitted_after_page_flip_serial(submitted_after_page_flip_serial)
                .map_scanout_buffer(|owner| Box::new(owner) as Box<dyn Any>),
        );
    }
    *rendered_primary_plane_scanout_in_flight_ticks = 0;
    *rendered_primary_plane_runtime_scanout_state = runtime_scanout_state;
    if runtime_scanout_state == Some(RuntimeScanoutState::Rejected) {
        if let Some(pending_runtime_scanout_states) = pending_runtime_scanout_states {
            pending_runtime_scanout_states.push_back(RuntimeScanoutState::Rejected);
        }
    }

    LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
        status: result.status.into(),
        scanout_target: result.scanout_target,
        target: result.target,
        export: result.export,
        submit: result.submit,
        commit_flags: result.commit_flags,
        runtime_scanout_state,
        in_flight: rendered_primary_plane_scanout_submission.is_some(),
        in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) struct LiveRenderedPrimaryPlaneRuntimeAdapter<'a, D, E> {
    pub(crate) inner: LiveRuntimeDriverAdapter,
    pub(crate) scanout_target: LiveKmsScanoutTargetStatus,
    pub(crate) target: Option<LiveGbmEglFrameTargetRecord>,
    pub(crate) rendered_primary_plane_scanout_submission:
        &'a mut Option<BoxedRenderedPrimaryPlaneScanoutSubmission>,
    pub(crate) rendered_primary_plane_runtime_scanout_state: &'a mut Option<RuntimeScanoutState>,
    pub(crate) rendered_primary_plane_scanout_in_flight_ticks: &'a mut u64,
    pub(crate) cleanup_pending: bool,
    pub(crate) submitted_after_page_flip_serial: Option<u64>,
    pub(crate) device: &'a D,
    pub(crate) exporter: &'a mut E,
    pub(crate) submit_report: &'a mut Option<LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport>,
}

#[cfg(feature = "libdrm-events")]
impl<D, E> RuntimeDriverAdapter for LiveRenderedPrimaryPlaneRuntimeAdapter<'_, D, E>
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
    E: LiveRenderedScanoutBufferExporter,
    E::Owner: 'static,
{
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, sophia_engine::EngineError> {
        self.inner.poll_x_events()
    }

    fn poll_x_observations(
        &mut self,
    ) -> Result<Vec<SessionRuntimeObservation>, sophia_engine::EngineError> {
        self.inner.poll_x_observations()
    }

    fn request_wm_layout(
        &mut self,
    ) -> Result<SessionRuntimeObservation, sophia_engine::EngineError> {
        self.inner.request_wm_layout()
    }

    fn schedule_frame(
        &mut self,
        frame_serial: u64,
    ) -> Result<SessionRuntimeObservation, sophia_engine::EngineError> {
        self.inner.schedule_frame(frame_serial)
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, sophia_engine::EngineError> {
        self.inner
            .render_frame(engine, output, frame_serial, last_committed)
    }

    fn submit_scanout(
        &mut self,
        frame_serial: u64,
    ) -> Result<SessionRuntimeObservation, sophia_engine::EngineError> {
        let report = track_rendered_primary_plane_scanout_submit_from_target_with(
            self.scanout_target,
            self.target,
            self.rendered_primary_plane_scanout_submission,
            self.rendered_primary_plane_runtime_scanout_state,
            self.rendered_primary_plane_scanout_in_flight_ticks,
            self.cleanup_pending,
            self.submitted_after_page_flip_serial,
            None,
            self.device,
            self.exporter,
        );
        let state = report
            .runtime_scanout_state
            .unwrap_or(RuntimeScanoutState::Rejected);
        *self.submit_report = Some(report);

        Ok(SessionRuntimeObservation::ScanoutStateChanged {
            state,
            frame_serial: Some(frame_serial),
        })
    }

    fn drain_portal_commands(
        &mut self,
    ) -> Result<SessionRuntimeObservation, sophia_engine::EngineError> {
        self.inner.drain_portal_commands()
    }

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, sophia_engine::EngineError> {
        self.inner.present_chrome()
    }
}

#[cfg(feature = "libdrm-events")]
pub fn retire_rendered_primary_plane_scanout_after_page_flip<D, Owner>(
    device: &D,
    submission: LiveRenderedPrimaryPlaneScanoutSubmission<Owner>,
    callback: &LivePageFlipCallbackReport,
) -> LiveRenderedPrimaryPlaneScanoutRetireResult<Owner>
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    let waiting_for_newer_page_flip = callback.decision == LivePageFlipCallbackDecision::Accepted
        && submission
            .submitted_after_page_flip_serial
            .is_some_and(|baseline| match callback.event.frame_serial {
                Some(serial) => serial <= baseline,
                None => true,
            });
    if waiting_for_newer_page_flip {
        return LiveRenderedPrimaryPlaneScanoutRetireResult {
            status: LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
            destroy: None,
            submission: Some(submission),
            cleanup: None,
        };
    }

    let mut owner = Some(submission.scanout_buffer);
    let submitted_after_page_flip_serial = submission.submitted_after_page_flip_serial;
    let retired = retire_native_primary_plane_scanout_after_page_flip(
        device,
        submission.primary_plane,
        callback,
    );
    let submission =
        retired
            .submission
            .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutSubmission {
                scanout_buffer: owner
                    .take()
                    .expect("waiting retirement should retain rendered owner"),
                primary_plane,
                submitted_after_page_flip_serial,
            });
    let cleanup = retired
        .cleanup
        .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutCleanup {
            scanout_buffer: owner
                .take()
                .expect("cleanup failure should retain rendered owner"),
            primary_plane,
        });

    LiveRenderedPrimaryPlaneScanoutRetireResult {
        status: retired.status,
        destroy: retired.destroy,
        submission,
        cleanup,
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutCleanupResult<Owner> {
    pub destroy: LibdrmNativePrimaryPlaneResourceDestroyStatus,
    pub cleanup: Option<LiveRenderedPrimaryPlaneScanoutCleanup<Owner>>,
}

#[cfg(feature = "libdrm-events")]
pub fn retry_rendered_primary_plane_scanout_cleanup<D, Owner>(
    device: &D,
    cleanup: LiveRenderedPrimaryPlaneScanoutCleanup<Owner>,
) -> LiveRenderedPrimaryPlaneScanoutCleanupResult<Owner>
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    let owner = cleanup.scanout_buffer;
    let report = cleanup.primary_plane.retry(device);
    let cleanup = report
        .cleanup
        .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutCleanup {
            scanout_buffer: owner,
            primary_plane,
        });

    LiveRenderedPrimaryPlaneScanoutCleanupResult {
        destroy: report.status,
        cleanup,
    }
}

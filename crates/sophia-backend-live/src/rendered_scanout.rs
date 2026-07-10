use super::*;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use std::io;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use std::os::fd::AsFd;
#[cfg(feature = "libdrm-events")]
use std::{any::Any, collections::VecDeque};

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExporter};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{
    NativeGbmRenderedScanoutContext, NativeGbmRenderedScanoutContextStatus,
};

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedScanoutBufferExport<Owner> {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub descriptor: Option<LiveRendererScanoutBufferDescriptor>,
    pub owner: Option<Owner>,
}

#[cfg(feature = "libdrm-events")]
pub trait LiveRenderedScanoutBufferExporter {
    type Owner;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner>;
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub struct NativeGbmRenderedScanoutBufferExporter<T> {
    device: Option<io::Result<T>>,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<T> NativeGbmRenderedScanoutBufferExporter<T> {
    pub fn new(device: io::Result<T>) -> Self {
        Self {
            device: Some(device),
        }
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<T> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferExporter<T>
where
    T: AsFd,
{
    type Owner = NativeGbmOwnedScanoutBuffer;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        let Some(device) = self.device.take() else {
            return LiveRenderedScanoutBufferExport {
                status: LiveRendererScanoutBufferExportStatus::Unavailable,
                descriptor: None,
                owner: None,
            };
        };

        let report = NativeGbmScanoutBufferExporter::export_rendered_owned_scanout_buffer_from_backend_device_result(
            device, target,
        );
        let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());

        LiveRenderedScanoutBufferExport {
            status: report.status,
            descriptor,
            owner: report.buffer,
        }
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub struct NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    discovery: R,
    context: Option<NativeGbmRenderedScanoutContext<R::Device>>,
    context_status: Option<NativeGbmRenderedScanoutContextStatus>,
    context_open_attempts: usize,
    export_attempts: usize,
    last_target: Option<LiveGbmEglFrameTargetRecord>,
    last_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    last_export_status: Option<LiveRendererScanoutBufferExportStatus>,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    pub fn new(discovery: R) -> Self {
        Self {
            discovery,
            context: None,
            context_status: None,
            context_open_attempts: 0,
            export_attempts: 0,
            last_target: None,
            last_target_lifecycle: None,
            last_export_status: None,
        }
    }

    pub const fn context_open_attempts(&self) -> usize {
        self.context_open_attempts
    }

    pub const fn export_attempts(&self) -> usize {
        self.export_attempts
    }

    pub const fn last_export_status(&self) -> Option<LiveRendererScanoutBufferExportStatus> {
        self.last_export_status
    }

    pub const fn last_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.last_target
    }

    pub const fn last_target_lifecycle(&self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.last_target_lifecycle
    }

    pub const fn context_status(&self) -> Option<NativeGbmRenderedScanoutContextStatus> {
        self.context_status
    }

    pub const fn context_ready(&self) -> bool {
        self.context.is_some()
    }

    pub fn discovery(&self) -> &R {
        &self.discovery
    }

    pub fn discovery_mut(&mut self) -> &mut R {
        &mut self.discovery
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    type Owner = NativeGbmOwnedScanoutBuffer;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        let target_lifecycle =
            LiveGbmEglFrameTargetLifecycleReport::from_size_update(self.last_target, target);
        self.last_target = Some(target);
        self.last_target_lifecycle = Some(target_lifecycle);

        if target.status != LiveGbmEglFrameTargetStatus::Ready {
            self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::InvalidTarget);
            return LiveRenderedScanoutBufferExport {
                status: LiveRendererScanoutBufferExportStatus::InvalidTarget,
                descriptor: None,
                owner: None,
            };
        }

        if self.context.is_none() {
            self.context_open_attempts = self.context_open_attempts.saturating_add(1);
            let report = NativeGbmRenderedScanoutContext::from_backend_device_result(
                self.discovery.open_render_device(),
            );
            self.context_status = Some(report.status);
            self.context = report.context;
        }

        let Some(context) = &self.context else {
            let status = match self.context_status {
                Some(NativeGbmRenderedScanoutContextStatus::Degraded) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Ready) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Unavailable) | None => {
                    LiveRendererScanoutBufferExportStatus::Unavailable
                }
            };
            self.last_export_status = Some(status);
            return LiveRenderedScanoutBufferExport {
                status,
                descriptor: None,
                owner: None,
            };
        };

        let report = context.export_rendered_owned_scanout_buffer(target);
        let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());
        self.last_export_status = Some(report.status);
        LiveRenderedScanoutBufferExport {
            status: report.status,
            descriptor,
            owner: report.buffer,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutSubmitResult<Owner> {
    pub status: LiveRenderedPrimaryPlaneScanoutSubmitStatus,
    pub target: Option<LiveGbmEglFrameTargetStatus>,
    pub export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
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
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable
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
    pub target: Option<LiveGbmEglFrameTargetStatus>,
    pub export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub runtime_scanout_state: Option<RuntimeScanoutState>,
    pub in_flight: bool,
    pub in_flight_ticks: u64,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
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
pub(crate) fn submit_rendered_primary_plane_scanout_from_target_with<D, E>(
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
    let Some(target) = target else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable,
            target: None,
            export: None,
            submit: None,
            submission: None,
        };
    };

    let export = exporter.export_rendered_scanout_buffer(target);
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            target: Some(target.status),
            export: Some(export.status),
            submit: None,
            submission: None,
        };
    }

    let (Some(descriptor), Some(owner)) = (export.descriptor, export.owner) else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            target: Some(target.status),
            export: Some(export.status),
            submit: None,
            submission: None,
        };
    };

    let mut submit =
        submit_native_primary_plane_scanout_from_renderer_descriptor(device, descriptor);
    if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            target: Some(target.status),
            export: Some(export.status),
            submit: Some(submit.status),
            submission: None,
        };
    }

    let Some(primary_plane) = submit.submission.take() else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            target: Some(target.status),
            export: Some(export.status),
            submit: Some(submit.status),
            submission: None,
        };
    };

    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        target: Some(target.status),
        export: Some(export.status),
        submit: Some(submit.status),
        submission: Some(LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: owner,
            primary_plane,
            submitted_after_page_flip_serial: None,
        }),
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) fn track_rendered_primary_plane_scanout_submit_from_target_with<D, E>(
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
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            runtime_scanout_state: Some(RuntimeScanoutState::Deferred),
            in_flight: true,
            in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
        };
    }

    if cleanup_pending {
        return LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            runtime_scanout_state: Some(RuntimeScanoutState::Deferred),
            in_flight: false,
            in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
        };
    }

    let mut result =
        submit_rendered_primary_plane_scanout_from_target_with(target, device, exporter);
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
        target: result.target,
        export: result.export,
        submit: result.submit,
        runtime_scanout_state,
        in_flight: rendered_primary_plane_scanout_submission.is_some(),
        in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) struct LiveRenderedPrimaryPlaneRuntimeAdapter<'a, D, E> {
    pub(crate) inner: LiveRuntimeDriverAdapter,
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

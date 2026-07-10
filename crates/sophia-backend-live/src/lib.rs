//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

use std::collections::VecDeque;
#[cfg(any(
    feature = "gbm-probe",
    feature = "libdrm-events",
    feature = "libinput-events"
))]
use std::io;
#[cfg(feature = "gbm-probe")]
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};

pub use sophia_engine::{
    BufferImportPath, CompositorBackendAssemblyError, CompositorBackendTickInput,
    CompositorBackendTickReport, DrmKmsOutputRegistry, HeadlessCompositorBackendAssembly,
    HeadlessOutput, LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventIngest,
    LibinputEventSource, LibinputPhysicalInputAdapter, LibinputPollReport,
    LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus,
    NonBlockingInputPoller, PageFlipCommitOutcome, QueuedInputPoller, RendererSelection,
};
use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};
pub use sophia_protocol::{BufferSource, DeviceId, InputEventPacket, OutputId, SeatId, Size};
#[cfg(feature = "gbm-probe")]
use sophia_renderer_live::GbmCapabilityProbeStatus;
#[cfg(feature = "egl-probe")]
use sophia_renderer_live::{
    EglCapabilityProbeStatus, FakeEglCapabilityProbe, NativeEglCapabilityProbe, NativeEglDrawSmoke,
};
#[cfg(feature = "egl-probe")]
pub use sophia_renderer_live::{EglContextProbeStatus, EglPlatformStatus};
#[cfg(feature = "egl-probe")]
pub use sophia_renderer_live::{EglDrawSmokeReport, EglDrawSmokeStatus};
pub use sophia_renderer_live::{
    FakeGbmEglFrameTargetAllocator, LiveGbmEglFrameTargetAllocationReport,
    LiveGbmEglFrameTargetAllocationRequest, LiveGbmEglFrameTargetAllocationStatus,
    LiveGbmEglFrameTargetAllocator, LiveGbmEglFrameTargetLifecycleReport,
    LiveGbmEglFrameTargetLifecycleStatus, LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus,
    LiveRendererImportBoundary, LiveRendererImportDecision, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportRejection, LiveRendererImportStartupStatus,
    LiveRendererPresentationReport, LiveRendererPresentationStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation,
};
#[cfg(feature = "gbm-probe")]
pub use sophia_renderer_live::{GbmCapabilityProbeReport, NativeGbmCapabilityProbe};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
use sophia_renderer_live::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglFrameTargetAllocator,
    NativeGbmBackedEglPlatformProbe, NativeGbmBackedEglPresentationSmoke,
};

pub const LIVE_PAGE_FLIP_CALLBACK_CHANNEL_CAPACITY: usize = 128;
pub const SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveHardwareValidationGateReport {
    pub target: LiveHardwareValidationTarget,
    pub status: LiveHardwareValidationGateStatus,
}

impl LiveHardwareValidationGateReport {
    pub const fn from_env_presence(target: LiveHardwareValidationTarget, present: bool) -> Self {
        Self {
            target,
            status: if present {
                LiveHardwareValidationGateStatus::Requested
            } else {
                LiveHardwareValidationGateStatus::SkippedOptInRequired
            },
        }
    }

    pub const fn is_requested(self) -> bool {
        matches!(self.status, LiveHardwareValidationGateStatus::Requested)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHardwareValidationTarget {
    LibdrmEvents,
    LibinputEvents,
}

impl LiveHardwareValidationTarget {
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::LibdrmEvents => SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE,
            Self::LibinputEvents => SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHardwareValidationGateStatus {
    SkippedOptInRequired,
    Requested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveHardwareValidationSmokeReport {
    pub target: LiveHardwareValidationTarget,
    pub status: LiveHardwareValidationSmokeStatus,
}

impl LiveHardwareValidationSmokeReport {
    pub const fn fail_closed_from_gate(gate: LiveHardwareValidationGateReport) -> Self {
        Self {
            target: gate.target,
            status: if gate.is_requested() {
                LiveHardwareValidationSmokeStatus::BackendUnavailable
            } else {
                LiveHardwareValidationSmokeStatus::SkippedOptInRequired
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHardwareValidationSmokeStatus {
    SkippedOptInRequired,
    BackendUnavailable,
    Passed,
    Failed,
}

pub fn real_libdrm_events_validation_gate() -> LiveHardwareValidationGateReport {
    let target = LiveHardwareValidationTarget::LibdrmEvents;
    LiveHardwareValidationGateReport::from_env_presence(
        target,
        std::env::var_os(target.env_var()).is_some(),
    )
}

pub fn real_libinput_events_validation_gate() -> LiveHardwareValidationGateReport {
    let target = LiveHardwareValidationTarget::LibinputEvents;
    LiveHardwareValidationGateReport::from_env_presence(
        target,
        std::env::var_os(target.env_var()).is_some(),
    )
}

pub fn real_libdrm_events_validation_smoke_report() -> LiveHardwareValidationSmokeReport {
    LiveHardwareValidationSmokeReport::fail_closed_from_gate(real_libdrm_events_validation_gate())
}

pub fn real_libinput_events_validation_smoke_report() -> LiveHardwareValidationSmokeReport {
    LiveHardwareValidationSmokeReport::fail_closed_from_gate(real_libinput_events_validation_gate())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyKind {
    LibDrm,
    LibInput,
    Gbm,
    Egl,
    DmaBuf,
    Wgpu,
    MitShm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyUse {
    Discovery,
    RuntimePolling,
    RendererImport,
    SharedMemoryImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyDecision {
    Allowed,
    Deferred { required_boundary: &'static str },
}

impl LiveBackendDependencyDecision {
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

pub fn live_backend_dependency_decision(
    kind: LiveBackendDependencyKind,
    use_case: LiveBackendDependencyUse,
) -> LiveBackendDependencyDecision {
    use LiveBackendDependencyDecision::{Allowed, Deferred};
    use LiveBackendDependencyKind::{DmaBuf, Egl, Gbm, LibDrm, LibInput, MitShm, Wgpu};
    use LiveBackendDependencyUse::{Discovery, RendererImport, RuntimePolling, SharedMemoryImport};

    match (kind, use_case) {
        (LibDrm | LibInput, Discovery | RuntimePolling) => Allowed,
        (MitShm, _) | (_, SharedMemoryImport) => Deferred {
            required_boundary: "bounded shared-memory import boundary",
        },
        (Wgpu, _) => Deferred {
            required_boundary: "validated GBM/EGL startup, drawing, and presentation seams",
        },
        (Gbm | Egl | DmaBuf, _) | (_, RendererImport) => Deferred {
            required_boundary: "live renderer import boundary",
        },
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputNativeEventAdapterReport {
    pub status: LibinputNativeEventAdapterStatus,
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputNativeEventAdapterStatus {
    SkeletonReady,
}

#[cfg(feature = "libinput-events")]
pub const fn native_libinput_event_adapter_report() -> LibinputNativeEventAdapterReport {
    LibinputNativeEventAdapterReport {
        status: LibinputNativeEventAdapterStatus::SkeletonReady,
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Debug, PartialEq)]
pub struct LibinputNativeEventReadResult {
    pub report: LibinputNativeEventReadReport,
    pub events: Vec<InputEventPacket>,
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputNativeEventReadReport {
    pub status: LibinputNativeEventReadStatus,
    pub events_read: usize,
    pub queued_remaining: usize,
}

#[cfg(feature = "libinput-events")]
impl LibinputNativeEventReadReport {
    pub const fn idle() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::Idle,
            events_read: 0,
            queued_remaining: 0,
        }
    }

    pub const fn would_block() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::WouldBlock,
            events_read: 0,
            queued_remaining: 0,
        }
    }

    pub const fn events_read(events_read: usize, queued_remaining: usize) -> Self {
        Self {
            status: if events_read == 0 {
                LibinputNativeEventReadStatus::Idle
            } else {
                LibinputNativeEventReadStatus::EventsRead
            },
            events_read,
            queued_remaining,
        }
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::ReadFailed,
            events_read: 0,
            queued_remaining: 0,
        }
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputNativeEventReadStatus {
    Idle,
    WouldBlock,
    EventsRead,
    ReadFailed,
}

#[cfg(feature = "libinput-events")]
pub trait LiveLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult;
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLibinputEventPoller<R> {
    reader: R,
    max_read_per_poll: usize,
    last_read: LibinputNativeEventReadReport,
}

#[cfg(feature = "libinput-events")]
impl<R> NativeLibinputEventPoller<R> {
    pub fn new(reader: R, max_read_per_poll: usize) -> Self {
        Self {
            reader,
            max_read_per_poll,
            last_read: LibinputNativeEventReadReport::idle(),
        }
    }

    pub const fn last_read_report(&self) -> LibinputNativeEventReadReport {
        self.last_read
    }

    pub const fn max_read_per_poll(&self) -> usize {
        self.max_read_per_poll
    }

    pub fn reader(&self) -> &R {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut R {
        &mut self.reader
    }
}

#[cfg(feature = "libinput-events")]
impl<R> NonBlockingInputPoller for NativeLibinputEventPoller<R>
where
    R: LiveLibinputEventReader,
{
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>> {
        let result = self.reader.read_ready_input_events(self.max_read_per_poll);
        self.last_read = result.report;
        if result.report.status == LibinputNativeEventReadStatus::ReadFailed {
            return Err(io::Error::other("reduced native libinput read failed"));
        }
        Ok(result.events)
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FakeLiveLibinputEventReader {
    queued: VecDeque<InputEventPacket>,
    fail_next_read: bool,
}

#[cfg(feature = "libinput-events")]
impl FakeLiveLibinputEventReader {
    pub fn new(events: impl IntoIterator<Item = InputEventPacket>) -> Self {
        Self {
            queued: events.into_iter().collect(),
            fail_next_read: false,
        }
    }

    pub fn fail_next_read(&mut self) {
        self.fail_next_read = true;
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}

#[cfg(feature = "libinput-events")]
impl LiveLibinputEventReader for FakeLiveLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult {
        if self.fail_next_read {
            self.fail_next_read = false;
            return LibinputNativeEventReadResult {
                report: LibinputNativeEventReadReport::read_failed(),
                events: Vec::new(),
            };
        }

        let mut events = Vec::new();
        for _ in 0..max_read {
            let Some(event) = self.queued.pop_front() else {
                break;
            };
            events.push(event);
        }

        LibinputNativeEventReadResult {
            report: LibinputNativeEventReadReport::events_read(events.len(), self.queued.len()),
            events,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveBackendConfig {
    pub drm_sysfs_root: PathBuf,
    pub input_devices: Vec<LibinputDeviceDescriptor>,
    pub renderer_import: LiveRendererImportBoundary,
    pub renderer_preference: LiveRendererPreference,
}

impl LiveBackendConfig {
    pub fn new(drm_sysfs_root: impl Into<PathBuf>) -> Self {
        Self {
            drm_sysfs_root: drm_sysfs_root.into(),
            input_devices: Vec::new(),
            renderer_import: LiveRendererImportBoundary::cpu_only(),
            renderer_preference: LiveRendererPreference::default(),
        }
    }

    pub fn with_input_device(mut self, device: LibinputDeviceDescriptor) -> Self {
        self.input_devices.push(device);
        self
    }

    pub fn with_renderer_import_boundary(
        mut self,
        renderer_import: LiveRendererImportBoundary,
    ) -> Self {
        self.renderer_import = renderer_import;
        self
    }

    pub fn with_renderer_preference(mut self, renderer_preference: LiveRendererPreference) -> Self {
        self.renderer_preference = renderer_preference;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveRendererPreference {
    #[default]
    GpuPreferred,
    CpuOnly,
    GpuRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveScanoutReadinessReport {
    pub status: LiveScanoutReadinessStatus,
}

impl LiveScanoutReadinessReport {
    fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        Self::from_output_and_presentation(backend.selected_output().is_some(), presentation)
    }

    fn from_output_and_presentation(
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
    fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        Self::from_output_target_and_presentation(
            backend.selected_output().map(|output| output.size),
            backend.selected_gbm_egl_frame_target(),
            presentation,
        )
    }

    fn from_output_target_and_presentation(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallback {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallbackReport {
    pub decision: LivePageFlipCallbackDecision,
    pub event: LivePageFlipEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePageFlipCallbackDecision {
    Accepted,
    RejectedUnexpectedOutput,
    RejectedStaleFrameSerial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallbackIntake {
    expected_output: OutputId,
    last_frame_serial: Option<u64>,
}

impl LivePageFlipCallbackIntake {
    pub const fn new(expected_output: OutputId) -> Self {
        Self {
            expected_output,
            last_frame_serial: None,
        }
    }

    pub const fn last_frame_serial(&self) -> Option<u64> {
        self.last_frame_serial
    }

    pub fn observe(&mut self, callback: LivePageFlipCallback) -> LivePageFlipCallbackReport {
        if callback.output != self.expected_output {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedUnexpectedOutput,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::WaitingForOutput,
                    frame_serial: None,
                },
            };
        }

        if self
            .last_frame_serial
            .is_some_and(|last_frame_serial| callback.frame_serial <= last_frame_serial)
        {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Rejected,
                    frame_serial: Some(callback.frame_serial),
                },
            };
        }

        self.last_frame_serial = Some(callback.frame_serial);
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(callback.frame_serial),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LivePageFlipCallbackQueueReport {
    pub drained: usize,
    pub accepted: usize,
    pub rejected_unexpected_output: usize,
    pub rejected_stale_frame_serial: usize,
    pub disconnected: bool,
    pub max_reached: bool,
}

impl LivePageFlipCallbackQueueReport {
    fn record_decision(&mut self, decision: LivePageFlipCallbackDecision) {
        match decision {
            LivePageFlipCallbackDecision::Accepted => {
                self.accepted = self.accepted.saturating_add(1);
            }
            LivePageFlipCallbackDecision::RejectedUnexpectedOutput => {
                self.rejected_unexpected_output = self.rejected_unexpected_output.saturating_add(1);
            }
            LivePageFlipCallbackDecision::RejectedStaleFrameSerial => {
                self.rejected_stale_frame_serial =
                    self.rejected_stale_frame_serial.saturating_add(1);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLibdrmPollerDiagnostics {
    pub status: LiveLibdrmPollerDiagnosticsStatus,
    pub route_count: usize,
    pub pending_callbacks: usize,
    pub decoded_callbacks: usize,
    pub rejected_callbacks: usize,
}

impl LiveLibdrmPollerDiagnostics {
    pub const fn not_configured() -> Self {
        Self {
            status: LiveLibdrmPollerDiagnosticsStatus::NotConfigured,
            route_count: 0,
            pending_callbacks: 0,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }
}

impl Default for LiveLibdrmPollerDiagnostics {
    fn default() -> Self {
        Self::not_configured()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveLibdrmPollerDiagnosticsStatus {
    NotConfigured,
    Idle,
    WouldBlock,
    CallbackDecoded,
    CallbackRejected,
    ReadFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLibdrmPollerStartupReport {
    pub status: LiveLibdrmPollerStartupStatus,
    pub route_count: usize,
}

impl LiveLibdrmPollerStartupReport {
    pub const fn not_configured() -> Self {
        Self {
            status: LiveLibdrmPollerStartupStatus::NotConfigured,
            route_count: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveLibdrmPollerStartupStatus {
    NotConfigured,
    Ready,
    NoOutputs,
    BackendNotReady,
}

pub struct LivePageFlipCallbackQueue {
    receiver: Receiver<LivePageFlipCallback>,
    max_drain_per_tick: usize,
}

impl LivePageFlipCallbackQueue {
    pub fn new(receiver: Receiver<LivePageFlipCallback>, max_drain_per_tick: usize) -> Self {
        Self {
            receiver,
            max_drain_per_tick,
        }
    }

    fn drain_ready(
        &self,
        intake: &mut LivePageFlipCallbackIntake,
        page_flip_event: &mut LivePageFlipEvent,
    ) -> LivePageFlipCallbackQueueReport {
        let mut report = LivePageFlipCallbackQueueReport::default();

        for _ in 0..self.max_drain_per_tick {
            match self.receiver.try_recv() {
                Ok(callback) => {
                    let callback_report = intake.observe(callback);
                    *page_flip_event = callback_report.event;
                    report.drained = report.drained.saturating_add(1);
                    report.record_decision(callback_report.decision);
                }
                Err(TryRecvError::Empty) => return report,
                Err(TryRecvError::Disconnected) => {
                    report.disconnected = true;
                    return report;
                }
            }
        }

        report.max_reached = true;
        report
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LivePageFlipCallbackSourceReport {
    pub emitted: usize,
    pub queued_remaining: usize,
    pub backpressure: bool,
    pub disconnected: bool,
    pub max_reached: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakePageFlipCallbackSource {
    queued: VecDeque<LivePageFlipCallback>,
}

impl FakePageFlipCallbackSource {
    pub fn new(callbacks: impl IntoIterator<Item = LivePageFlipCallback>) -> Self {
        Self {
            queued: callbacks.into_iter().collect(),
        }
    }

    pub fn push(&mut self, callback: LivePageFlipCallback) {
        self.queued.push_back(callback);
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn emit_ready(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LivePageFlipCallbackSourceReport {
        let mut report = LivePageFlipCallbackSourceReport::default();

        for _ in 0..max_emit {
            let Some(callback) = self.queued.pop_front() else {
                report.queued_remaining = self.queued.len();
                return report;
            };

            match sender.try_send(callback) {
                Ok(()) => {
                    report.emitted = report.emitted.saturating_add(1);
                }
                Err(TrySendError::Full(callback)) => {
                    self.queued.push_front(callback);
                    report.backpressure = true;
                    report.queued_remaining = self.queued.len();
                    return report;
                }
                Err(TrySendError::Disconnected(callback)) => {
                    self.queued.push_front(callback);
                    report.disconnected = true;
                    report.queued_remaining = self.queued.len();
                    return report;
                }
            }
        }

        report.queued_remaining = self.queued.len();
        report.max_reached = !self.queued.is_empty();
        report
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmDependencyAdmissionReport {
    pub status: LibdrmDependencyAdmissionStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmDependencyAdmissionStatus {
    TypedPageFlipEventAvailable,
}

#[cfg(feature = "libdrm-events")]
pub fn libdrm_dependency_admission_report() -> LibdrmDependencyAdmissionReport {
    native_drm_admission::dependency_admission_report()
}

#[cfg(feature = "libdrm-events")]
pub fn native_libdrm_event_adapter_report() -> LibdrmNativeEventAdapterReport {
    native_libdrm_events::adapter_report()
}

#[cfg(feature = "libdrm-events")]
pub fn native_libdrm_event_adapter_report_for_authority(
    authority: LibdrmBackendFdAuthority,
) -> LibdrmNativeEventAdapterReport {
    native_libdrm_events::adapter_report_for_authority(authority)
}

#[cfg(feature = "libdrm-events")]
pub fn libdrm_fd_authority_report(
    authority: LibdrmBackendFdAuthority,
) -> LibdrmBackendFdAuthorityReport {
    native_libdrm_events::fd_authority_report(authority)
}

#[cfg(feature = "libdrm-events")]
mod native_drm_admission {
    use super::{LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus};

    pub(super) fn dependency_admission_report() -> LibdrmDependencyAdmissionReport {
        let _ = core::mem::size_of::<drm::control::PageFlipEvent>();
        LibdrmDependencyAdmissionReport {
            status: LibdrmDependencyAdmissionStatus::TypedPageFlipEventAvailable,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeEventAdapterReport {
    pub status: LibdrmNativeEventAdapterStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeEventAdapterStatus {
    SkeletonReady,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipSource {
    _private: (),
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipSource {
    pub fn from_authority(authority: LibdrmBackendFdAuthority) -> Self {
        native_libdrm_events::page_flip_source_from_authority(authority)
    }

    pub const fn report(&self) -> LibdrmNativePageFlipSourceReport {
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipSourceReport {
    pub status: LibdrmNativePageFlipSourceStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePageFlipSourceStatus {
    ConstructedWithoutPolling,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeOutputSlot {
    raw: u16,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeOutputSlot {
    pub const fn new(raw: u16) -> Option<Self> {
        if raw == 0 {
            return None;
        }

        Some(Self { raw })
    }

    pub const fn raw(self) -> u16 {
        self.raw
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeOutputRoute {
    pub slot: LibdrmNativeOutputSlot,
    pub output: OutputId,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeCrtcRoute {
    crtc: drm::control::crtc::Handle,
    slot: LibdrmNativeOutputSlot,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeCrtcRoute {
    pub const fn new(crtc: drm::control::crtc::Handle, slot: LibdrmNativeOutputSlot) -> Self {
        Self { crtc, slot }
    }

    const fn slot(self) -> LibdrmNativeOutputSlot {
        self.slot
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipCallback {
    pub output_slot: LibdrmNativeOutputSlot,
    pub frame_serial: u64,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipCallback {
    pub const fn new(output_slot: LibdrmNativeOutputSlot, frame_serial: u64) -> Self {
        Self {
            output_slot,
            frame_serial,
        }
    }

    pub fn decode(self, routes: &[LibdrmNativeOutputRoute]) -> LibdrmNativePageFlipDecodeReport {
        if self.frame_serial == 0 {
            return LibdrmNativePageFlipDecodeReport {
                status: LibdrmNativePageFlipDecodeStatus::InvalidFrameSerial,
                callback: None,
            };
        }

        let Some(route) = routes
            .iter()
            .find(|route| route.slot == self.output_slot)
            .copied()
        else {
            return LibdrmNativePageFlipDecodeReport {
                status: LibdrmNativePageFlipDecodeStatus::UnknownOutputSlot,
                callback: None,
            };
        };

        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::Decoded,
            callback: Some(LivePageFlipCallback {
                output: route.output,
                frame_serial: self.frame_serial,
            }),
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub fn reduce_native_page_flip_event(
    event: &drm::control::PageFlipEvent,
    routes: &[LibdrmNativeCrtcRoute],
) -> Option<LibdrmNativePageFlipCallback> {
    let route = routes.iter().find(|route| route.crtc == event.crtc)?;
    Some(LibdrmNativePageFlipCallback::new(
        route.slot(),
        u64::from(event.frame),
    ))
}

#[cfg(feature = "libdrm-events")]
pub fn decode_native_page_flip_batch(
    callbacks: &[LibdrmNativePageFlipCallback],
    routes: &[LibdrmNativeOutputRoute],
    sender: &SyncSender<LivePageFlipCallback>,
    max_decode: usize,
) -> LibdrmNativePageFlipBatchReport {
    let mut source_report = LivePageFlipCallbackSourceReport::default();
    let mut decoded_callbacks = 0usize;
    let mut rejected_callbacks = 0usize;
    let mut stopped_at = None;

    for (index, native) in callbacks.iter().take(max_decode).copied().enumerate() {
        let decode = native.decode(routes);
        let Some(callback) = decode.callback else {
            rejected_callbacks = rejected_callbacks.saturating_add(1);
            continue;
        };
        decoded_callbacks = decoded_callbacks.saturating_add(1);

        match sender.try_send(callback) {
            Ok(()) => {
                source_report.emitted = source_report.emitted.saturating_add(1);
            }
            Err(TrySendError::Full(_)) => {
                source_report.backpressure = true;
                stopped_at = Some(index);
                break;
            }
            Err(TrySendError::Disconnected(_)) => {
                source_report.disconnected = true;
                stopped_at = Some(index);
                break;
            }
        }
    }

    if let Some(index) = stopped_at {
        source_report.queued_remaining = callbacks.len().saturating_sub(index);
    }

    if callbacks.len() > max_decode {
        source_report.max_reached = true;
        source_report.queued_remaining = source_report
            .queued_remaining
            .max(callbacks.len() - max_decode);
    }

    LibdrmNativePageFlipBatchReport {
        read_loop: LibdrmNativeReadLoopReport::callbacks_decoded(
            decoded_callbacks,
            rejected_callbacks,
        )
        .unwrap_or_else(LibdrmNativeReadLoopReport::idle),
        poll: LibdrmPageFlipEventPollReport::from_source_report(source_report),
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipBatchReport {
    pub read_loop: LibdrmNativeReadLoopReport,
    pub poll: LibdrmPageFlipEventPollReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipDecodeReport {
    pub status: LibdrmNativePageFlipDecodeStatus,
    pub callback: Option<LivePageFlipCallback>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePageFlipDecodeStatus {
    Decoded,
    UnknownOutputSlot,
    InvalidFrameSerial,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLibdrmPageFlipEventPoller {
    source: LibdrmNativePageFlipSource,
    routes: Vec<LibdrmNativeOutputRoute>,
    pending_callbacks: VecDeque<LibdrmNativePageFlipCallback>,
    last_read_loop: LibdrmNativeReadLoopReport,
}

#[cfg(feature = "libdrm-events")]
impl NativeLibdrmPageFlipEventPoller {
    pub fn new(source: LibdrmNativePageFlipSource) -> Self {
        Self {
            source,
            routes: Vec::new(),
            pending_callbacks: VecDeque::new(),
            last_read_loop: LibdrmNativeReadLoopReport::idle(),
        }
    }

    pub fn with_routes(
        mut self,
        routes: impl IntoIterator<Item = LibdrmNativeOutputRoute>,
    ) -> Self {
        self.replace_routes(routes);
        self
    }

    pub fn replace_routes(&mut self, routes: impl IntoIterator<Item = LibdrmNativeOutputRoute>) {
        self.routes.clear();
        self.routes.extend(routes);
    }

    pub fn inject_callbacks(
        &mut self,
        callbacks: impl IntoIterator<Item = LibdrmNativePageFlipCallback>,
    ) {
        self.pending_callbacks.extend(callbacks);
    }

    pub fn read_page_flip_events<R>(
        &mut self,
        reader: &mut R,
        max_read: usize,
    ) -> LibdrmNativeReadLoopReport
    where
        R: LibdrmNativePageFlipReader,
    {
        let result = reader.read_ready_page_flip_callbacks(max_read);
        self.last_read_loop = result.report;
        if result.report.status != LibdrmNativeReadLoopStatus::ReadFailed {
            self.pending_callbacks.extend(result.callbacks);
        }
        result.report
    }

    pub fn read_and_poll_page_flip_events<R>(
        &mut self,
        reader: &mut R,
        sender: &SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> LibdrmNativeReadAndPollReport
    where
        R: LibdrmNativePageFlipReader,
    {
        let read_loop = self.read_page_flip_events(reader, max_read);
        if read_loop.status == LibdrmNativeReadLoopStatus::ReadFailed {
            return LibdrmNativeReadAndPollReport {
                read_loop,
                poll: read_loop.into_poll_report(),
            };
        }

        LibdrmNativeReadAndPollReport {
            read_loop,
            poll: self.poll_page_flip_events(sender, max_emit),
        }
    }

    pub const fn source_report(&self) -> LibdrmNativePageFlipSourceReport {
        self.source.report()
    }

    pub const fn last_read_loop_report(&self) -> LibdrmNativeReadLoopReport {
        self.last_read_loop
    }

    pub fn pending_callback_count(&self) -> usize {
        self.pending_callbacks.len()
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn diagnostics(&self) -> LibdrmNativePollerDiagnostics {
        LibdrmNativePollerDiagnostics {
            route_count: self.routes.len(),
            pending_callbacks: self.pending_callbacks.len(),
            last_read_loop: self.last_read_loop,
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePageFlipReader {
    fn read_ready_page_flip_callbacks(&mut self, max_read: usize)
    -> LibdrmNativePageFlipReadResult;
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipReadResult {
    pub report: LibdrmNativeReadLoopReport,
    pub callbacks: Vec<LibdrmNativePageFlipCallback>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeLibdrmNativePageFlipReader {
    queued: VecDeque<LibdrmNativePageFlipCallback>,
    fail_next_read: bool,
}

#[cfg(feature = "libdrm-events")]
impl FakeLibdrmNativePageFlipReader {
    pub fn new(callbacks: impl IntoIterator<Item = LibdrmNativePageFlipCallback>) -> Self {
        Self {
            queued: callbacks.into_iter().collect(),
            fail_next_read: false,
        }
    }

    pub fn fail_next_read(&mut self) {
        self.fail_next_read = true;
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipReader for FakeLibdrmNativePageFlipReader {
    fn read_ready_page_flip_callbacks(
        &mut self,
        max_read: usize,
    ) -> LibdrmNativePageFlipReadResult {
        if self.fail_next_read {
            self.fail_next_read = false;
            return LibdrmNativePageFlipReadResult {
                report: LibdrmNativeReadLoopReport::read_failed(),
                callbacks: Vec::new(),
            };
        }

        let mut callbacks = Vec::new();
        for _ in 0..max_read {
            let Some(callback) = self.queued.pop_front() else {
                break;
            };
            callbacks.push(callback);
        }

        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callbacks_decoded(callbacks.len(), 0)
                .unwrap_or_else(LibdrmNativeReadLoopReport::would_block),
            callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct NativeLibdrmPageFlipEventReader<D> {
    device: D,
    crtc_routes: Vec<LibdrmNativeCrtcRoute>,
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmPageFlipEventReader<D> {
    pub fn new(device: D) -> Self {
        Self {
            device,
            crtc_routes: Vec::new(),
        }
    }

    pub fn with_crtc_routes(
        mut self,
        routes: impl IntoIterator<Item = LibdrmNativeCrtcRoute>,
    ) -> Self {
        self.replace_crtc_routes(routes);
        self
    }

    pub fn replace_crtc_routes(&mut self, routes: impl IntoIterator<Item = LibdrmNativeCrtcRoute>) {
        self.crtc_routes.clear();
        self.crtc_routes.extend(routes);
    }

    pub fn crtc_route_count(&self) -> usize {
        self.crtc_routes.len()
    }
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePageFlipReader for NativeLibdrmPageFlipEventReader<D>
where
    D: drm::control::Device,
{
    fn read_ready_page_flip_callbacks(
        &mut self,
        max_read: usize,
    ) -> LibdrmNativePageFlipReadResult {
        if max_read == 0 {
            return LibdrmNativePageFlipReadResult {
                report: LibdrmNativeReadLoopReport::would_block(),
                callbacks: Vec::new(),
            };
        }

        let events = match self.device.receive_events() {
            Ok(events) => events,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return LibdrmNativePageFlipReadResult {
                    report: LibdrmNativeReadLoopReport::would_block(),
                    callbacks: Vec::new(),
                };
            }
            Err(_) => {
                return LibdrmNativePageFlipReadResult {
                    report: LibdrmNativeReadLoopReport::read_failed(),
                    callbacks: Vec::new(),
                };
            }
        };

        let mut callbacks = Vec::new();
        let mut rejected_callbacks = 0usize;

        for event in events.take(max_read) {
            let drm::control::Event::PageFlip(page_flip) = event else {
                continue;
            };

            match reduce_native_page_flip_event(&page_flip, &self.crtc_routes) {
                Some(callback) => callbacks.push(callback),
                None => rejected_callbacks = rejected_callbacks.saturating_add(1),
            }
        }

        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callbacks_decoded(
                callbacks.len(),
                rejected_callbacks,
            )
            .unwrap_or_else(LibdrmNativeReadLoopReport::would_block),
            callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeReadAndPollReport {
    pub read_loop: LibdrmNativeReadLoopReport,
    pub poll: LibdrmPageFlipEventPollReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePollerDiagnostics {
    pub route_count: usize,
    pub pending_callbacks: usize,
    pub last_read_loop: LibdrmNativeReadLoopReport,
}

#[cfg(feature = "libdrm-events")]
impl From<LibdrmNativePollerDiagnostics> for LiveLibdrmPollerDiagnostics {
    fn from(diagnostics: LibdrmNativePollerDiagnostics) -> Self {
        Self {
            status: diagnostics.last_read_loop.status.into(),
            route_count: diagnostics.route_count,
            pending_callbacks: diagnostics.pending_callbacks,
            decoded_callbacks: diagnostics.last_read_loop.decoded_callbacks,
            rejected_callbacks: diagnostics.last_read_loop.rejected_callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeReadLoopReport {
    pub status: LibdrmNativeReadLoopStatus,
    pub decoded_callbacks: usize,
    pub rejected_callbacks: usize,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeReadLoopReport {
    pub const fn idle() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::Idle,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub const fn would_block() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::WouldBlock,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub const fn callbacks_decoded(
        decoded_callbacks: usize,
        rejected_callbacks: usize,
    ) -> Option<Self> {
        if decoded_callbacks == 0 && rejected_callbacks == 0 {
            return None;
        }

        Some(Self {
            status: if decoded_callbacks > 0 {
                LibdrmNativeReadLoopStatus::CallbackDecoded
            } else {
                LibdrmNativeReadLoopStatus::CallbackRejected
            },
            decoded_callbacks,
            rejected_callbacks,
        })
    }

    pub const fn callback_decoded(decoded_callbacks: usize) -> Option<Self> {
        Self::callbacks_decoded(decoded_callbacks, 0)
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::ReadFailed,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub fn into_poll_report(self) -> LibdrmPageFlipEventPollReport {
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: if matches!(self.status, LibdrmNativeReadLoopStatus::CallbackDecoded) {
                self.decoded_callbacks
            } else {
                0
            },
            queued_remaining: 0,
            backpressure: false,
            disconnected: matches!(self.status, LibdrmNativeReadLoopStatus::ReadFailed),
            max_reached: false,
        })
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeReadLoopStatus {
    Idle,
    WouldBlock,
    CallbackDecoded,
    CallbackRejected,
    ReadFailed,
}

#[cfg(feature = "libdrm-events")]
impl From<LibdrmNativeReadLoopStatus> for LiveLibdrmPollerDiagnosticsStatus {
    fn from(status: LibdrmNativeReadLoopStatus) -> Self {
        match status {
            LibdrmNativeReadLoopStatus::Idle => Self::Idle,
            LibdrmNativeReadLoopStatus::WouldBlock => Self::WouldBlock,
            LibdrmNativeReadLoopStatus::CallbackDecoded => Self::CallbackDecoded,
            LibdrmNativeReadLoopStatus::CallbackRejected => Self::CallbackRejected,
            LibdrmNativeReadLoopStatus::ReadFailed => Self::ReadFailed,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmBackendFdAuthority {
    generation: u64,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmBackendFdAuthority {
    pub const fn new(generation: u64) -> Option<Self> {
        if generation == 0 {
            return None;
        }

        Some(Self { generation })
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmBackendFdAuthorityReport {
    pub status: LibdrmBackendFdAuthorityStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmBackendFdAuthorityStatus {
    BackendOwned,
}

#[cfg(feature = "libdrm-events")]
mod native_libdrm_events {
    use super::{
        LibdrmBackendFdAuthority, LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
        LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus, LibdrmNativePageFlipSource,
    };

    pub(super) fn adapter_report() -> LibdrmNativeEventAdapterReport {
        let _ = core::mem::align_of::<drm::control::PageFlipEvent>();
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    }

    pub(super) fn adapter_report_for_authority(
        authority: LibdrmBackendFdAuthority,
    ) -> LibdrmNativeEventAdapterReport {
        let _ = fd_authority_report(authority);
        adapter_report()
    }

    pub(super) fn page_flip_source_from_authority(
        authority: LibdrmBackendFdAuthority,
    ) -> LibdrmNativePageFlipSource {
        let _ = fd_authority_report(authority);
        LibdrmNativePageFlipSource { _private: () }
    }

    pub(super) fn fd_authority_report(
        _authority: LibdrmBackendFdAuthority,
    ) -> LibdrmBackendFdAuthorityReport {
        LibdrmBackendFdAuthorityReport {
            status: LibdrmBackendFdAuthorityStatus::BackendOwned,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmPageFlipEventPollReport {
    pub status: LibdrmPageFlipEventPollStatus,
    pub callbacks: LivePageFlipCallbackSourceReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmPageFlipEventPollStatus {
    Idle,
    Emitted,
    Backpressure,
    Disconnected,
    EmitLimitReached,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPollReport {
    pub fn from_source_report(callbacks: LivePageFlipCallbackSourceReport) -> Self {
        let status = if callbacks.disconnected {
            LibdrmPageFlipEventPollStatus::Disconnected
        } else if callbacks.backpressure {
            LibdrmPageFlipEventPollStatus::Backpressure
        } else if callbacks.max_reached {
            LibdrmPageFlipEventPollStatus::EmitLimitReached
        } else if callbacks.emitted > 0 {
            LibdrmPageFlipEventPollStatus::Emitted
        } else {
            LibdrmPageFlipEventPollStatus::Idle
        };

        Self { status, callbacks }
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport;
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakeLibdrmPageFlipEventPoller {
    source: FakePageFlipCallbackSource,
}

#[cfg(feature = "libdrm-events")]
impl FakeLibdrmPageFlipEventPoller {
    pub fn new(callbacks: impl IntoIterator<Item = LivePageFlipCallback>) -> Self {
        Self {
            source: FakePageFlipCallbackSource::new(callbacks),
        }
    }

    pub fn queued_len(&self) -> usize {
        self.source.queued_len()
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPoller for FakeLibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport {
        LibdrmPageFlipEventPollReport::from_source_report(self.source.emit_ready(sender, max_emit))
    }
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPoller for NativeLibdrmPageFlipEventPoller {
    fn poll_page_flip_events(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LibdrmPageFlipEventPollReport {
        let _ = self.source.report();
        if self.pending_callbacks.is_empty() {
            self.last_read_loop = LibdrmNativeReadLoopReport::idle();
            return self.last_read_loop.into_poll_report();
        }

        let pending = self.pending_callbacks.iter().copied().collect::<Vec<_>>();
        let report = decode_native_page_flip_batch(&pending, &self.routes, sender, max_emit);
        let processed_callbacks = pending
            .len()
            .saturating_sub(report.poll.callbacks.queued_remaining);

        for _ in 0..processed_callbacks {
            let _ = self.pending_callbacks.pop_front();
        }

        self.last_read_loop = report.read_loop;
        report.poll
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendStartupReport {
    pub discovery: LiveCompositorBackendDiscoveryReport,
    pub renderer_import: LiveRendererImportBoundary,
    pub renderer_preference: LiveRendererPreference,
}

impl LiveBackendStartupReport {
    pub fn status(&self) -> &LiveCompositorBackendDiscoveryStatus {
        &self.discovery.status
    }

    pub fn selected_output(&self) -> Option<HeadlessOutput> {
        self.discovery.selected_output
    }

    pub fn renderer_selection(&self) -> RendererSelection {
        self.try_renderer_selection()
            .unwrap_or(RendererSelection::CpuFallback)
    }

    pub fn try_renderer_selection(&self) -> Option<RendererSelection> {
        self.renderer_selection_for_status(self.renderer_import_status())
    }

    pub fn renderer_selection_for_status(
        &self,
        status: LiveRendererImportStartupStatus,
    ) -> Option<RendererSelection> {
        match self.renderer_preference {
            LiveRendererPreference::CpuOnly => Some(RendererSelection::CpuFallback),
            LiveRendererPreference::GpuPreferred => {
                Some(selection_from_native_status(status).unwrap_or(RendererSelection::CpuFallback))
            }
            LiveRendererPreference::GpuRequired => selection_from_native_status(status),
        }
    }

    pub fn renderer_runtime_status_for_preference(
        &self,
        status: LiveRendererImportStartupStatus,
    ) -> LiveRendererImportStartupStatus {
        match self.renderer_preference {
            LiveRendererPreference::CpuOnly => cpu_fallback_renderer_status(),
            LiveRendererPreference::GpuPreferred | LiveRendererPreference::GpuRequired => status,
        }
    }

    pub fn renderer_import_status(&self) -> LiveRendererImportStartupStatus {
        self.renderer_import.startup_status()
    }

    pub fn scanout_readiness_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveScanoutReadinessReport {
        LiveScanoutReadinessReport::from_backend_and_presentation(self, presentation)
    }

    pub fn kms_scanout_target_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveKmsScanoutTargetReport {
        LiveKmsScanoutTargetReport::from_backend_and_presentation(self, presentation)
    }

    pub fn selected_gbm_egl_frame_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.selected_output()
            .map(|output| LiveGbmEglFrameTargetRecord::new(output.size))
    }

    #[cfg(feature = "libdrm-events")]
    pub fn native_libdrm_output_routes(&self) -> Vec<LibdrmNativeOutputRoute> {
        self.discovery
            .outputs
            .outputs()
            .enumerate()
            .filter_map(|(index, output)| {
                LibdrmNativeOutputSlot::new(
                    u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX),
                )
                .map(|slot| LibdrmNativeOutputRoute {
                    slot,
                    output: output.output,
                })
            })
            .collect()
    }

    #[cfg(feature = "libdrm-events")]
    pub fn native_libdrm_poller_startup_report(&self) -> LiveLibdrmPollerStartupReport {
        if !self.discovery.is_ready() {
            return LiveLibdrmPollerStartupReport {
                status: if self.discovery.selected_output.is_none() {
                    LiveLibdrmPollerStartupStatus::NoOutputs
                } else {
                    LiveLibdrmPollerStartupStatus::BackendNotReady
                },
                route_count: 0,
            };
        }

        let route_count = self.native_libdrm_output_routes().len();
        LiveLibdrmPollerStartupReport {
            status: if route_count == 0 {
                LiveLibdrmPollerStartupStatus::NoOutputs
            } else {
                LiveLibdrmPollerStartupStatus::Ready
            },
            route_count,
        }
    }

    #[cfg(feature = "libdrm-events")]
    pub fn native_libdrm_poller_from_authority(
        &self,
        authority: LibdrmBackendFdAuthority,
    ) -> Option<NativeLibdrmPageFlipEventPoller> {
        if !self.discovery.is_ready() {
            return None;
        }

        Some(
            NativeLibdrmPageFlipEventPoller::new(LibdrmNativePageFlipSource::from_authority(
                authority,
            ))
            .with_routes(self.native_libdrm_output_routes()),
        )
    }

    #[cfg(feature = "egl-probe")]
    pub fn egl_probe_report(
        &self,
        platform: EglPlatformStatus,
        context: EglContextProbeStatus,
    ) -> LiveEglStartupReport {
        LiveEglStartupReport::from_probe_status(
            FakeEglCapabilityProbe::new(platform, context)
                .probe_report()
                .status,
        )
    }

    #[cfg(feature = "egl-probe")]
    pub fn native_egl_probe_report(&self) -> LiveEglStartupReport {
        LiveEglStartupReport::from_probe_status(NativeEglCapabilityProbe::probe_report().status)
    }

    #[cfg(feature = "egl-probe")]
    pub fn native_egl_draw_smoke_report(&self) -> EglDrawSmokeReport {
        NativeEglDrawSmoke::smoke_report()
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn gbm_backed_egl_platform_report(
        &self,
        gpu_startup: LiveGpuStartupReport,
    ) -> LiveGbmBackedEglPlatformReport {
        LiveGbmBackedEglPlatformReport::from_gpu_startup(gpu_startup)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_platform_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveGbmBackedEglPlatformReport {
        LiveGbmBackedEglPlatformReport {
            status: NativeGbmBackedEglPlatformProbe::platform_status_from_backend_device_result(
                device,
            ),
        }
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_platform_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveGbmBackedEglPlatformReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_platform_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_draw_smoke_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> EglDrawSmokeReport {
        NativeGbmBackedEglDrawSmoke::smoke_report_from_backend_device_result(device)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_draw_smoke_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> EglDrawSmokeReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_draw_smoke_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_presentation_smoke_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveRendererPresentationReport {
        NativeGbmBackedEglPresentationSmoke::smoke_report_from_backend_device_result(device)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_presentation_smoke_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveRendererPresentationReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_presentation_smoke_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_frame_target_allocation_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport {
        NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
            device, request,
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_frame_target_allocation_report_with_gbm_device<D>(
        &self,
        discovery: &D,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_frame_target_allocation_report_from_device_result(
            discovery.open_render_device(),
            request,
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn egl_probe_report_from_gbm_startup(
        &self,
        gpu_startup: LiveGpuStartupReport,
        context: EglContextProbeStatus,
    ) -> LiveEglStartupReport {
        self.egl_probe_report(
            self.gbm_backed_egl_platform_report(gpu_startup).status,
            context,
        )
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_import_status_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveRendererImportStartupStatus
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.renderer_probe_report_with_gbm_device(discovery)
            .renderer_import
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_selection_with_gbm_device<D>(&self, discovery: &D) -> Option<RendererSelection>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.renderer_selection_for_status(self.renderer_import_status_with_gbm_device(discovery))
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_probe_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveBackendRendererProbeReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        if self.renderer_preference == LiveRendererPreference::CpuOnly
            || !self.renderer_import.import_dmabuf
        {
            return LiveBackendRendererProbeReport {
                render_device: LiveRenderDeviceDiscoveryReport {
                    status: LiveRenderDeviceDiscoveryStatus::NotRequested,
                },
                gpu_startup: LiveGpuStartupReport::not_requested(),
                renderer_import: self
                    .renderer_runtime_status_for_preference(self.renderer_import_status()),
            };
        }

        let device = discovery.open_render_device();
        let render_device = LiveRenderDeviceDiscoveryReport::from_open_result(&device);
        let probe_report =
            NativeGbmCapabilityProbe::probe_report_from_backend_device_result(device);
        let renderer_import = self.renderer_runtime_status_for_preference(
            self.renderer_import_status_from_gbm_probe(probe_report),
        );
        let gpu_startup =
            LiveGpuStartupReport::from_discovery_and_probe(render_device, probe_report.status);

        LiveBackendRendererProbeReport {
            render_device,
            gpu_startup,
            renderer_import,
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_import_status_from_gbm_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveRendererImportStartupStatus {
        let configured = self.renderer_import_status();

        if !self.renderer_import.import_dmabuf {
            return configured;
        }

        self.renderer_import_status_from_gbm_probe(
            NativeGbmCapabilityProbe::probe_report_from_backend_device_result(device),
        )
    }

    #[cfg(feature = "gbm-probe")]
    fn renderer_import_status_from_gbm_probe(
        &self,
        probe_report: GbmCapabilityProbeReport,
    ) -> LiveRendererImportStartupStatus {
        let configured = self.renderer_import_status();

        if !self.renderer_import.import_dmabuf {
            return configured;
        }

        LiveRendererImportStartupStatus::from_path_statuses(
            configured.xpixmap,
            probe_report.startup_status.dmabuf,
        )
    }

    pub fn into_configured_headless_assembly<P>(
        self,
        poller: P,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let renderer = self.try_renderer_selection()?;
        self.into_headless_assembly(poller, renderer)
    }

    pub fn into_live_runtime_assembly<P>(self, poller: P) -> Option<LiveBackendRuntimeAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let renderer_status =
            self.renderer_runtime_status_for_preference(self.renderer_import_status());
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_configured_headless_assembly_with_gbm_device<P, D>(
        self,
        poller: P,
        discovery: &D,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        let renderer = self.renderer_selection_for_status(renderer_status)?;
        self.into_headless_assembly(poller, renderer)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_live_runtime_assembly_with_gbm_device<P, D>(
        self,
        poller: P,
        discovery: &D,
    ) -> Option<LiveBackendRuntimeAssembly<P>>
    where
        P: NonBlockingInputPoller,
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    pub fn into_headless_assembly<P>(
        self,
        poller: P,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        self.discovery.into_headless_assembly(poller, renderer)
    }

    fn into_live_runtime_assembly_with_status<P>(
        self,
        poller: P,
        renderer_status: LiveRendererImportStartupStatus,
    ) -> Option<LiveBackendRuntimeAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let renderer_selection = self.renderer_selection_for_status(renderer_status)?;
        let selected_output = self.selected_output()?;
        let renderer_observation = LiveRendererRuntimeObservation::from_startup_status(
            renderer_status,
            selection_observation(renderer_selection),
        );
        let scanout_readiness = self.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        });
        let kms_scanout_target = self.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        });
        let page_flip_event =
            LivePageFlipEvent::from_kms_scanout_target_status(kms_scanout_target.status);
        let page_flip_callback_intake = LivePageFlipCallbackIntake::new(selected_output.id);
        let gbm_egl_frame_target = LiveGbmEglFrameTargetRecord::new(selected_output.size);
        self.into_headless_assembly(poller, renderer_selection)
            .map(|assembly| LiveBackendRuntimeAssembly {
                assembly,
                renderer_observation,
                output_size: Some(selected_output.size),
                scanout_readiness,
                kms_scanout_target,
                gbm_egl_frame_target: Some(gbm_egl_frame_target),
                gbm_egl_frame_target_lifecycle: Some(
                    LiveGbmEglFrameTargetLifecycleReport::created(gbm_egl_frame_target),
                ),
                gbm_egl_frame_target_allocation: None,
                page_flip_event,
                page_flip_callback_intake,
                page_flip_callback_queue: None,
                libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics::not_configured(),
            })
    }
}

#[cfg(feature = "gbm-probe")]
pub trait RenderDeviceDiscoveryBackend {
    type Device: AsFd;

    fn open_render_device(&self) -> io::Result<Self::Device>;
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendRendererProbeReport {
    pub render_device: LiveRenderDeviceDiscoveryReport,
    pub gpu_startup: LiveGpuStartupReport,
    pub renderer_import: LiveRendererImportStartupStatus,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRenderDeviceDiscoveryReport {
    pub status: LiveRenderDeviceDiscoveryStatus,
}

#[cfg(feature = "gbm-probe")]
impl LiveRenderDeviceDiscoveryReport {
    fn from_open_result<T>(device: &io::Result<T>) -> Self {
        Self {
            status: if device.is_ok() {
                LiveRenderDeviceDiscoveryStatus::Opened
            } else {
                LiveRenderDeviceDiscoveryStatus::Unavailable
            },
        }
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderDeviceDiscoveryStatus {
    NotRequested,
    Opened,
    Unavailable,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGpuStartupReport {
    pub status: LiveGpuStartupStatus,
}

#[cfg(feature = "gbm-probe")]
impl LiveGpuStartupReport {
    fn not_requested() -> Self {
        Self {
            status: LiveGpuStartupStatus::NotRequested,
        }
    }

    fn from_discovery_and_probe(
        discovery: LiveRenderDeviceDiscoveryReport,
        probe_status: GbmCapabilityProbeStatus,
    ) -> Self {
        if discovery.status != LiveRenderDeviceDiscoveryStatus::Opened {
            return Self {
                status: LiveGpuStartupStatus::RenderDeviceUnavailable,
            };
        }

        Self {
            status: match probe_status {
                GbmCapabilityProbeStatus::NativeCapable => LiveGpuStartupStatus::NativeCapable,
                GbmCapabilityProbeStatus::ReducedDeviceUnavailable => {
                    LiveGpuStartupStatus::RenderDeviceUnavailable
                }
                GbmCapabilityProbeStatus::NativeDeviceRejected => {
                    LiveGpuStartupStatus::GbmDeviceRejected
                }
                GbmCapabilityProbeStatus::PrivateAllocationUnavailable => {
                    LiveGpuStartupStatus::PrivateAllocationUnavailable
                }
            },
        }
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGpuStartupStatus {
    NotRequested,
    NativeCapable,
    RenderDeviceUnavailable,
    GbmDeviceRejected,
    PrivateAllocationUnavailable,
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl From<LiveGpuStartupStatus> for EglPlatformStatus {
    fn from(status: LiveGpuStartupStatus) -> Self {
        match status {
            LiveGpuStartupStatus::NativeCapable => EglPlatformStatus::NativePlatformCapable,
            LiveGpuStartupStatus::NotRequested | LiveGpuStartupStatus::RenderDeviceUnavailable => {
                EglPlatformStatus::PlatformUnavailable
            }
            LiveGpuStartupStatus::GbmDeviceRejected
            | LiveGpuStartupStatus::PrivateAllocationUnavailable => {
                EglPlatformStatus::PlatformDegraded
            }
        }
    }
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmBackedEglPlatformReport {
    pub status: EglPlatformStatus,
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl LiveGbmBackedEglPlatformReport {
    pub fn from_gpu_startup(gpu_startup: LiveGpuStartupReport) -> Self {
        Self {
            status: EglPlatformStatus::from(gpu_startup.status),
        }
    }
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRealGbmSmokeEvidence {
    pub status: LiveRealGbmSmokeEvidenceStatus,
    pub draw: EglDrawSmokeStatus,
    pub presentation: LiveRendererPresentationStatus,
    pub frame_target_allocation: LiveGbmEglFrameTargetAllocationStatus,
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl LiveRealGbmSmokeEvidence {
    pub const fn from_reports(
        draw: EglDrawSmokeReport,
        presentation: LiveRendererPresentationReport,
        frame_target_allocation: LiveGbmEglFrameTargetAllocationReport,
    ) -> Self {
        let status = match (
            draw.status,
            presentation.status,
            frame_target_allocation.status,
        ) {
            (
                EglDrawSmokeStatus::ClearColorReady,
                LiveRendererPresentationStatus::Ready,
                LiveGbmEglFrameTargetAllocationStatus::Ready,
            ) => LiveRealGbmSmokeEvidenceStatus::Passed,
            _ => LiveRealGbmSmokeEvidenceStatus::Failed,
        };

        Self {
            status,
            draw: draw.status,
            presentation: presentation.status,
            frame_target_allocation: frame_target_allocation.status,
        }
    }
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRealGbmSmokeEvidenceStatus {
    Passed,
    Failed,
}

#[cfg(feature = "egl-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveEglStartupReport {
    pub status: LiveEglStartupStatus,
}

#[cfg(feature = "egl-probe")]
impl LiveEglStartupReport {
    fn from_probe_status(status: EglCapabilityProbeStatus) -> Self {
        Self {
            status: match status {
                EglCapabilityProbeStatus::NativeDrawingCapable => {
                    LiveEglStartupStatus::NativeDrawingCapable
                }
                EglCapabilityProbeStatus::PlatformUnavailable => {
                    LiveEglStartupStatus::PlatformUnavailable
                }
                EglCapabilityProbeStatus::PlatformDegraded => {
                    LiveEglStartupStatus::PlatformDegraded
                }
                EglCapabilityProbeStatus::ContextUnavailable => {
                    LiveEglStartupStatus::ContextUnavailable
                }
            },
        }
    }
}

#[cfg(feature = "egl-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveEglStartupStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

pub struct LiveBackendRuntimeAssembly<P = QueuedInputPoller> {
    assembly: HeadlessCompositorBackendAssembly<P>,
    renderer_observation: LiveRendererRuntimeObservation,
    output_size: Option<Size>,
    scanout_readiness: LiveScanoutReadinessReport,
    kms_scanout_target: LiveKmsScanoutTargetReport,
    gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    page_flip_event: LivePageFlipEvent,
    page_flip_callback_intake: LivePageFlipCallbackIntake,
    page_flip_callback_queue: Option<LivePageFlipCallbackQueue>,
    libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics,
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly<P> {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly<P> {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
    }

    pub fn with_page_flip_callback_queue(mut self, queue: LivePageFlipCallbackQueue) -> Self {
        self.page_flip_callback_queue = Some(queue);
        self
    }

    pub fn with_libdrm_poller_diagnostics(
        mut self,
        diagnostics: LiveLibdrmPollerDiagnostics,
    ) -> Self {
        self.libdrm_poller_diagnostics = diagnostics;
        self
    }

    #[cfg(feature = "libdrm-events")]
    pub fn with_native_libdrm_poller_diagnostics(
        self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) -> Self {
        self.with_libdrm_poller_diagnostics(diagnostics.into())
    }

    pub fn observe_libdrm_poller_diagnostics(&mut self, diagnostics: LiveLibdrmPollerDiagnostics) {
        self.libdrm_poller_diagnostics = diagnostics;
    }

    #[cfg(feature = "libdrm-events")]
    pub fn observe_native_libdrm_poller_diagnostics(
        &mut self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) {
        self.observe_libdrm_poller_diagnostics(diagnostics.into());
    }

    pub fn libdrm_poller_diagnostics(&self) -> LiveLibdrmPollerDiagnostics {
        self.libdrm_poller_diagnostics
    }

    pub fn scanout_readiness_observation(&self) -> LiveScanoutReadinessReport {
        self.scanout_readiness
    }

    pub fn kms_scanout_target_observation(&self) -> LiveKmsScanoutTargetReport {
        self.kms_scanout_target
    }

    pub fn gbm_egl_frame_target_observation(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.gbm_egl_frame_target
    }

    pub fn gbm_egl_frame_target_lifecycle_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.gbm_egl_frame_target_lifecycle
    }

    pub fn gbm_egl_frame_target_allocation_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport> {
        self.gbm_egl_frame_target_allocation
    }

    pub fn observe_gbm_egl_frame_target_size(&mut self, size: Size) -> LiveGbmEglFrameTargetRecord {
        let previous = self.gbm_egl_frame_target;
        let record = LiveGbmEglFrameTargetRecord::new(size);
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::from_size_update(previous, record);
        self.gbm_egl_frame_target = Some(record);
        self.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        if lifecycle.status != LiveGbmEglFrameTargetLifecycleStatus::Retained {
            self.gbm_egl_frame_target_allocation = None;
        }
        self.refresh_kms_scanout_target(LiveRendererPresentationReport {
            status: match self.scanout_readiness.status {
                LiveScanoutReadinessStatus::Ready => LiveRendererPresentationStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable
                | LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LiveRendererPresentationStatus::Unavailable
                }
                LiveScanoutReadinessStatus::Degraded => LiveRendererPresentationStatus::Degraded,
            },
        });
        record
    }

    pub fn retire_gbm_egl_frame_target(&mut self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        let target = self.gbm_egl_frame_target.take()?;
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::retired(target);
        self.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        self.gbm_egl_frame_target_allocation = None;
        self.refresh_kms_scanout_target(LiveRendererPresentationReport {
            status: match self.scanout_readiness.status {
                LiveScanoutReadinessStatus::Ready => LiveRendererPresentationStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable
                | LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LiveRendererPresentationStatus::Unavailable
                }
                LiveScanoutReadinessStatus::Degraded => LiveRendererPresentationStatus::Degraded,
            },
        });
        Some(lifecycle)
    }

    pub fn allocate_gbm_egl_frame_target<A>(
        &mut self,
        allocator: &mut A,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        A: LiveGbmEglFrameTargetAllocator,
    {
        let target = self.gbm_egl_frame_target?;
        let report =
            allocator.allocate_frame_target(LiveGbmEglFrameTargetAllocationRequest { target });
        self.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn allocate_native_gbm_egl_frame_target_with_gbm_device<D>(
        &mut self,
        discovery: &D,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let target = self.gbm_egl_frame_target?;
        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
                discovery.open_render_device(),
                LiveGbmEglFrameTargetAllocationRequest { target },
            );
        self.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    pub fn page_flip_observation(&self) -> LivePageFlipEvent {
        self.page_flip_event
    }

    pub fn observe_presentation_report(&mut self, presentation: LiveRendererPresentationReport) {
        self.scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        self.refresh_kms_scanout_target(presentation);
    }

    pub fn observe_page_flip_outcome(&mut self, outcome: &PageFlipCommitOutcome) {
        self.page_flip_event = LivePageFlipEvent::from_commit_outcome(outcome);
    }

    pub fn observe_atomic_scanout_commit(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report = LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_with<C>(
        &mut self,
        committer: &mut C,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let report = committer.commit_atomic_scanout(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_after_page_flip_with<C>(
        &mut self,
        committer: &mut C,
        callback: LivePageFlipCallback,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let callback_report = self.page_flip_callback_intake.observe(callback);
        let report = committer.commit_atomic_scanout_after_page_flip(&callback_report, outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn observe_page_flip_callback(
        &mut self,
        callback: LivePageFlipCallback,
    ) -> LivePageFlipCallbackReport {
        let report = self.page_flip_callback_intake.observe(callback);
        self.page_flip_event = report.event;
        report
    }

    pub fn run_tick(
        &mut self,
        input: CompositorBackendTickInput,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError> {
        let page_flip_callbacks = self
            .page_flip_callback_queue
            .as_ref()
            .map(|queue| {
                queue.drain_ready(
                    &mut self.page_flip_callback_intake,
                    &mut self.page_flip_event,
                )
            })
            .unwrap_or_default();
        let engine = self.assembly.run_tick(input)?;

        Ok(LiveBackendRuntimeTickReport {
            engine,
            renderer: self.renderer_observation,
            scanout: self.scanout_readiness,
            kms_scanout_target: self.kms_scanout_target,
            gbm_egl_frame_target: self.gbm_egl_frame_target,
            gbm_egl_frame_target_lifecycle: self.gbm_egl_frame_target_lifecycle,
            gbm_egl_frame_target_allocation: self.gbm_egl_frame_target_allocation,
            page_flip: self.page_flip_event,
            page_flip_callbacks,
            libdrm_poller: self.libdrm_poller_diagnostics,
        })
    }
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    fn refresh_kms_scanout_target(&mut self, presentation: LiveRendererPresentationReport) {
        self.kms_scanout_target = LiveKmsScanoutTargetReport::from_output_target_and_presentation(
            self.output_size,
            self.gbm_egl_frame_target,
            presentation,
        );
        self.page_flip_event =
            LivePageFlipEvent::from_kms_scanout_target_status(self.kms_scanout_target.status);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeTickReport {
    pub engine: CompositorBackendTickReport,
    pub renderer: LiveRendererRuntimeObservation,
    pub scanout: LiveScanoutReadinessReport,
    pub kms_scanout_target: LiveKmsScanoutTargetReport,
    pub gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    pub gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    pub page_flip: LivePageFlipEvent,
    pub page_flip_callbacks: LivePageFlipCallbackQueueReport,
    pub libdrm_poller: LiveLibdrmPollerDiagnostics,
}

pub fn discover_live_backend(config: &LiveBackendConfig) -> LiveBackendStartupReport {
    let output_backend = SysfsDrmKmsOutputBackend::new(&config.drm_sysfs_root);
    let input_backend = StaticInputDiscoveryBackend::new(config.input_devices.clone());

    LiveBackendStartupReport {
        discovery: discover_live_compositor_backend(&output_backend, &input_backend),
        renderer_import: config.renderer_import,
        renderer_preference: config.renderer_preference,
    }
}

fn selection_from_native_status(
    status: LiveRendererImportStartupStatus,
) -> Option<RendererSelection> {
    if status.health != LiveRendererImportHealth::NativeImportCapable {
        return None;
    }

    Some(RendererSelection::ImportCapable {
        import_xpixmap: status.xpixmap == LiveRendererImportPathStatus::Enabled,
        import_dmabuf: status.dmabuf == LiveRendererImportPathStatus::Enabled,
    })
}

fn cpu_fallback_renderer_status() -> LiveRendererImportStartupStatus {
    LiveRendererImportBoundary::cpu_only().startup_status()
}

fn selection_observation(selection: RendererSelection) -> LiveRendererSelectionObservation {
    match selection {
        RendererSelection::CpuFallback => LiveRendererSelectionObservation::CpuFallback,
        RendererSelection::ImportCapable { .. } => {
            LiveRendererSelectionObservation::NativeImportCapable
        }
    }
}

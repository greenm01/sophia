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
#[cfg(feature = "libdrm-events")]
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportStatus, LiveRendererScanoutBufferStatus,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
use sophia_renderer_live::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglFrameTargetAllocator,
    NativeGbmBackedEglPlatformProbe, NativeGbmBackedEglPresentationSmoke,
};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExporter};

pub const LIVE_PAGE_FLIP_CALLBACK_CHANNEL_CAPACITY: usize = 128;
pub const SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE: &str = "SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE";

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
    AtomicScanout,
}

impl LiveHardwareValidationTarget {
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::LibdrmEvents => SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE,
            Self::LibinputEvents => SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE,
            Self::AtomicScanout => SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE,
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

pub fn real_atomic_scanout_validation_gate() -> LiveHardwareValidationGateReport {
    let target = LiveHardwareValidationTarget::AtomicScanout;
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

pub fn real_atomic_scanout_validation_smoke_report() -> LiveHardwareValidationSmokeReport {
    LiveHardwareValidationSmokeReport::fail_closed_from_gate(real_atomic_scanout_validation_gate())
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

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativeAtomicCommitRequest {
    request: drm::control::atomic::AtomicModeReq,
    page_flip_event: bool,
    nonblocking: bool,
    allow_modeset: bool,
    test_only: bool,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicCommitRequest {
    pub const fn new(request: drm::control::atomic::AtomicModeReq) -> Self {
        Self {
            request,
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    }

    pub const fn without_page_flip_event(mut self) -> Self {
        self.page_flip_event = false;
        self
    }

    pub const fn blocking(mut self) -> Self {
        self.nonblocking = false;
        self
    }

    pub const fn allow_modeset(mut self) -> Self {
        self.allow_modeset = true;
        self
    }

    pub const fn test_only(mut self) -> Self {
        self.test_only = true;
        self
    }

    pub const fn reduced_flags(&self) -> LibdrmNativeAtomicCommitFlagsReport {
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: self.page_flip_event,
            nonblocking: self.nonblocking,
            allow_modeset: self.allow_modeset,
            test_only: self.test_only,
        }
    }

    fn into_native(
        self,
    ) -> (
        drm::control::AtomicCommitFlags,
        drm::control::atomic::AtomicModeReq,
    ) {
        let mut flags = drm::control::AtomicCommitFlags::empty();
        if self.page_flip_event {
            flags |= drm::control::AtomicCommitFlags::PAGE_FLIP_EVENT;
        }
        if self.nonblocking {
            flags |= drm::control::AtomicCommitFlags::NONBLOCK;
        }
        if self.allow_modeset {
            flags |= drm::control::AtomicCommitFlags::ALLOW_MODESET;
        }
        if self.test_only {
            flags |= drm::control::AtomicCommitFlags::TEST_ONLY;
        }
        (flags, self.request)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicCommitFlagsReport {
    pub page_flip_event: bool,
    pub nonblocking: bool,
    pub allow_modeset: bool,
    pub test_only: bool,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicCommitSubmitReport {
    pub status: LibdrmNativeAtomicCommitSubmitStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicCommitSubmitStatus {
    Submitted,
    WouldBlock,
    Rejected,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlanePropertyHandles {
    connector_crtc_id: drm::control::property::Handle,
    crtc_mode_id: drm::control::property::Handle,
    crtc_active: drm::control::property::Handle,
    plane_fb_id: drm::control::property::Handle,
    plane_crtc_id: drm::control::property::Handle,
    plane_src_x: drm::control::property::Handle,
    plane_src_y: drm::control::property::Handle,
    plane_src_w: drm::control::property::Handle,
    plane_src_h: drm::control::property::Handle,
    plane_crtc_x: drm::control::property::Handle,
    plane_crtc_y: drm::control::property::Handle,
    plane_crtc_w: drm::control::property::Handle,
    plane_crtc_h: drm::control::property::Handle,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlanePropertyHandles {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        connector_crtc_id: drm::control::property::Handle,
        crtc_mode_id: drm::control::property::Handle,
        crtc_active: drm::control::property::Handle,
        plane_fb_id: drm::control::property::Handle,
        plane_crtc_id: drm::control::property::Handle,
        plane_src_x: drm::control::property::Handle,
        plane_src_y: drm::control::property::Handle,
        plane_src_w: drm::control::property::Handle,
        plane_src_h: drm::control::property::Handle,
        plane_crtc_x: drm::control::property::Handle,
        plane_crtc_y: drm::control::property::Handle,
        plane_crtc_w: drm::control::property::Handle,
        plane_crtc_h: drm::control::property::Handle,
    ) -> Self {
        Self {
            connector_crtc_id,
            crtc_mode_id,
            crtc_active,
            plane_fb_id,
            plane_crtc_id,
            plane_src_x,
            plane_src_y,
            plane_src_w,
            plane_src_h,
            plane_crtc_x,
            plane_crtc_y,
            plane_crtc_w,
            plane_crtc_h,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibdrmNativePropertyHandleSet {
    handles: Vec<(String, drm::control::property::Handle)>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePropertyHandleSet {
    pub fn new(
        handles: impl IntoIterator<Item = (impl Into<String>, drm::control::property::Handle)>,
    ) -> Self {
        Self {
            handles: handles
                .into_iter()
                .map(|(name, handle)| (name.into(), handle))
                .collect(),
        }
    }

    pub fn from_property_info_map(
        map: std::collections::HashMap<String, drm::control::property::Info>,
    ) -> Self {
        Self {
            handles: map
                .into_iter()
                .map(|(name, info)| (name, info.handle()))
                .collect(),
        }
    }

    fn get(&self, name: &str) -> Option<drm::control::property::Handle> {
        self.handles
            .iter()
            .find_map(|(candidate, handle)| (candidate == name).then_some(*handle))
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePropertyLookupDevice {
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet>;

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet>;

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePropertyLookupDevice for D
where
    D: drm::control::Device,
{
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        Ok(LibdrmNativePropertyHandleSet::from_property_info_map(
            self.get_properties(connector)?.as_hashmap(self)?,
        ))
    }

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        Ok(LibdrmNativePropertyHandleSet::from_property_info_map(
            self.get_properties(crtc)?.as_hashmap(self)?,
        ))
    }

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        Ok(LibdrmNativePropertyHandleSet::from_property_info_map(
            self.get_properties(plane)?.as_hashmap(self)?,
        ))
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlanePropertyDiscoveryResult {
    pub status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyHandles>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlanePropertyDiscoveryStatus {
    Discovered,
    ReadFailed,
    MissingConnectorProperty,
    MissingCrtcProperty,
    MissingPlaneProperty,
}

#[cfg(feature = "libdrm-events")]
pub fn discover_native_primary_plane_property_handles<D>(
    device: &D,
    connector: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
    plane: drm::control::plane::Handle,
) -> LibdrmNativePrimaryPlanePropertyDiscoveryResult
where
    D: LibdrmNativePropertyLookupDevice,
{
    let Ok(connector_properties) = device.connector_property_handles(connector) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed,
            properties: None,
        };
    };
    let Some(connector_crtc_id) = connector_properties.get("CRTC_ID") else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingConnectorProperty,
            properties: None,
        };
    };

    let Ok(crtc_properties) = device.crtc_property_handles(crtc) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed,
            properties: None,
        };
    };
    let (Some(crtc_mode_id), Some(crtc_active)) = (
        crtc_properties.get("MODE_ID"),
        crtc_properties.get("ACTIVE"),
    ) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingCrtcProperty,
            properties: None,
        };
    };

    let Ok(plane_properties) = device.plane_property_handles(plane) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed,
            properties: None,
        };
    };
    let (
        Some(plane_fb_id),
        Some(plane_crtc_id),
        Some(plane_src_x),
        Some(plane_src_y),
        Some(plane_src_w),
        Some(plane_src_h),
        Some(plane_crtc_x),
        Some(plane_crtc_y),
        Some(plane_crtc_w),
        Some(plane_crtc_h),
    ) = (
        plane_properties.get("FB_ID"),
        plane_properties.get("CRTC_ID"),
        plane_properties.get("SRC_X"),
        plane_properties.get("SRC_Y"),
        plane_properties.get("SRC_W"),
        plane_properties.get("SRC_H"),
        plane_properties.get("CRTC_X"),
        plane_properties.get("CRTC_Y"),
        plane_properties.get("CRTC_W"),
        plane_properties.get("CRTC_H"),
    )
    else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingPlaneProperty,
            properties: None,
        };
    };

    LibdrmNativePrimaryPlanePropertyDiscoveryResult {
        status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered,
        properties: Some(LibdrmNativePrimaryPlanePropertyHandles::new(
            connector_crtc_id,
            crtc_mode_id,
            crtc_active,
            plane_fb_id,
            plane_crtc_id,
            plane_src_x,
            plane_src_y,
            plane_src_w,
            plane_src_h,
            plane_crtc_x,
            plane_crtc_y,
            plane_crtc_w,
            plane_crtc_h,
        )),
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeConnectorSnapshot {
    connected: bool,
    current_encoder: Option<drm::control::encoder::Handle>,
    encoders: Vec<drm::control::encoder::Handle>,
    mode_size: Option<Size>,
    native_mode: Option<drm::control::Mode>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeConnectorSnapshot {
    pub fn new(
        connected: bool,
        current_encoder: Option<drm::control::encoder::Handle>,
        encoders: impl IntoIterator<Item = drm::control::encoder::Handle>,
        mode_size: Option<Size>,
    ) -> Self {
        Self {
            connected,
            current_encoder,
            encoders: encoders.into_iter().collect(),
            mode_size,
            native_mode: None,
        }
    }

    pub fn new_with_native_mode(
        connected: bool,
        current_encoder: Option<drm::control::encoder::Handle>,
        encoders: impl IntoIterator<Item = drm::control::encoder::Handle>,
        mode: Option<drm::control::Mode>,
    ) -> Self {
        let mode_size = mode.map(|mode| {
            let (width, height) = mode.size();
            Size {
                width: i32::from(width),
                height: i32::from(height),
            }
        });
        Self {
            connected,
            current_encoder,
            encoders: encoders.into_iter().collect(),
            mode_size,
            native_mode: mode,
        }
    }

    fn ordered_encoders(&self) -> Vec<drm::control::encoder::Handle> {
        let mut handles = Vec::new();
        if let Some(current) = self.current_encoder {
            handles.push(current);
        }
        for encoder in self.encoders.iter().copied() {
            if !handles.contains(&encoder) {
                handles.push(encoder);
            }
        }
        handles
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeEncoderSnapshot {
    current_crtc: Option<drm::control::crtc::Handle>,
    compatible_crtcs: Vec<drm::control::crtc::Handle>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeEncoderSnapshot {
    pub fn new(
        current_crtc: Option<drm::control::crtc::Handle>,
        compatible_crtcs: impl IntoIterator<Item = drm::control::crtc::Handle>,
    ) -> Self {
        Self {
            current_crtc,
            compatible_crtcs: compatible_crtcs.into_iter().collect(),
        }
    }

    fn ordered_crtcs(&self) -> Vec<drm::control::crtc::Handle> {
        let mut handles = Vec::new();
        if let Some(current) = self.current_crtc {
            handles.push(current);
        }
        for crtc in self.compatible_crtcs.iter().copied() {
            if !handles.contains(&crtc) {
                handles.push(crtc);
            }
        }
        handles
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePlaneSnapshot {
    compatible_crtcs: Vec<drm::control::crtc::Handle>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePlaneSnapshot {
    pub fn new(compatible_crtcs: impl IntoIterator<Item = drm::control::crtc::Handle>) -> Self {
        Self {
            compatible_crtcs: compatible_crtcs.into_iter().collect(),
        }
    }

    fn supports_crtc(&self, crtc: drm::control::crtc::Handle) -> bool {
        self.compatible_crtcs.contains(&crtc)
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativeKmsSelectionDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>>;

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>>;

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot>;

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot>;

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>>;

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot>;

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativeKmsSelectionDevice for D
where
    D: drm::control::Device,
{
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        Ok(self.resource_handles()?.connectors().to_vec())
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        Ok(self.resource_handles()?.crtcs().to_vec())
    }

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        let info = self.get_connector(connector, false)?;
        let selected_mode = info
            .modes()
            .iter()
            .find(|mode| {
                mode.mode_type()
                    .contains(drm::control::ModeTypeFlags::PREFERRED)
            })
            .or_else(|| info.modes().first())
            .copied();
        Ok(LibdrmNativeConnectorSnapshot::new_with_native_mode(
            info.state() == drm::control::connector::State::Connected,
            info.current_encoder(),
            info.encoders().iter().copied(),
            selected_mode,
        ))
    }

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        let resources = self.resource_handles()?;
        let info = self.get_encoder(encoder)?;
        Ok(LibdrmNativeEncoderSnapshot::new(
            info.crtc(),
            resources.filter_crtcs(info.possible_crtcs()),
        ))
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        drm::control::Device::plane_handles(self)
    }

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        let resources = self.resource_handles()?;
        let info = self.get_plane(plane)?;
        Ok(LibdrmNativePlaneSnapshot::new(
            resources.filter_crtcs(info.possible_crtcs()),
        ))
    }

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        for (property, value) in self.get_properties(plane)?.iter() {
            let info = self.get_property(*property)?;
            if info
                .name()
                .to_str()
                .map(|name| name == "type")
                .unwrap_or(false)
            {
                return Ok(match *value as u32 {
                    x if x == drm::control::PlaneType::Primary as u32 => {
                        Some(drm::control::PlaneType::Primary)
                    }
                    x if x == drm::control::PlaneType::Overlay as u32 => {
                        Some(drm::control::PlaneType::Overlay)
                    }
                    x if x == drm::control::PlaneType::Cursor as u32 => {
                        Some(drm::control::PlaneType::Cursor)
                    }
                    _ => None,
                });
            }
        }
        Ok(None)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneSelectionResult {
    pub status: LibdrmNativePrimaryPlaneSelectionStatus,
    pub selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneSelectionStatus {
    Selected,
    ReadFailed,
    NoConnectedConnector,
    NoUsableMode,
    NoUsableEncoder,
    NoCompatibleCrtc,
    NoCompatiblePrimaryPlane,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneSelection {
    connector: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
    plane: drm::control::plane::Handle,
    size: Size,
    mode: Option<drm::control::Mode>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneSelection {
    pub const fn size(self) -> Size {
        self.size
    }

    pub const fn crtc_route(self, slot: LibdrmNativeOutputSlot) -> LibdrmNativeCrtcRoute {
        LibdrmNativeCrtcRoute::new(self.crtc, slot)
    }

    pub const fn into_objects(
        self,
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: u64,
    ) -> LibdrmNativePrimaryPlaneObjects {
        LibdrmNativePrimaryPlaneObjects::new(
            self.connector,
            self.crtc,
            self.plane,
            framebuffer,
            mode_blob,
            self.size,
        )
    }
}

#[cfg(feature = "libdrm-events")]
pub fn select_native_primary_plane_target<D>(device: &D) -> LibdrmNativePrimaryPlaneSelectionResult
where
    D: LibdrmNativeKmsSelectionDevice,
{
    let (Ok(connectors), Ok(crtcs), Ok(planes)) = (
        device.connector_handles(),
        device.crtc_handles(),
        device.plane_handles(),
    ) else {
        return LibdrmNativePrimaryPlaneSelectionResult {
            status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
            selection: None,
        };
    };

    let mut saw_connected = false;
    let mut saw_mode = false;
    let mut saw_encoder = false;
    let mut saw_crtc = false;

    for connector in connectors {
        let Ok(connector_snapshot) = device.connector_snapshot(connector) else {
            return LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
                selection: None,
            };
        };
        if !connector_snapshot.connected {
            continue;
        }
        saw_connected = true;
        let Some(size) = connector_snapshot.mode_size else {
            continue;
        };
        if size.width <= 0 || size.height <= 0 {
            continue;
        }
        saw_mode = true;

        for encoder in connector_snapshot.ordered_encoders() {
            saw_encoder = true;
            let Ok(encoder_snapshot) = device.encoder_snapshot(encoder) else {
                return LibdrmNativePrimaryPlaneSelectionResult {
                    status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
                    selection: None,
                };
            };
            for crtc in encoder_snapshot.ordered_crtcs() {
                if !crtcs.contains(&crtc) {
                    continue;
                }
                saw_crtc = true;
                let plane = match select_primary_plane_for_crtc(device, &planes, crtc) {
                    Ok(Some(plane)) => plane,
                    Ok(None) => continue,
                    Err(()) => {
                        return LibdrmNativePrimaryPlaneSelectionResult {
                            status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
                            selection: None,
                        };
                    }
                };
                return LibdrmNativePrimaryPlaneSelectionResult {
                    status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                    selection: Some(LibdrmNativePrimaryPlaneSelection {
                        connector,
                        crtc,
                        plane,
                        size,
                        mode: connector_snapshot.native_mode,
                    }),
                };
            }
        }
    }

    LibdrmNativePrimaryPlaneSelectionResult {
        status: if !saw_connected {
            LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
        } else if !saw_mode {
            LibdrmNativePrimaryPlaneSelectionStatus::NoUsableMode
        } else if !saw_encoder {
            LibdrmNativePrimaryPlaneSelectionStatus::NoUsableEncoder
        } else if !saw_crtc {
            LibdrmNativePrimaryPlaneSelectionStatus::NoCompatibleCrtc
        } else {
            LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
        },
        selection: None,
    }
}

#[cfg(feature = "libdrm-events")]
fn select_primary_plane_for_crtc<D>(
    device: &D,
    planes: &[drm::control::plane::Handle],
    crtc: drm::control::crtc::Handle,
) -> Result<Option<drm::control::plane::Handle>, ()>
where
    D: LibdrmNativeKmsSelectionDevice,
{
    for plane in planes.iter().copied() {
        let Ok(snapshot) = device.plane_snapshot(plane) else {
            return Err(());
        };
        if !snapshot.supports_crtc(crtc) {
            continue;
        }
        let Ok(plane_type) = device.plane_type(plane) else {
            return Err(());
        };
        if plane_type == Some(drm::control::PlaneType::Primary) {
            return Ok(Some(plane));
        }
    }
    Ok(None)
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64>;

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized;

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()>;

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePrimaryPlaneResourceDevice for D
where
    D: drm::control::Device,
{
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        let Some(mode) = selection.mode else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "selected KMS target does not carry a native mode",
            ));
        };
        match self.create_property_blob(&mode)? {
            drm::control::property::Value::Blob(blob) => Ok(blob),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "DRM mode blob creation returned a non-blob value",
            )),
        }
    }

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        self.add_framebuffer(buffer, depth, bpp)
    }

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.destroy_framebuffer(framebuffer)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.destroy_property_blob(mode_blob)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceBundle {
    framebuffer: drm::control::framebuffer::Handle,
    mode_blob: u64,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceBundle {
    pub const fn into_objects(
        self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> LibdrmNativePrimaryPlaneObjects {
        selection.into_objects(self.framebuffer, self.mode_blob)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmRendererScanoutBuffer {
    size: Size,
    pitch: u32,
    format: u32,
    handle: drm::buffer::Handle,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmRendererScanoutBuffer {
    pub fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Option<Self> {
        if descriptor.status != LiveRendererScanoutBufferStatus::Ready
            || descriptor.format != LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
            || descriptor.size.width <= 0
            || descriptor.size.height <= 0
            || descriptor.pitch == 0
        {
            return None;
        }

        Some(Self {
            size: descriptor.size,
            pitch: descriptor.pitch,
            format: descriptor.format,
            handle: drm::control::from_u32(descriptor.gem_handle)?,
        })
    }
}

#[cfg(feature = "libdrm-events")]
impl drm::buffer::Buffer for LibdrmRendererScanoutBuffer {
    fn size(&self) -> (u32, u32) {
        (self.size.width as u32, self.size.height as u32)
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        drm::buffer::DrmFourcc::Xrgb8888
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn handle(&self) -> drm::buffer::Handle {
        self.handle
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneResourceCreateResult {
    pub status: LibdrmNativePrimaryPlaneResourceCreateStatus,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceBundle>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceCreateStatus {
    Created,
    InvalidSelectionSize,
    BufferSizeMismatch,
    MissingMode,
    ModeBlobCreateFailed,
    FramebufferCreateFailed,
}

#[cfg(feature = "libdrm-events")]
pub fn create_native_primary_plane_resources<D, B>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    buffer: &B,
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + ?Sized,
{
    if selection.size.width <= 0 || selection.size.height <= 0 {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            resources: None,
        };
    }

    let (buffer_width, buffer_height) = buffer.size();
    if buffer_width != selection.size.width as u32 || buffer_height != selection.size.height as u32
    {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch,
            resources: None,
        };
    }

    let mode_blob = match device.create_mode_blob_for_selection(selection) {
        Ok(mode_blob) => mode_blob,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode,
                resources: None,
            };
        }
        Err(_) => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed,
                resources: None,
            };
        }
    };
    let Ok(framebuffer) = device.add_scanout_framebuffer(buffer, 24, 32) else {
        let _ = device.destroy_mode_blob(mode_blob);
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed,
            resources: None,
        };
    };

    LibdrmNativePrimaryPlaneResourceCreateResult {
        status: LibdrmNativePrimaryPlaneResourceCreateStatus::Created,
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle {
            framebuffer,
            mode_blob,
            size: selection.size,
        }),
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceDestroyReport {
    pub status: LibdrmNativePrimaryPlaneResourceDestroyStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceDestroyStatus {
    Destroyed,
    FramebufferDestroyFailed,
    ModeBlobDestroyFailed,
}

#[cfg(feature = "libdrm-events")]
pub fn destroy_native_primary_plane_resources<D>(
    device: &D,
    resources: LibdrmNativePrimaryPlaneResourceBundle,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    if device
        .destroy_scanout_framebuffer(resources.framebuffer)
        .is_err()
    {
        return LibdrmNativePrimaryPlaneResourceDestroyReport {
            status: LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed,
        };
    }
    if device.destroy_mode_blob(resources.mode_blob).is_err() {
        return LibdrmNativePrimaryPlaneResourceDestroyReport {
            status: LibdrmNativePrimaryPlaneResourceDestroyStatus::ModeBlobDestroyFailed,
        };
    }

    LibdrmNativePrimaryPlaneResourceDestroyReport {
        status: LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed,
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneObjects {
    connector: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
    plane: drm::control::plane::Handle,
    framebuffer: drm::control::framebuffer::Handle,
    mode_blob: u64,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneObjects {
    pub const fn new(
        connector: drm::control::connector::Handle,
        crtc: drm::control::crtc::Handle,
        plane: drm::control::plane::Handle,
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: u64,
        size: Size,
    ) -> Self {
        Self {
            connector,
            crtc,
            plane,
            framebuffer,
            mode_blob,
            size,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativeAtomicRequestBuildResult {
    pub status: LibdrmNativeAtomicRequestBuildStatus,
    pub request: Option<LibdrmNativeAtomicCommitRequest>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicRequestBuildStatus {
    Built,
    InvalidSize,
}

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_atomic_request(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
) -> LibdrmNativeAtomicRequestBuildResult {
    if objects.size.width <= 0 || objects.size.height <= 0 {
        return LibdrmNativeAtomicRequestBuildResult {
            status: LibdrmNativeAtomicRequestBuildStatus::InvalidSize,
            request: None,
        };
    }

    let width = objects.size.width as u64;
    let height = objects.size.height as u64;
    let mut request = drm::control::atomic::AtomicModeReq::new();
    request.add_property(
        objects.connector,
        properties.connector_crtc_id,
        drm::control::property::Value::CRTC(Some(objects.crtc)),
    );
    request.add_property(
        objects.crtc,
        properties.crtc_mode_id,
        drm::control::property::Value::Blob(objects.mode_blob),
    );
    request.add_property(
        objects.crtc,
        properties.crtc_active,
        drm::control::property::Value::Boolean(true),
    );
    request.add_property(
        objects.plane,
        properties.plane_fb_id,
        drm::control::property::Value::Framebuffer(Some(objects.framebuffer)),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_id,
        drm::control::property::Value::CRTC(Some(objects.crtc)),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_x,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_y,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_w,
        drm::control::property::Value::UnsignedRange(width << 16),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_h,
        drm::control::property::Value::UnsignedRange(height << 16),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_x,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_y,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_w,
        drm::control::property::Value::UnsignedRange(width),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_h,
        drm::control::property::Value::UnsignedRange(height),
    );

    LibdrmNativeAtomicRequestBuildResult {
        status: LibdrmNativeAtomicRequestBuildStatus::Built,
        request: Some(LibdrmNativeAtomicCommitRequest::new(request)),
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativeAtomicCommitDevice {
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativeAtomicCommitDevice for D
where
    D: drm::control::Device,
{
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.atomic_commit(flags, request)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct NativeLibdrmAtomicScanoutCommitter<D> {
    device: D,
    submitted: usize,
    rejected: usize,
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmAtomicScanoutCommitter<D> {
    pub const fn new(device: D) -> Self {
        Self {
            device,
            submitted: 0,
            rejected: 0,
        }
    }

    pub const fn submitted_count(&self) -> usize {
        self.submitted
    }

    pub const fn rejected_count(&self) -> usize {
        self.rejected
    }
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmAtomicScanoutCommitter<D>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    pub fn submit_native_atomic_commit(
        &mut self,
        request: LibdrmNativeAtomicCommitRequest,
    ) -> LibdrmNativeAtomicCommitSubmitReport {
        let (flags, request) = request.into_native();
        match self.device.submit_atomic_commit(flags, request) {
            Ok(()) => {
                self.submitted = self.submitted.saturating_add(1);
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::WouldBlock,
                }
            }
            Err(_) => {
                self.rejected = self.rejected.saturating_add(1);
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::Rejected,
                }
            }
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmission {
    resources: LibdrmNativePrimaryPlaneResourceBundle,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneScanoutSubmission {
    pub fn retire<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resources(device, self.resources)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitResult {
    pub status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
    pub selection: LibdrmNativePrimaryPlaneSelectionStatus,
    pub scanout_buffer: LiveRendererScanoutBufferStatus,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub submit: Option<LibdrmNativeAtomicCommitSubmitStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    KmsTargetUnavailable,
    ScanoutBufferUnavailable,
    PropertyDiscoveryUnavailable,
    ResourceCreationUnavailable,
    AtomicRequestBuildFailed,
    AtomicSubmitFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutRetireResult {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutRetireStatus {
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

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

        let report =
            NativeGbmScanoutBufferExporter::export_rendered_owned_scanout_buffer_from_backend_device_result(
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderedPrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    FrameTargetUnavailable,
    ScanoutExportFailed,
    PrimaryPlaneSubmitFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutSubmission<Owner> {
    scanout_buffer: Owner,
    primary_plane: LibdrmNativePrimaryPlaneScanoutSubmission,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutSubmission<Owner> {
    pub fn into_scanout_buffer(self) -> Owner {
        self.scanout_buffer
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutRetireResult<Owner> {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LiveRenderedPrimaryPlaneScanoutSubmission<Owner>>,
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
    let owner = submission.scanout_buffer;
    let retired = retire_native_primary_plane_scanout_after_page_flip(
        device,
        submission.primary_plane,
        callback,
    );
    let submission =
        retired
            .submission
            .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutSubmission {
                scanout_buffer: owner,
                primary_plane,
            });

    LiveRenderedPrimaryPlaneScanoutRetireResult {
        status: retired.status,
        destroy: retired.destroy,
        submission,
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicScanoutSmokeEvidence {
    pub status: LibdrmNativeAtomicScanoutSmokeStatus,
    pub gbm_export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub page_flip_poll: Option<LibdrmPageFlipEventPollStatus>,
    pub page_flip: Option<LivePageFlipEventStatus>,
    pub retire: Option<LibdrmNativePrimaryPlaneScanoutRetireStatus>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicScanoutSmokeEvidence {
    pub const fn no_primary_card() -> Self {
        Self {
            status: LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard,
            gbm_export: None,
            submit: None,
            page_flip_poll: None,
            page_flip: None,
            retire: None,
        }
    }

    pub const fn kms_selection_failed() -> Self {
        Self {
            status: LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed,
            gbm_export: None,
            submit: None,
            page_flip_poll: None,
            page_flip: None,
            retire: None,
        }
    }

    pub fn from_pipeline_reports(
        gbm_export: LiveRendererScanoutBufferExportStatus,
        submit: Option<&LibdrmNativePrimaryPlaneScanoutSubmitResult>,
        poll: Option<&LibdrmPageFlipEventPollReport>,
        callback: Option<&LivePageFlipCallbackReport>,
        retire: Option<&LibdrmNativePrimaryPlaneScanoutRetireResult>,
    ) -> Self {
        let submit_status = submit.map(|report| report.status);
        let page_flip_poll = poll.map(|report| report.status);
        let page_flip = callback.map(|report| report.event.status);
        let accepted_page_flip =
            callback.map(|report| report.decision) == Some(LivePageFlipCallbackDecision::Accepted);
        let retire_status = retire.map(|report| report.status);

        let status = if gbm_export != LiveRendererScanoutBufferExportStatus::Exported {
            LibdrmNativeAtomicScanoutSmokeStatus::GbmExportFailed
        } else if submit_status
            != Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::SubmitFailed
        } else if !accepted_page_flip
            || page_flip_poll != Some(LibdrmPageFlipEventPollStatus::Emitted)
            || page_flip != Some(LivePageFlipEventStatus::Presented)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::PageFlipMissing
        } else if retire_status
            != Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
        } else {
            LibdrmNativeAtomicScanoutSmokeStatus::Passed
        };

        Self {
            status,
            gbm_export: Some(gbm_export),
            submit: submit_status,
            page_flip_poll,
            page_flip,
            retire: retire_status,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokeStatus {
    Passed,
    NoPrimaryCard,
    KmsSelectionFailed,
    GbmExportFailed,
    SubmitFailed,
    PageFlipMissing,
    RetireFailed,
}

#[cfg(feature = "libdrm-events")]
pub fn submit_native_primary_plane_scanout_from_renderer_descriptor<D>(
    device: &D,
    descriptor: LiveRendererScanoutBufferDescriptor,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    let selection = select_native_primary_plane_target(device);
    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: None,
            resources: None,
            request: None,
            submit: None,
            submission: None,
        };
    };

    let Some(buffer) = LibdrmRendererScanoutBuffer::from_descriptor(descriptor) else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: None,
            resources: None,
            request: None,
            submit: None,
            submission: None,
        };
    };

    let properties = discover_native_primary_plane_property_handles(
        device,
        selected.connector,
        selected.crtc,
        selected.plane,
    );
    let Some(property_handles) = properties.properties else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::PropertyDiscoveryUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: None,
            request: None,
            submit: None,
            submission: None,
        };
    };

    let resources = create_native_primary_plane_resources(device, selected, &buffer);
    let Some(resource_bundle) = resources.resources else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: None,
            submit: None,
            submission: None,
        };
    };

    let request = build_native_primary_plane_atomic_request(
        resource_bundle.into_objects(selected),
        property_handles,
    );
    let Some(request) = request.request else {
        let _ = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(request.status),
            submit: None,
            submission: None,
        };
    };

    let request = request.allow_modeset();
    let (flags, request) = request.into_native();
    let submit = match device.submit_atomic_commit(flags, request) {
        Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
        }
        Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
    };

    if submit != LibdrmNativeAtomicCommitSubmitStatus::Submitted {
        let _ = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
            submit: Some(submit),
            submission: None,
        };
    }

    LibdrmNativePrimaryPlaneScanoutSubmitResult {
        status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        selection: selection.status,
        scanout_buffer: descriptor.status,
        properties: Some(properties.status),
        resources: Some(resources.status),
        request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
        submit: Some(submit),
        submission: Some(LibdrmNativePrimaryPlaneScanoutSubmission {
            resources: resource_bundle,
        }),
    }
}

#[cfg(feature = "libdrm-events")]
pub fn retire_native_primary_plane_scanout_after_page_flip<D>(
    device: &D,
    submission: LibdrmNativePrimaryPlaneScanoutSubmission,
    callback: &LivePageFlipCallbackReport,
) -> LibdrmNativePrimaryPlaneScanoutRetireResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    if callback.decision != LivePageFlipCallbackDecision::Accepted
        || callback.event.status != LivePageFlipEventStatus::Presented
    {
        return LibdrmNativePrimaryPlaneScanoutRetireResult {
            status: LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
            destroy: None,
            submission: Some(submission),
        };
    }

    let destroy = submission.retire(device);
    if destroy.status != LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed {
        return LibdrmNativePrimaryPlaneScanoutRetireResult {
            status: LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed,
            destroy: Some(destroy.status),
            submission: None,
        };
    }

    LibdrmNativePrimaryPlaneScanoutRetireResult {
        status: LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
        destroy: Some(destroy.status),
        submission: None,
    }
}

#[cfg(feature = "libdrm-events")]
impl<D> LiveAtomicScanoutCommitter for NativeLibdrmAtomicScanoutCommitter<D> {
    fn commit_atomic_scanout(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome)
    }

    fn commit_atomic_scanout_after_page_flip(
        &mut self,
        callback: &LivePageFlipCallbackReport,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        LiveAtomicScanoutCommitReport::from_page_flip_callback_and_outcome(callback, outcome)
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

    #[cfg(feature = "libdrm-events")]
    pub fn submit_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
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
        let Some(target) = self.gbm_egl_frame_target else {
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
        if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
        {
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
            }),
        }
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

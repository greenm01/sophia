//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};
#[cfg(feature = "gbm-probe")]
use std::{io, os::fd::AsFd};

pub use sophia_engine::{
    BufferImportPath, CompositorBackendAssemblyError, CompositorBackendTickInput,
    CompositorBackendTickReport, DrmKmsOutputRegistry, HeadlessCompositorBackendAssembly,
    HeadlessOutput, LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventSource,
    LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus,
    PageFlipCommitOutcome, QueuedInputPoller, RendererSelection,
};
use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};
pub use sophia_protocol::{BufferSource, DeviceId, OutputId, SeatId, Size};
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
#[cfg(feature = "gbm-probe")]
pub use sophia_renderer_live::{GbmCapabilityProbeReport, NativeGbmCapabilityProbe};
pub use sophia_renderer_live::{
    LiveRendererImportBoundary, LiveRendererImportDecision, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportRejection, LiveRendererImportStartupStatus,
    LiveRendererPresentationReport, LiveRendererPresentationStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
use sophia_renderer_live::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglPlatformProbe,
    NativeGbmBackedEglPresentationSmoke,
};

pub const LIVE_PAGE_FLIP_CALLBACK_CHANNEL_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyKind {
    LibDrm,
    LibInput,
    Gbm,
    Egl,
    DmaBuf,
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
    use LiveBackendDependencyKind::{DmaBuf, Egl, Gbm, LibDrm, LibInput, MitShm};
    use LiveBackendDependencyUse::{Discovery, RendererImport, RuntimePolling, SharedMemoryImport};

    match (kind, use_case) {
        (LibDrm | LibInput, Discovery | RuntimePolling) => Allowed,
        (MitShm, _) | (_, SharedMemoryImport) => Deferred {
            required_boundary: "bounded shared-memory import boundary",
        },
        (Gbm | Egl | DmaBuf, _) | (_, RendererImport) => Deferred {
            required_boundary: "live renderer import boundary",
        },
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
    PresentationUnavailable,
    Degraded,
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

    pub fn into_configured_headless_assembly(
        self,
        poller: QueuedInputPoller,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        let renderer = self.try_renderer_selection()?;
        self.into_headless_assembly(poller, renderer)
    }

    pub fn into_live_runtime_assembly(
        self,
        poller: QueuedInputPoller,
    ) -> Option<LiveBackendRuntimeAssembly> {
        let renderer_status =
            self.renderer_runtime_status_for_preference(self.renderer_import_status());
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_configured_headless_assembly_with_gbm_device<D>(
        self,
        poller: QueuedInputPoller,
        discovery: &D,
    ) -> Option<HeadlessCompositorBackendAssembly>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        let renderer = self.renderer_selection_for_status(renderer_status)?;
        self.into_headless_assembly(poller, renderer)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_live_runtime_assembly_with_gbm_device<D>(
        self,
        poller: QueuedInputPoller,
        discovery: &D,
    ) -> Option<LiveBackendRuntimeAssembly>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    pub fn into_headless_assembly(
        self,
        poller: QueuedInputPoller,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        self.discovery.into_headless_assembly(poller, renderer)
    }

    fn into_live_runtime_assembly_with_status(
        self,
        poller: QueuedInputPoller,
        renderer_status: LiveRendererImportStartupStatus,
    ) -> Option<LiveBackendRuntimeAssembly> {
        let renderer_selection = self.renderer_selection_for_status(renderer_status)?;
        let selected_output = self.selected_output()?;
        let renderer_observation = LiveRendererRuntimeObservation::from_startup_status(
            renderer_status,
            selection_observation(renderer_selection),
        );
        let scanout_readiness = self.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        });
        let page_flip_event = LivePageFlipEvent::from_scanout_status(scanout_readiness.status);
        let page_flip_callback_intake = LivePageFlipCallbackIntake::new(selected_output.id);
        self.into_headless_assembly(poller, renderer_selection)
            .map(|assembly| LiveBackendRuntimeAssembly {
                assembly,
                renderer_observation,
                scanout_readiness,
                page_flip_event,
                page_flip_callback_intake,
                page_flip_callback_queue: None,
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

pub struct LiveBackendRuntimeAssembly {
    assembly: HeadlessCompositorBackendAssembly,
    renderer_observation: LiveRendererRuntimeObservation,
    scanout_readiness: LiveScanoutReadinessReport,
    page_flip_event: LivePageFlipEvent,
    page_flip_callback_intake: LivePageFlipCallbackIntake,
    page_flip_callback_queue: Option<LivePageFlipCallbackQueue>,
}

impl LiveBackendRuntimeAssembly {
    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
    }

    pub fn with_page_flip_callback_queue(mut self, queue: LivePageFlipCallbackQueue) -> Self {
        self.page_flip_callback_queue = Some(queue);
        self
    }

    pub fn scanout_readiness_observation(&self) -> LiveScanoutReadinessReport {
        self.scanout_readiness
    }

    pub fn page_flip_observation(&self) -> LivePageFlipEvent {
        self.page_flip_event
    }

    pub fn observe_presentation_report(&mut self, presentation: LiveRendererPresentationReport) {
        self.scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        self.page_flip_event =
            LivePageFlipEvent::from_scanout_status(self.scanout_readiness.status);
    }

    pub fn observe_page_flip_outcome(&mut self, outcome: &PageFlipCommitOutcome) {
        self.page_flip_event = LivePageFlipEvent::from_commit_outcome(outcome);
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
            page_flip: self.page_flip_event,
            page_flip_callbacks,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeTickReport {
    pub engine: CompositorBackendTickReport,
    pub renderer: LiveRendererRuntimeObservation,
    pub scanout: LiveScanoutReadinessReport,
    pub page_flip: LivePageFlipEvent,
    pub page_flip_callbacks: LivePageFlipCallbackQueueReport,
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

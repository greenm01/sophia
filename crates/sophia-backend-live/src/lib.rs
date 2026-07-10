//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

#[cfg(feature = "libdrm-events")]
mod native_atomic;
#[cfg(feature = "libdrm-events")]
mod native_kms;
#[cfg(feature = "libdrm-events")]
mod native_page_flip;
#[cfg(feature = "libdrm-events")]
mod native_primary_plane;
#[cfg(feature = "libdrm-events")]
mod native_scanout;
mod page_flip;
#[cfg(feature = "libdrm-events")]
mod rendered_scanout;

#[cfg(any(feature = "libdrm-events", feature = "libinput-events"))]
use std::collections::VecDeque;
#[cfg(any(
    feature = "gbm-probe",
    feature = "libdrm-events",
    feature = "libinput-events"
))]
use std::io;
#[cfg(feature = "gbm-probe")]
use std::os::fd::AsFd;

#[cfg(feature = "libdrm-events")]
pub use native_atomic::*;
#[cfg(feature = "libdrm-events")]
pub use native_kms::*;
#[cfg(feature = "libdrm-events")]
pub use native_page_flip::*;
#[cfg(feature = "libdrm-events")]
pub use native_primary_plane::*;
#[cfg(feature = "libdrm-events")]
pub use native_scanout::*;
pub use page_flip::*;
#[cfg(feature = "libdrm-events")]
pub use rendered_scanout::*;
pub use sophia_engine::{
    BufferImportPath, CompositorBackendAssemblyError, CompositorBackendTickInput,
    CompositorBackendTickReport, DrmKmsOutputRegistry, HeadlessCompositorBackendAssembly,
    HeadlessEngine, HeadlessOutput, LastCommittedLayout, LibinputDeviceDescriptor,
    LibinputDeviceKind, LibinputEventIngest, LibinputEventSource, LibinputPhysicalInputAdapter,
    LibinputPollReport, LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus,
    LiveRuntimeDriverAdapter, LiveRuntimeDriverIntake, NonBlockingInputPoller,
    PageFlipCommitOutcome, QueuedInputPoller, RendererSelection, RuntimeDriverAdapter,
    RuntimeScanoutState, SessionRuntimeObservation, SessionTickReport,
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
pub use sophia_renderer_live::{
    GbmCapabilityProbeReport, NativeGbmCapabilityProbe, NativeGbmRenderedScanoutContextStatus,
};
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
mod dependency;
mod hardware_validation;
#[cfg(feature = "libinput-events")]
mod libinput;
mod runtime;
mod scanout_status;
mod startup;

pub use dependency::*;
pub use hardware_validation::*;
#[cfg(feature = "libinput-events")]
pub use libinput::*;
pub use runtime::*;
pub use scanout_status::*;
pub use startup::*;

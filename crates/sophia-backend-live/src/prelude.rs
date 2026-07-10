#![allow(unused_imports)]

#[cfg(any(feature = "libdrm-events", feature = "libinput-events"))]
pub(crate) use std::collections::VecDeque;
#[cfg(any(
    feature = "gbm-probe",
    feature = "libdrm-events",
    feature = "libinput-events"
))]
pub(crate) use std::io;
#[cfg(feature = "gbm-probe")]
pub(crate) use std::os::fd::AsFd;

pub(crate) use crate::api::*;
pub(crate) use crate::drm::*;
pub(crate) use crate::runtime::*;
pub(crate) use crate::scanout::*;
#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
pub(crate) use crate::session_loop::*;
pub(crate) use crate::startup::*;

pub(crate) use sophia_protocol::TransactionOutcome;
#[cfg(feature = "gbm-probe")]
pub(crate) use sophia_renderer_live::GbmCapabilityProbeStatus;
#[cfg(feature = "egl-probe")]
pub(crate) use sophia_renderer_live::{
    EglCapabilityProbeStatus, FakeEglCapabilityProbe, NativeEglCapabilityProbe, NativeEglDrawSmoke,
};
#[cfg(feature = "libdrm-events")]
pub(crate) use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
    LiveRendererScanoutBufferDescriptor, LiveRendererScanoutBufferExportDetail,
    LiveRendererScanoutBufferExportStatus, LiveRendererScanoutBufferStatus,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
pub(crate) use sophia_renderer_live::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglFrameTargetAllocator,
    NativeGbmBackedEglPlatformProbe, NativeGbmBackedEglPresentationSmoke,
};

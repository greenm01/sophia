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
#[cfg(feature = "libdrm-events")]
pub(crate) use crate::native_atomic::*;
#[cfg(feature = "libdrm-events")]
pub(crate) use crate::native_kms::*;
#[cfg(feature = "libdrm-events")]
pub(crate) use crate::native_page_flip::*;
#[cfg(feature = "libdrm-events")]
pub(crate) use crate::native_primary_plane::*;
#[cfg(feature = "libdrm-events")]
pub(crate) use crate::native_scanout::*;
pub(crate) use crate::page_flip::*;
#[cfg(feature = "libdrm-events")]
pub(crate) use crate::rendered_scanout::*;
pub(crate) use crate::runtime::*;
pub(crate) use crate::scanout_status::*;
pub(crate) use crate::startup::*;

#[cfg(feature = "gbm-probe")]
pub(crate) use sophia_renderer_live::GbmCapabilityProbeStatus;
#[cfg(feature = "egl-probe")]
pub(crate) use sophia_renderer_live::{
    EglCapabilityProbeStatus, FakeEglCapabilityProbe, NativeEglCapabilityProbe, NativeEglDrawSmoke,
};
#[cfg(feature = "libdrm-events")]
pub(crate) use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportStatus, LiveRendererScanoutBufferStatus,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
pub(crate) use sophia_renderer_live::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglFrameTargetAllocator,
    NativeGbmBackedEglPlatformProbe, NativeGbmBackedEglPresentationSmoke,
};

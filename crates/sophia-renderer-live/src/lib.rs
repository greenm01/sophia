//! Live renderer boundary.
//!
//! This crate is the future home for renderer-private resources such as GBM,
//! EGL, DMA-BUF import, explicit sync fences, and upload caches. Public types
//! stay reduced so backend-live can prove scanout behavior without leaking
//! native renderer identity into the engine.

pub use sophia_engine::BufferImportPath;
pub use sophia_protocol::{BufferSource, Size};

mod frame_target;
mod import;
mod presentation;
mod scanout_buffer;

#[cfg(feature = "egl-probe")]
mod egl_probe;
#[cfg(feature = "gbm-probe")]
mod gbm_probe;
#[cfg(feature = "gbm-probe")]
mod native_scanout;

pub use frame_target::*;
pub use import::*;
pub use presentation::*;
pub use scanout_buffer::*;

#[cfg(feature = "egl-probe")]
pub use egl_probe::{
    EglCapabilityProbeReport, EglCapabilityProbeStatus, EglContextProbeStatus, EglDrawSmokeReport,
    EglDrawSmokeStatus, EglPlatformStatus, FakeEglCapabilityProbe, FakeEglDrawSmoke,
    NativeEglCapabilityProbe, NativeEglDrawSmoke,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
pub use egl_probe::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglFrameTargetAllocator,
    NativeGbmBackedEglPlatformProbe, NativeGbmBackedEglPresentationSmoke,
};

#[cfg(feature = "gbm-probe")]
pub use gbm_probe::{
    FakeGbmCapabilityProbe, GbmCapabilityProbeReport, GbmCapabilityProbeStatus,
    GbmRenderDeviceToken, NativeGbmCapabilityProbe,
};
#[cfg(feature = "gbm-probe")]
pub use native_scanout::*;

pub const LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888: u32 = 875_713_089;
pub const LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888: u32 = 875_713_112;

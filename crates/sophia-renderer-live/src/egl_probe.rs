#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeEglCapabilityProbe {
    pub platform: EglPlatformStatus,
    pub context: EglContextProbeStatus,
}

impl FakeEglCapabilityProbe {
    pub const fn new(platform: EglPlatformStatus, context: EglContextProbeStatus) -> Self {
        Self { platform, context }
    }

    pub const fn probe_report(self) -> EglCapabilityProbeReport {
        EglCapabilityProbeReport {
            status: match (self.platform, self.context) {
                (EglPlatformStatus::NativePlatformCapable, EglContextProbeStatus::Available) => {
                    EglCapabilityProbeStatus::NativeDrawingCapable
                }
                (EglPlatformStatus::NativePlatformCapable, EglContextProbeStatus::Unavailable) => {
                    EglCapabilityProbeStatus::ContextUnavailable
                }
                (EglPlatformStatus::PlatformUnavailable, _) => {
                    EglCapabilityProbeStatus::PlatformUnavailable
                }
                (EglPlatformStatus::PlatformDegraded, _) => {
                    EglCapabilityProbeStatus::PlatformDegraded
                }
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEglCapabilityProbe;

impl NativeEglCapabilityProbe {
    pub fn probe_report() -> EglCapabilityProbeReport {
        report_from_probe_result(native::probe())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeEglDrawSmoke {
    pub status: EglDrawSmokeStatus,
}

impl FakeEglDrawSmoke {
    pub const fn new(status: EglDrawSmokeStatus) -> Self {
        Self { status }
    }

    pub const fn smoke_report(self) -> EglDrawSmokeReport {
        EglDrawSmokeReport {
            status: self.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEglDrawSmoke;

impl NativeEglDrawSmoke {
    pub fn smoke_report() -> EglDrawSmokeReport {
        draw_report_from_smoke_result(native::draw_smoke())
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmBackedEglPlatformProbe;

#[cfg(feature = "gbm-probe")]
impl NativeGbmBackedEglPlatformProbe {
    pub fn platform_status_from_backend_device_result<T: std::os::fd::AsFd>(
        device: std::io::Result<T>,
    ) -> EglPlatformStatus {
        native::gbm_backed_platform_status_from_backend_device_result(device)
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmBackedEglDrawSmoke;

#[cfg(feature = "gbm-probe")]
impl NativeGbmBackedEglDrawSmoke {
    pub fn smoke_report_from_backend_device_result<T: std::os::fd::AsFd>(
        device: std::io::Result<T>,
    ) -> EglDrawSmokeReport {
        draw_report_from_smoke_result(
            native::gbm_backed_draw_smoke_result_from_backend_device_result(device),
        )
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmBackedEglPresentationSmoke;

#[cfg(feature = "gbm-probe")]
impl NativeGbmBackedEglPresentationSmoke {
    pub fn smoke_report_from_backend_device_result<T: std::os::fd::AsFd>(
        device: std::io::Result<T>,
    ) -> crate::LiveRendererPresentationReport {
        crate::LiveRendererPresentationReport {
            status: native::gbm_backed_presentation_status_from_backend_device_result(device),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EglCapabilityProbeReport {
    pub status: EglCapabilityProbeStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EglDrawSmokeReport {
    pub status: EglDrawSmokeStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglCapabilityProbeStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglPlatformStatus {
    NativePlatformCapable,
    PlatformUnavailable,
    PlatformDegraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglContextProbeStatus {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglDrawSmokeStatus {
    ClearColorReady,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
    SurfaceUnavailable,
    MakeCurrentUnavailable,
    GlUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EglProbeResult {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EglDrawSmokeResult {
    ClearColorReady,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
    SurfaceUnavailable,
    MakeCurrentUnavailable,
    GlUnavailable,
}

fn report_from_probe_result(result: EglProbeResult) -> EglCapabilityProbeReport {
    EglCapabilityProbeReport {
        status: match result {
            EglProbeResult::NativeDrawingCapable => EglCapabilityProbeStatus::NativeDrawingCapable,
            EglProbeResult::PlatformUnavailable => EglCapabilityProbeStatus::PlatformUnavailable,
            EglProbeResult::PlatformDegraded => EglCapabilityProbeStatus::PlatformDegraded,
            EglProbeResult::ContextUnavailable => EglCapabilityProbeStatus::ContextUnavailable,
        },
    }
}

fn draw_report_from_smoke_result(result: EglDrawSmokeResult) -> EglDrawSmokeReport {
    EglDrawSmokeReport {
        status: match result {
            EglDrawSmokeResult::ClearColorReady => EglDrawSmokeStatus::ClearColorReady,
            EglDrawSmokeResult::PlatformUnavailable => EglDrawSmokeStatus::PlatformUnavailable,
            EglDrawSmokeResult::PlatformDegraded => EglDrawSmokeStatus::PlatformDegraded,
            EglDrawSmokeResult::ContextUnavailable => EglDrawSmokeStatus::ContextUnavailable,
            EglDrawSmokeResult::SurfaceUnavailable => EglDrawSmokeStatus::SurfaceUnavailable,
            EglDrawSmokeResult::MakeCurrentUnavailable => {
                EglDrawSmokeStatus::MakeCurrentUnavailable
            }
            EglDrawSmokeResult::GlUnavailable => EglDrawSmokeStatus::GlUnavailable,
        },
    }
}

mod native {
    use super::{EglDrawSmokeResult, EglProbeResult};

    pub(super) fn probe() -> EglProbeResult {
        match sophia_renderer_native_egl::probe_default_display_context() {
            sophia_renderer_native_egl::NativeEglProbeStatus::NativeDrawingCapable => {
                EglProbeResult::NativeDrawingCapable
            }
            sophia_renderer_native_egl::NativeEglProbeStatus::PlatformUnavailable => {
                EglProbeResult::PlatformUnavailable
            }
            sophia_renderer_native_egl::NativeEglProbeStatus::PlatformDegraded => {
                EglProbeResult::PlatformDegraded
            }
            sophia_renderer_native_egl::NativeEglProbeStatus::ContextUnavailable => {
                EglProbeResult::ContextUnavailable
            }
        }
    }

    pub(super) fn draw_smoke() -> EglDrawSmokeResult {
        match sophia_renderer_native_egl::smoke_default_display_pbuffer() {
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::ClearColorReady => {
                EglDrawSmokeResult::ClearColorReady
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::PlatformUnavailable => {
                EglDrawSmokeResult::PlatformUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::PlatformDegraded => {
                EglDrawSmokeResult::PlatformDegraded
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::ContextUnavailable => {
                EglDrawSmokeResult::ContextUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::SurfaceUnavailable => {
                EglDrawSmokeResult::SurfaceUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::MakeCurrentUnavailable => {
                EglDrawSmokeResult::MakeCurrentUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::GlUnavailable => {
                EglDrawSmokeResult::GlUnavailable
            }
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub(super) fn gbm_backed_platform_status_from_backend_device_result<T: std::os::fd::AsFd>(
        device: std::io::Result<T>,
    ) -> super::EglPlatformStatus {
        match sophia_renderer_native_egl::probe_gbm_backed_platform_from_backend_device_result(
            device,
        ) {
            sophia_renderer_native_egl::NativeGbmBackedEglPlatformStatus::NativePlatformCapable => {
                super::EglPlatformStatus::NativePlatformCapable
            }
            sophia_renderer_native_egl::NativeGbmBackedEglPlatformStatus::PlatformUnavailable => {
                super::EglPlatformStatus::PlatformUnavailable
            }
            sophia_renderer_native_egl::NativeGbmBackedEglPlatformStatus::PlatformDegraded => {
                super::EglPlatformStatus::PlatformDegraded
            }
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub(super) fn gbm_backed_draw_smoke_result_from_backend_device_result<T: std::os::fd::AsFd>(
        device: std::io::Result<T>,
    ) -> EglDrawSmokeResult {
        match sophia_renderer_native_egl::smoke_gbm_backed_private_target_from_backend_device_result(
            device,
        ) {
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::ClearColorReady => {
                EglDrawSmokeResult::ClearColorReady
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::PlatformUnavailable => {
                EglDrawSmokeResult::PlatformUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::PlatformDegraded => {
                EglDrawSmokeResult::PlatformDegraded
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::ContextUnavailable => {
                EglDrawSmokeResult::ContextUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::SurfaceUnavailable => {
                EglDrawSmokeResult::SurfaceUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::MakeCurrentUnavailable => {
                EglDrawSmokeResult::MakeCurrentUnavailable
            }
            sophia_renderer_native_egl::NativeEglDrawSmokeStatus::GlUnavailable => {
                EglDrawSmokeResult::GlUnavailable
            }
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub(super) fn gbm_backed_presentation_status_from_backend_device_result<
        T: std::os::fd::AsFd,
    >(
        device: std::io::Result<T>,
    ) -> crate::LiveRendererPresentationStatus {
        match sophia_renderer_native_egl::present_gbm_backed_offscreen_from_backend_device_result(
            device,
        ) {
            sophia_renderer_native_egl::NativePresentationSmokeStatus::Ready => {
                crate::LiveRendererPresentationStatus::Ready
            }
            sophia_renderer_native_egl::NativePresentationSmokeStatus::Unavailable => {
                crate::LiveRendererPresentationStatus::Unavailable
            }
            sophia_renderer_native_egl::NativePresentationSmokeStatus::Degraded => {
                crate::LiveRendererPresentationStatus::Degraded
            }
        }
    }
}

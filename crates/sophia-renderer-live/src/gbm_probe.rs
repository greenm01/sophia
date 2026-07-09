use crate::{LiveRendererImportPathStatus, LiveRendererImportStartupStatus};
use std::os::fd::AsFd;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GbmRenderDeviceToken {
    pub raw: u64,
}

impl GbmRenderDeviceToken {
    pub const fn from_raw(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self { raw }) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeGbmCapabilityProbe {
    pub device: Option<GbmRenderDeviceToken>,
}

impl FakeGbmCapabilityProbe {
    pub const fn new(device: Option<GbmRenderDeviceToken>) -> Self {
        Self { device }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        status_from_probe_result(fake_probe(self.device))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmCapabilityProbe {
    pub device: Option<GbmRenderDeviceToken>,
}

impl NativeGbmCapabilityProbe {
    pub const fn new(device: Option<GbmRenderDeviceToken>) -> Self {
        Self { device }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        status_from_probe_result(native::probe(self.device))
    }

    pub fn startup_status_from_backend_device<T: AsFd>(
        device: T,
    ) -> LiveRendererImportStartupStatus {
        status_from_probe_result(native::probe_backend_device(device))
    }

    pub fn startup_status_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
    ) -> LiveRendererImportStartupStatus {
        status_from_probe_result(match device {
            Ok(device) => native::probe_backend_device(device),
            Err(_error) => GbmProbeResult::NativeError,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GbmProbeResult {
    Capable,
    Unavailable,
    NativeError,
}

fn fake_probe(device: Option<GbmRenderDeviceToken>) -> GbmProbeResult {
    if device.is_some() {
        GbmProbeResult::Capable
    } else {
        GbmProbeResult::Unavailable
    }
}

fn status_from_probe_result(result: GbmProbeResult) -> LiveRendererImportStartupStatus {
    LiveRendererImportStartupStatus::from_path_statuses(
        LiveRendererImportPathStatus::Disabled,
        match result {
            GbmProbeResult::Capable => LiveRendererImportPathStatus::Enabled,
            GbmProbeResult::Unavailable | GbmProbeResult::NativeError => {
                LiveRendererImportPathStatus::Degraded
            }
        },
    )
}

mod native {
    use super::{GbmProbeResult, GbmRenderDeviceToken};
    use std::os::fd::AsFd;

    pub(super) fn probe(device: Option<GbmRenderDeviceToken>) -> GbmProbeResult {
        let Some(_device) = device else {
            return GbmProbeResult::Unavailable;
        };

        let _format = gbm::Format::Argb8888;
        let _usage = gbm::BufferObjectFlags::RENDERING;
        GbmProbeResult::Capable
    }

    pub(super) fn probe_backend_device<T: AsFd>(device: T) -> GbmProbeResult {
        match gbm::Device::new(device) {
            Ok(device) if supports_first_render_target(&device) => GbmProbeResult::Capable,
            Ok(_device) => GbmProbeResult::NativeError,
            Err(_error) => GbmProbeResult::NativeError,
        }
    }

    fn supports_first_render_target<T: AsFd>(device: &gbm::Device<T>) -> bool {
        device.is_format_supported(gbm::Format::Argb8888, gbm::BufferObjectFlags::RENDERING)
    }
}

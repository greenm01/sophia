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
        self.probe_report().startup_status
    }

    pub fn probe_report(self) -> GbmCapabilityProbeReport {
        report_from_probe_result(fake_probe(self.device))
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
        self.probe_report().startup_status
    }

    pub fn probe_report(self) -> GbmCapabilityProbeReport {
        report_from_probe_result(native::probe(self.device))
    }

    pub fn startup_status_from_backend_device<T: AsFd>(
        device: T,
    ) -> LiveRendererImportStartupStatus {
        Self::probe_report_from_backend_device(device).startup_status
    }

    pub fn startup_status_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
    ) -> LiveRendererImportStartupStatus {
        Self::probe_report_from_backend_device_result(device).startup_status
    }

    pub fn probe_report_from_backend_device<T: AsFd>(device: T) -> GbmCapabilityProbeReport {
        report_from_probe_result(native::probe_backend_device(device))
    }

    pub fn probe_report_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
    ) -> GbmCapabilityProbeReport {
        report_from_probe_result(match device {
            Ok(device) => native::probe_backend_device(device),
            Err(_error) => GbmProbeResult::ReducedDeviceUnavailable,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GbmCapabilityProbeReport {
    pub status: GbmCapabilityProbeStatus,
    pub startup_status: LiveRendererImportStartupStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GbmCapabilityProbeStatus {
    NativeCapable,
    ReducedDeviceUnavailable,
    NativeDeviceRejected,
    PrivateAllocationUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GbmProbeResult {
    Capable,
    ReducedDeviceUnavailable,
    NativeDeviceRejected,
    PrivateAllocationUnavailable,
}

fn fake_probe(device: Option<GbmRenderDeviceToken>) -> GbmProbeResult {
    if device.is_some() {
        GbmProbeResult::Capable
    } else {
        GbmProbeResult::ReducedDeviceUnavailable
    }
}

fn report_from_probe_result(result: GbmProbeResult) -> GbmCapabilityProbeReport {
    let status = match result {
        GbmProbeResult::Capable => GbmCapabilityProbeStatus::NativeCapable,
        GbmProbeResult::ReducedDeviceUnavailable => {
            GbmCapabilityProbeStatus::ReducedDeviceUnavailable
        }
        GbmProbeResult::NativeDeviceRejected => GbmCapabilityProbeStatus::NativeDeviceRejected,
        GbmProbeResult::PrivateAllocationUnavailable => {
            GbmCapabilityProbeStatus::PrivateAllocationUnavailable
        }
    };
    let dmabuf = if result == GbmProbeResult::Capable {
        LiveRendererImportPathStatus::Enabled
    } else {
        LiveRendererImportPathStatus::Degraded
    };

    GbmCapabilityProbeReport {
        status,
        startup_status: LiveRendererImportStartupStatus::from_path_statuses(
            LiveRendererImportPathStatus::Disabled,
            dmabuf,
        ),
    }
}

mod native {
    use super::{GbmProbeResult, GbmRenderDeviceToken};
    use std::os::fd::AsFd;

    pub(super) fn probe(device: Option<GbmRenderDeviceToken>) -> GbmProbeResult {
        let Some(_device) = device else {
            return GbmProbeResult::ReducedDeviceUnavailable;
        };

        let _format = gbm::Format::Argb8888;
        let _usage = gbm::BufferObjectFlags::RENDERING;
        GbmProbeResult::Capable
    }

    pub(super) fn probe_backend_device<T: AsFd>(device: T) -> GbmProbeResult {
        match gbm::Device::new(device) {
            Ok(device) if can_allocate_first_private_buffer(&device) => GbmProbeResult::Capable,
            Ok(_device) => GbmProbeResult::PrivateAllocationUnavailable,
            Err(_error) => GbmProbeResult::NativeDeviceRejected,
        }
    }

    fn can_allocate_first_private_buffer<T: AsFd>(device: &gbm::Device<T>) -> bool {
        let format = gbm::Format::Argb8888;
        let usage = gbm::BufferObjectFlags::RENDERING;

        device.is_format_supported(format, usage)
            && device
                .create_buffer_object::<()>(1, 1, format, usage)
                .is_ok()
    }
}

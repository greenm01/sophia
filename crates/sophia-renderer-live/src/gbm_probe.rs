use crate::{LiveRendererImportPathStatus, LiveRendererImportStartupStatus};

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GbmProbeResult {
    Capable,
    Unavailable,
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
            GbmProbeResult::Unavailable => LiveRendererImportPathStatus::Degraded,
        },
    )
}

mod native {
    use super::{GbmProbeResult, GbmRenderDeviceToken};

    pub(super) fn probe(device: Option<GbmRenderDeviceToken>) -> GbmProbeResult {
        let Some(_device) = device else {
            return GbmProbeResult::Unavailable;
        };

        let _format = gbm::Format::Argb8888;
        let _usage = gbm::BufferObjectFlags::RENDERING;
        GbmProbeResult::Capable
    }
}

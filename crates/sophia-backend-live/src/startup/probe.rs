use crate::prelude::*;

#[cfg(feature = "gbm-probe")]
pub trait RenderDeviceDiscoveryBackend {
    type Device: AsFd;

    fn open_render_device(&self) -> io::Result<Self::Device>;
}

#[cfg(feature = "gbm-probe")]
impl<T> RenderDeviceDiscoveryBackend for &T
where
    T: RenderDeviceDiscoveryBackend + ?Sized,
{
    type Device = T::Device;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        (*self).open_render_device()
    }
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
    pub(super) fn from_open_result<T>(device: &io::Result<T>) -> Self {
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
    pub(super) fn not_requested() -> Self {
        Self {
            status: LiveGpuStartupStatus::NotRequested,
        }
    }

    pub(super) fn from_discovery_and_probe(
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
    pub(super) fn from_probe_status(status: EglCapabilityProbeStatus) -> Self {
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

use crate::prelude::*;

#[cfg(feature = "gbm-probe")]
use super::{LiveGpuStartupReport, LiveGpuStartupStatus};

#[cfg(feature = "gbm-probe")]
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

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmBackedEglPlatformReport {
    pub status: EglPlatformStatus,
}

#[cfg(feature = "gbm-probe")]
impl LiveGbmBackedEglPlatformReport {
    pub fn from_gpu_startup(gpu_startup: LiveGpuStartupReport) -> Self {
        Self {
            status: EglPlatformStatus::from(gpu_startup.status),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveEglStartupReport {
    pub status: LiveEglStartupStatus,
}

impl LiveEglStartupReport {
    pub(in crate::startup) fn from_probe_status(status: EglCapabilityProbeStatus) -> Self {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveEglStartupStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

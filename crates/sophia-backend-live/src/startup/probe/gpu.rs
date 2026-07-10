use crate::prelude::*;

use super::{LiveRenderDeviceDiscoveryReport, LiveRenderDeviceDiscoveryStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendRendererProbeReport {
    pub render_device: LiveRenderDeviceDiscoveryReport,
    pub gpu_startup: LiveGpuStartupReport,
    pub renderer_import: LiveRendererImportStartupStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGpuStartupReport {
    pub status: LiveGpuStartupStatus,
}

impl LiveGpuStartupReport {
    pub(in crate::startup) fn not_requested() -> Self {
        Self {
            status: LiveGpuStartupStatus::NotRequested,
        }
    }

    pub(in crate::startup) fn from_discovery_and_probe(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGpuStartupStatus {
    NotRequested,
    NativeCapable,
    RenderDeviceUnavailable,
    GbmDeviceRejected,
    PrivateAllocationUnavailable,
}

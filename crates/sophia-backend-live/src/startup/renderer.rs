use crate::prelude::*;

use super::{LiveBackendStartupReport, LiveRendererPreference};

fn selection_from_native_status(
    status: LiveRendererImportStartupStatus,
) -> Option<RendererSelection> {
    if status.health != LiveRendererImportHealth::NativeImportCapable {
        return None;
    }

    Some(RendererSelection::ImportCapable {
        import_xpixmap: status.xpixmap == LiveRendererImportPathStatus::Enabled,
        import_dmabuf: status.dmabuf == LiveRendererImportPathStatus::Enabled,
    })
}

fn cpu_fallback_renderer_status() -> LiveRendererImportStartupStatus {
    LiveRendererImportBoundary::cpu_only().startup_status()
}

pub(super) fn selection_observation(
    selection: RendererSelection,
) -> LiveRendererSelectionObservation {
    match selection {
        RendererSelection::CpuFallback => LiveRendererSelectionObservation::CpuFallback,
        RendererSelection::ImportCapable { .. } => {
            LiveRendererSelectionObservation::NativeImportCapable
        }
    }
}

impl LiveBackendStartupReport {
    pub fn renderer_selection(&self) -> RendererSelection {
        self.try_renderer_selection()
            .unwrap_or(RendererSelection::CpuFallback)
    }

    pub fn try_renderer_selection(&self) -> Option<RendererSelection> {
        self.renderer_selection_for_status(self.renderer_import_status())
    }

    pub fn renderer_selection_for_status(
        &self,
        status: LiveRendererImportStartupStatus,
    ) -> Option<RendererSelection> {
        match self.renderer_preference {
            LiveRendererPreference::CpuOnly => Some(RendererSelection::CpuFallback),
            LiveRendererPreference::GpuPreferred => {
                Some(selection_from_native_status(status).unwrap_or(RendererSelection::CpuFallback))
            }
            LiveRendererPreference::GpuRequired => selection_from_native_status(status),
        }
    }

    pub fn renderer_runtime_status_for_preference(
        &self,
        status: LiveRendererImportStartupStatus,
    ) -> LiveRendererImportStartupStatus {
        match self.renderer_preference {
            LiveRendererPreference::CpuOnly => cpu_fallback_renderer_status(),
            LiveRendererPreference::GpuPreferred | LiveRendererPreference::GpuRequired => status,
        }
    }

    pub fn renderer_import_status(&self) -> LiveRendererImportStartupStatus {
        self.renderer_import.startup_status()
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_import_status_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveRendererImportStartupStatus
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        self.renderer_probe_report_with_gbm_device(discovery)
            .renderer_import
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_selection_with_gbm_device<D>(&self, discovery: &D) -> Option<RendererSelection>
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        self.renderer_selection_for_status(self.renderer_import_status_with_gbm_device(discovery))
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_probe_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> super::LiveBackendRendererProbeReport
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        if self.renderer_preference == LiveRendererPreference::CpuOnly
            || !self.renderer_import.import_dmabuf
        {
            return super::LiveBackendRendererProbeReport {
                render_device: super::LiveRenderDeviceDiscoveryReport {
                    status: super::LiveRenderDeviceDiscoveryStatus::NotRequested,
                },
                gpu_startup: super::LiveGpuStartupReport::not_requested(),
                renderer_import: self
                    .renderer_runtime_status_for_preference(self.renderer_import_status()),
            };
        }

        let device = discovery.open_render_device();
        let render_device = super::LiveRenderDeviceDiscoveryReport::from_open_result(&device);
        let probe_report =
            NativeGbmCapabilityProbe::probe_report_from_backend_device_result(device);
        let renderer_import = self.renderer_runtime_status_for_preference(
            self.renderer_import_status_from_gbm_probe(probe_report),
        );
        let gpu_startup = super::LiveGpuStartupReport::from_discovery_and_probe(
            render_device,
            probe_report.status,
        );

        super::LiveBackendRendererProbeReport {
            render_device,
            gpu_startup,
            renderer_import,
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_import_status_from_gbm_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveRendererImportStartupStatus {
        let configured = self.renderer_import_status();

        if !self.renderer_import.import_dmabuf {
            return configured;
        }

        self.renderer_import_status_from_gbm_probe(
            NativeGbmCapabilityProbe::probe_report_from_backend_device_result(device),
        )
    }

    #[cfg(feature = "gbm-probe")]
    fn renderer_import_status_from_gbm_probe(
        &self,
        probe_report: GbmCapabilityProbeReport,
    ) -> LiveRendererImportStartupStatus {
        let configured = self.renderer_import_status();

        if !self.renderer_import.import_dmabuf {
            return configured;
        }

        LiveRendererImportStartupStatus::from_path_statuses(
            configured.xpixmap,
            probe_report.startup_status.dmabuf,
        )
    }
}

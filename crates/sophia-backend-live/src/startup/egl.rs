use crate::prelude::*;

use super::LiveBackendStartupReport;

impl LiveBackendStartupReport {
    pub fn egl_probe_report(
        &self,
        platform: EglPlatformStatus,
        context: EglContextProbeStatus,
    ) -> super::LiveEglStartupReport {
        super::LiveEglStartupReport::from_probe_status(
            FakeEglCapabilityProbe::new(platform, context)
                .probe_report()
                .status,
        )
    }

    pub fn native_egl_probe_report(&self) -> super::LiveEglStartupReport {
        super::LiveEglStartupReport::from_probe_status(
            NativeEglCapabilityProbe::probe_report().status,
        )
    }

    pub fn native_egl_draw_smoke_report(&self) -> EglDrawSmokeReport {
        NativeEglDrawSmoke::smoke_report()
    }

    #[cfg(feature = "gbm-probe")]
    pub fn gbm_backed_egl_platform_report(
        &self,
        gpu_startup: super::LiveGpuStartupReport,
    ) -> super::LiveGbmBackedEglPlatformReport {
        super::LiveGbmBackedEglPlatformReport::from_gpu_startup(gpu_startup)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_platform_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> super::LiveGbmBackedEglPlatformReport {
        super::LiveGbmBackedEglPlatformReport {
            status: NativeGbmBackedEglPlatformProbe::platform_status_from_backend_device_result(
                device,
            ),
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_platform_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> super::LiveGbmBackedEglPlatformReport
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_platform_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_draw_smoke_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> EglDrawSmokeReport {
        NativeGbmBackedEglDrawSmoke::smoke_report_from_backend_device_result(device)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_draw_smoke_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> EglDrawSmokeReport
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_draw_smoke_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_presentation_smoke_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveRendererPresentationReport {
        NativeGbmBackedEglPresentationSmoke::smoke_report_from_backend_device_result(device)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_presentation_smoke_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveRendererPresentationReport
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_presentation_smoke_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_frame_target_allocation_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport {
        NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
            device, request,
        )
    }

    #[cfg(feature = "gbm-probe")]
    pub fn native_gbm_backed_egl_frame_target_allocation_report_with_gbm_device<D>(
        &self,
        discovery: &D,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport
    where
        D: super::RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_frame_target_allocation_report_from_device_result(
            discovery.open_render_device(),
            request,
        )
    }

    #[cfg(feature = "gbm-probe")]
    pub fn egl_probe_report_from_gbm_startup(
        &self,
        gpu_startup: super::LiveGpuStartupReport,
        context: EglContextProbeStatus,
    ) -> super::LiveEglStartupReport {
        self.egl_probe_report(
            self.gbm_backed_egl_platform_report(gpu_startup).status,
            context,
        )
    }
}

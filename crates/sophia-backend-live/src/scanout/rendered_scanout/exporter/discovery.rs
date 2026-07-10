use super::{LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter};
use crate::api::*;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{
    LiveRendererScanoutBufferExportStatus, NativeGbmOwnedScanoutBuffer,
    NativeGbmRenderedScanoutContext, NativeGbmRenderedScanoutContextStatus,
};

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub struct NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    discovery: R,
    context: Option<NativeGbmRenderedScanoutContext<R::Device>>,
    context_status: Option<NativeGbmRenderedScanoutContextStatus>,
    context_open_attempts: usize,
    export_attempts: usize,
    last_target: Option<LiveGbmEglFrameTargetRecord>,
    last_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    last_export_status: Option<LiveRendererScanoutBufferExportStatus>,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    pub fn new(discovery: R) -> Self {
        Self {
            discovery,
            context: None,
            context_status: None,
            context_open_attempts: 0,
            export_attempts: 0,
            last_target: None,
            last_target_lifecycle: None,
            last_export_status: None,
        }
    }

    pub const fn context_open_attempts(&self) -> usize {
        self.context_open_attempts
    }

    pub const fn export_attempts(&self) -> usize {
        self.export_attempts
    }

    pub const fn last_export_status(&self) -> Option<LiveRendererScanoutBufferExportStatus> {
        self.last_export_status
    }

    pub const fn last_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.last_target
    }

    pub const fn last_target_lifecycle(&self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.last_target_lifecycle
    }

    pub const fn context_status(&self) -> Option<NativeGbmRenderedScanoutContextStatus> {
        self.context_status
    }

    pub const fn context_ready(&self) -> bool {
        self.context.is_some()
    }

    pub fn discovery(&self) -> &R {
        &self.discovery
    }

    pub fn discovery_mut(&mut self) -> &mut R {
        &mut self.discovery
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    type Owner = NativeGbmOwnedScanoutBuffer;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        let target_lifecycle =
            LiveGbmEglFrameTargetLifecycleReport::from_size_update(self.last_target, target);
        self.last_target = Some(target);
        self.last_target_lifecycle = Some(target_lifecycle);

        if !target.is_valid_scanout_target() {
            self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::InvalidTarget);
            return LiveRenderedScanoutBufferExport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                None,
                None,
            );
        }

        if self.context.is_none() {
            self.context_open_attempts = self.context_open_attempts.saturating_add(1);
            let report = NativeGbmRenderedScanoutContext::from_backend_device_result(
                self.discovery.open_render_device(),
            );
            self.context_status = Some(report.status);
            self.context = report.context;
        }

        let Some(context) = &self.context else {
            let status = match self.context_status {
                Some(NativeGbmRenderedScanoutContextStatus::Degraded) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Ready) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Unavailable) | None => {
                    LiveRendererScanoutBufferExportStatus::Unavailable
                }
            };
            self.last_export_status = Some(status);
            return LiveRenderedScanoutBufferExport::new(status, None, None);
        };

        let report = context.export_rendered_owned_scanout_buffer(target);
        let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());
        self.last_export_status = Some(report.status);
        LiveRenderedScanoutBufferExport::new(report.status, descriptor, report.buffer)
    }
}

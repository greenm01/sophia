#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use super::{LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use crate::api::*;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use std::io;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use std::os::fd::AsFd;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{
    LiveRendererScanoutBufferExportDetail, LiveRendererScanoutBufferExportStatus,
};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExporter};

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub struct NativeGbmRenderedScanoutBufferExporter<T> {
    device: Option<io::Result<T>>,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<T> NativeGbmRenderedScanoutBufferExporter<T> {
    pub fn new(device: io::Result<T>) -> Self {
        Self {
            device: Some(device),
        }
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<T> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferExporter<T>
where
    T: AsFd,
{
    type Owner = NativeGbmOwnedScanoutBuffer;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        let Some(device) = self.device.take() else {
            return LiveRenderedScanoutBufferExport::new(
                LiveRendererScanoutBufferExportStatus::Unavailable,
                LiveRendererScanoutBufferExportDetail::BackendDeviceUnavailable,
                None,
                None,
            );
        };

        let report =
            NativeGbmScanoutBufferExporter::export_rendered_owned_scanout_buffer_from_backend_device_result(
                device, target,
            );
        let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());

        LiveRenderedScanoutBufferExport::new(
            report.status,
            report.detail,
            descriptor,
            report.buffer,
        )
    }
}

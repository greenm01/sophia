use std::os::fd::AsFd;

use crate::{
    LiveGbmEglFrameTargetRecord, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportStatus, Size,
};

#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBuffer {
    descriptor: LiveRendererScanoutBufferDescriptor,
    _buffer: sophia_renderer_native_egl::NativeGbmOwnedScanoutBuffer,
}

impl NativeGbmOwnedScanoutBuffer {
    pub const fn descriptor(&self) -> LiveRendererScanoutBufferDescriptor {
        self.descriptor
    }
}

#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBufferExportReport {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

pub struct NativeGbmRenderedScanoutContext<T: AsFd> {
    inner: sophia_renderer_native_egl::NativeGbmRenderedScanoutContext<T>,
}

impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: AsFd,
{
    pub fn from_backend_device_result(
        device: std::io::Result<T>,
    ) -> NativeGbmRenderedScanoutContextReport<T> {
        let report =
            sophia_renderer_native_egl::NativeGbmRenderedScanoutContext::from_backend_device_result(
                device,
            );
        NativeGbmRenderedScanoutContextReport {
            status: match report.status {
                sophia_renderer_native_egl::NativeGbmRenderedScanoutContextStatus::Ready => {
                    NativeGbmRenderedScanoutContextStatus::Ready
                }
                sophia_renderer_native_egl::NativeGbmRenderedScanoutContextStatus::Unavailable => {
                    NativeGbmRenderedScanoutContextStatus::Unavailable
                }
                sophia_renderer_native_egl::NativeGbmRenderedScanoutContextStatus::Degraded => {
                    NativeGbmRenderedScanoutContextStatus::Degraded
                }
            },
            context: report
                .context
                .map(|inner| NativeGbmRenderedScanoutContext { inner }),
        }
    }

    pub fn export_rendered_owned_scanout_buffer(
        &self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target() {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: LiveRendererScanoutBufferExportStatus::InvalidTarget,
                buffer: None,
            };
        }

        reduced_native_owned_scanout_buffer_export_report(
            self.inner.export_rendered_owned_scanout_buffer(
                target.size.width as u32,
                target.size.height as u32,
            ),
        )
    }
}

pub struct NativeGbmRenderedScanoutContextReport<T: AsFd> {
    pub status: NativeGbmRenderedScanoutContextStatus,
    pub context: Option<NativeGbmRenderedScanoutContext<T>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmScanoutBufferExporter;

impl NativeGbmScanoutBufferExporter {
    pub fn export_owned_scanout_buffer_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        export_native_owned_scanout_buffer_from_backend_device_result(
            device,
            target,
            sophia_renderer_native_egl::export_gbm_scanout_buffer_from_backend_device_result,
        )
    }

    pub fn export_rendered_owned_scanout_buffer_from_backend_device_result<T: AsFd>(
        device: std::io::Result<T>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        export_native_owned_scanout_buffer_from_backend_device_result(
            device,
            target,
            sophia_renderer_native_egl::export_rendered_gbm_scanout_buffer_from_backend_device_result,
        )
    }
}

fn export_native_owned_scanout_buffer_from_backend_device_result<T, F>(
    device: std::io::Result<T>,
    target: LiveGbmEglFrameTargetRecord,
    export: F,
) -> NativeGbmOwnedScanoutBufferExportReport
where
    T: AsFd,
    F: FnOnce(
        std::io::Result<T>,
        u32,
        u32,
    ) -> sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferExportReport,
{
    if !target.is_valid_scanout_target() {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: LiveRendererScanoutBufferExportStatus::InvalidTarget,
            buffer: None,
        };
    }

    let report = export(device, target.size.width as u32, target.size.height as u32);
    reduced_native_owned_scanout_buffer_export_report(report)
}

fn reduced_native_owned_scanout_buffer_export_report(
    report: sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferExportReport,
) -> NativeGbmOwnedScanoutBufferExportReport {
    let status = match report.status {
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::Exported => {
            LiveRendererScanoutBufferExportStatus::Exported
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::InvalidTarget => {
            LiveRendererScanoutBufferExportStatus::InvalidTarget
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::Unavailable => {
            LiveRendererScanoutBufferExportStatus::Unavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportStatus::Degraded => {
            LiveRendererScanoutBufferExportStatus::Degraded
        }
    };

    let buffer = report.buffer.and_then(|buffer| {
        let descriptor = LiveRendererScanoutBufferDescriptor::new(
            Size {
                width: buffer.width() as i32,
                height: buffer.height() as i32,
            },
            buffer.pitch(),
            buffer.format(),
            buffer.gem_handle(),
        );
        descriptor
            .is_valid_scanout_buffer()
            .then_some(NativeGbmOwnedScanoutBuffer {
                descriptor,
                _buffer: buffer,
            })
    });
    NativeGbmOwnedScanoutBufferExportReport { status, buffer }
}

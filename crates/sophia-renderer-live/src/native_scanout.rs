use std::os::fd::AsFd;

use crate::{
    LiveGbmEglFrameTargetRecord, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportDetail, LiveRendererScanoutBufferExportStatus, Size,
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
    pub detail: LiveRendererScanoutBufferExportDetail,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

impl NativeGbmOwnedScanoutBufferExportReport {
    pub fn new(
        status: LiveRendererScanoutBufferExportStatus,
        detail: LiveRendererScanoutBufferExportDetail,
        buffer: Option<NativeGbmOwnedScanoutBuffer>,
    ) -> Self {
        match status {
            LiveRendererScanoutBufferExportStatus::Exported => Self {
                status: if buffer.is_some() {
                    LiveRendererScanoutBufferExportStatus::Exported
                } else {
                    LiveRendererScanoutBufferExportStatus::Degraded
                },
                detail: if buffer.is_some() {
                    detail
                } else {
                    LiveRendererScanoutBufferExportDetail::RetainedBufferMissing
                },
                buffer,
            },
            status => Self {
                status,
                detail,
                buffer: None,
            },
        }
    }
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
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
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
        return NativeGbmOwnedScanoutBufferExportReport::new(
            LiveRendererScanoutBufferExportStatus::InvalidTarget,
            LiveRendererScanoutBufferExportDetail::InvalidTarget,
            None,
        );
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
        let descriptor = LiveRendererScanoutBufferDescriptor::new_with_planes(
            Size {
                width: buffer.width() as i32,
                height: buffer.height() as i32,
            },
            buffer.pitch(),
            buffer.format(),
            buffer.gem_handle(),
            buffer.plane_count(),
            buffer.plane_handles(),
            buffer.plane_pitches(),
            buffer.plane_offsets(),
            buffer.modifier(),
        );
        descriptor
            .is_valid_scanout_buffer()
            .then_some(NativeGbmOwnedScanoutBuffer {
                descriptor,
                _buffer: buffer,
            })
    });
    NativeGbmOwnedScanoutBufferExportReport::new(
        status,
        reduced_native_owned_scanout_buffer_export_detail(report.detail),
        buffer,
    )
}

fn reduced_native_owned_scanout_buffer_export_detail(
    detail: sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail,
) -> LiveRendererScanoutBufferExportDetail {
    match detail {
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::Exported => {
            LiveRendererScanoutBufferExportDetail::Exported
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::InvalidTarget => {
            LiveRendererScanoutBufferExportDetail::InvalidTarget
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::BackendDeviceUnavailable => {
            LiveRendererScanoutBufferExportDetail::BackendDeviceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::GbmDeviceUnavailable => {
            LiveRendererScanoutBufferExportDetail::GbmDeviceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglDisplayUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglDisplayUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglInitializeFailed => {
            LiveRendererScanoutBufferExportDetail::EglInitializeFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglBindApiFailed => {
            LiveRendererScanoutBufferExportDetail::EglBindApiFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglConfigUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglConfigUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable => {
            LiveRendererScanoutBufferExportDetail::GbmSurfaceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglSurfaceUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglContextUnavailable => {
            LiveRendererScanoutBufferExportDetail::EglContextUnavailable
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed => {
            LiveRendererScanoutBufferExportDetail::EglMakeCurrentFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::GlSmokeFailed => {
            LiveRendererScanoutBufferExportDetail::GlSmokeFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed => {
            LiveRendererScanoutBufferExportDetail::EglSwapBuffersFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed => {
            LiveRendererScanoutBufferExportDetail::FrontBufferLockFailed
        }
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor => {
            LiveRendererScanoutBufferExportDetail::InvalidBufferDescriptor
        }
    }
}

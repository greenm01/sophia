use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

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

    pub fn export_scanout_dma_buf_fds(&self) -> std::io::Result<NativeGbmScanoutBufferPlaneFds> {
        self._buffer
            .export_plane_fds()
            .map(NativeGbmScanoutBufferPlaneFds::new)
            .map_err(|_error| std::io::Error::other("GBM scanout DMA-BUF export failed"))
    }
}

pub struct NativeGbmScanoutBufferPlaneFds {
    inner: sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferPlaneFds,
}

impl NativeGbmScanoutBufferPlaneFds {
    fn new(inner: sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferPlaneFds) -> Self {
        Self { inner }
    }

    pub const fn plane_count(&self) -> u8 {
        self.inner.plane_count()
    }

    pub fn into_plane_fds(self) -> [Option<OwnedFd>; 4] {
        self.inner.into_plane_fds()
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

#[derive(Debug)]
pub struct LiveOwnedDmaBufFrame {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct LiveDmaBufFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

impl LiveOwnedDmaBufFrame {
    pub fn as_frame(&self) -> LiveDmaBufFrame<'_> {
        LiveDmaBufFrame {
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: self.modifier,
            fd: self.fd.as_fd(),
            offset: self.offset,
            stride: self.stride,
        }
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            width: self.width,
            height: self.height,
            format: self.format,
            modifier: self.modifier,
            fd: self.fd.try_clone()?,
            offset: self.offset,
            stride: self.stride,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveNativePersistentRenderStats {
    pub target_creations: usize,
    pub target_recreations: usize,
    pub gl_pipeline_creations: usize,
    pub frame_uploads: usize,
    pub max_upload: std::time::Duration,
}

impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: AsFd,
{
    pub fn persistent_render_stats(&self) -> LiveNativePersistentRenderStats {
        let stats = self.inner.persistent_render_stats();
        LiveNativePersistentRenderStats {
            target_creations: stats.target_creations,
            target_recreations: stats.target_recreations,
            gl_pipeline_creations: stats.gl_pipeline_creations,
            frame_uploads: stats.frame_uploads,
            max_upload: stats.max_upload,
        }
    }

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
        self.export_rendered_owned_scanout_buffer_with_modifiers(target, &[])
    }

    pub fn export_rendered_owned_scanout_buffer_with_modifiers(
        &self,
        target: LiveGbmEglFrameTargetRecord,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target() {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }

        reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_rendered_owned_scanout_buffer_with_modifiers(
                    target.size.width as u32,
                    target.size.height as u32,
                    preferred_modifiers,
                ),
        )
    }

    pub fn export_xrgb8888_owned_scanout_buffer_with_modifiers(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        frame: &crate::LiveCpuComposedFrame,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target()
            || frame.size != target.size
            || frame.format != crate::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
        {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }

        reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_xrgb8888_owned_scanout_buffer_with_modifiers(
                    target.size.width as u32,
                    target.size.height as u32,
                    frame.stride,
                    &frame.bytes,
                    preferred_modifiers,
                ),
        )
    }

    pub fn export_dmabuf_owned_scanout_buffer_with_modifiers(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
        frame: LiveDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target()
            || target.size.width != i32::try_from(frame.width).unwrap_or(i32::MAX)
            || target.size.height != i32::try_from(frame.height).unwrap_or(i32::MAX)
        {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }
        reduced_native_owned_scanout_buffer_export_report(
            self.inner
                .export_dmabuf_owned_scanout_buffer_with_modifiers(
                    sophia_renderer_native_egl::NativeDmaBufFrame {
                        width: frame.width,
                        height: frame.height,
                        format: frame.format,
                        modifier: frame.modifier,
                        fd: frame.fd,
                        offset: frame.offset,
                        stride: frame.stride,
                    },
                    preferred_modifiers,
                ),
        )
    }

    pub fn export_rendered_owned_scanout_buffer_with_modifiers_from_backend_device_result<
        Device: AsFd,
    >(
        device: std::io::Result<Device>,
        target: LiveGbmEglFrameTargetRecord,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !target.is_valid_scanout_target() {
            return NativeGbmOwnedScanoutBufferExportReport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
            );
        }

        reduced_native_owned_scanout_buffer_export_report(
            sophia_renderer_native_egl::export_rendered_gbm_scanout_buffer_with_modifiers_from_backend_device_result(
                device,
                target.size.width as u32,
                target.size.height as u32,
                preferred_modifiers,
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
        sophia_renderer_native_egl::NativeGbmScanoutBufferExportDetail::DmaBufImportFailed => {
            LiveRendererScanoutBufferExportDetail::DmaBufImportFailed
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

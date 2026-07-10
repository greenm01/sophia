use crate::Size;
use crate::{LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveGbmEglFrameTargetRecord};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererScanoutBufferDescriptor {
    pub status: LiveRendererScanoutBufferStatus,
    pub size: Size,
    pub pitch: u32,
    pub format: u32,
    pub gem_handle: u32,
}

impl LiveRendererScanoutBufferDescriptor {
    pub const fn new(size: Size, pitch: u32, format: u32, gem_handle: u32) -> Self {
        Self {
            status: if is_valid_scanout_buffer_shape(size, pitch, format, gem_handle) {
                LiveRendererScanoutBufferStatus::Ready
            } else {
                LiveRendererScanoutBufferStatus::Invalid
            },
            size,
            pitch,
            format,
            gem_handle,
        }
    }

    pub const fn is_valid_scanout_buffer(self) -> bool {
        matches!(self.status, LiveRendererScanoutBufferStatus::Ready)
            && is_valid_scanout_buffer_shape(self.size, self.pitch, self.format, self.gem_handle)
    }
}

const fn is_valid_scanout_buffer_shape(
    size: Size,
    pitch: u32,
    format: u32,
    gem_handle: u32,
) -> bool {
    size.width > 0
        && size.height > 0
        && size.width <= (u32::MAX / LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL) as i32
        && pitch >= minimum_xrgb8888_pitch(size.width)
        && gem_handle > 0
        && format == LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
}

const LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL: u32 = 4;

const fn minimum_xrgb8888_pitch(width: i32) -> u32 {
    width as u32 * LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererScanoutBufferStatus {
    Ready,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererScanoutBufferExportReport {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub descriptor: Option<LiveRendererScanoutBufferDescriptor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererScanoutBufferExportStatus {
    Exported,
    InvalidTarget,
    Unavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererScanoutBufferExportDetail {
    Exported,
    InvalidTarget,
    BackendDeviceUnavailable,
    GbmDeviceUnavailable,
    EglUnavailable,
    EglDisplayUnavailable,
    EglInitializeFailed,
    EglBindApiFailed,
    EglConfigUnavailable,
    GbmSurfaceUnavailable,
    EglSurfaceUnavailable,
    EglContextUnavailable,
    EglMakeCurrentFailed,
    GlSmokeFailed,
    EglSwapBuffersFailed,
    FrontBufferLockFailed,
    InvalidBufferDescriptor,
    RetainedBufferMissing,
}

impl LiveRendererScanoutBufferExportDetail {
    pub const fn from_status(status: LiveRendererScanoutBufferExportStatus) -> Self {
        match status {
            LiveRendererScanoutBufferExportStatus::Exported => Self::Exported,
            LiveRendererScanoutBufferExportStatus::InvalidTarget => Self::InvalidTarget,
            LiveRendererScanoutBufferExportStatus::Unavailable => Self::BackendDeviceUnavailable,
            LiveRendererScanoutBufferExportStatus::Degraded => Self::RetainedBufferMissing,
        }
    }
}

pub trait LiveRendererScanoutBufferExporter {
    fn export_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRendererScanoutBufferExportReport;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeRendererScanoutBufferExporter {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub pitch: u32,
    pub format: u32,
    pub gem_handle: u32,
}

impl FakeRendererScanoutBufferExporter {
    pub const fn new(status: LiveRendererScanoutBufferExportStatus) -> Self {
        Self {
            status,
            pitch: 0,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            gem_handle: 0,
        }
    }

    pub const fn with_descriptor(mut self, pitch: u32, format: u32, gem_handle: u32) -> Self {
        self.pitch = pitch;
        self.format = format;
        self.gem_handle = gem_handle;
        self
    }
}

impl LiveRendererScanoutBufferExporter for FakeRendererScanoutBufferExporter {
    fn export_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRendererScanoutBufferExportReport {
        if !target.is_valid_scanout_target() {
            return LiveRendererScanoutBufferExportReport {
                status: LiveRendererScanoutBufferExportStatus::InvalidTarget,
                descriptor: None,
            };
        }

        match self.status {
            LiveRendererScanoutBufferExportStatus::Exported => {
                let descriptor = LiveRendererScanoutBufferDescriptor::new(
                    target.size,
                    self.pitch,
                    self.format,
                    self.gem_handle,
                );
                if descriptor.is_valid_scanout_buffer() {
                    LiveRendererScanoutBufferExportReport {
                        status: LiveRendererScanoutBufferExportStatus::Exported,
                        descriptor: Some(descriptor),
                    }
                } else {
                    LiveRendererScanoutBufferExportReport {
                        status: LiveRendererScanoutBufferExportStatus::Degraded,
                        descriptor: None,
                    }
                }
            }
            status => LiveRendererScanoutBufferExportReport {
                status,
                descriptor: None,
            },
        }
    }
}

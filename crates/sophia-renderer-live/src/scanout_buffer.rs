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
            status: if size.width > 0
                && size.height > 0
                && pitch > 0
                && gem_handle > 0
                && format == LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
            {
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
                if descriptor.status == LiveRendererScanoutBufferStatus::Ready {
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

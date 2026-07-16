use crate::Size;
use crate::{
    LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
    LiveGbmEglFrameTargetRecord,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererScanoutBufferDescriptor {
    pub status: LiveRendererScanoutBufferStatus,
    pub size: Size,
    pub pitch: u32,
    pub format: u32,
    pub gem_handle: u32,
    pub plane_count: u8,
    pub plane_handles: [u32; 4],
    pub plane_pitches: [u32; 4],
    pub plane_offsets: [u32; 4],
    pub modifier: Option<u64>,
}

impl LiveRendererScanoutBufferDescriptor {
    pub const fn new(size: Size, pitch: u32, format: u32, gem_handle: u32) -> Self {
        Self::new_with_planes(
            size,
            pitch,
            format,
            gem_handle,
            1,
            [gem_handle, 0, 0, 0],
            [pitch, 0, 0, 0],
            [0, 0, 0, 0],
            None,
        )
    }

    pub const fn new_with_planes(
        size: Size,
        pitch: u32,
        format: u32,
        gem_handle: u32,
        plane_count: u8,
        plane_handles: [u32; 4],
        plane_pitches: [u32; 4],
        plane_offsets: [u32; 4],
        modifier: Option<u64>,
    ) -> Self {
        Self {
            status: if is_valid_scanout_buffer_shape(
                size,
                pitch,
                format,
                gem_handle,
                plane_count,
                plane_handles,
                plane_pitches,
                plane_offsets,
            ) {
                LiveRendererScanoutBufferStatus::Ready
            } else {
                LiveRendererScanoutBufferStatus::Invalid
            },
            size,
            pitch,
            format,
            gem_handle,
            plane_count,
            plane_handles,
            plane_pitches,
            plane_offsets,
            modifier,
        }
    }

    pub const fn is_valid_scanout_buffer(self) -> bool {
        matches!(self.status, LiveRendererScanoutBufferStatus::Ready)
            && is_valid_scanout_buffer_shape(
                self.size,
                self.pitch,
                self.format,
                self.gem_handle,
                self.plane_count,
                self.plane_handles,
                self.plane_pitches,
                self.plane_offsets,
            )
    }
}

const fn is_valid_scanout_buffer_shape(
    size: Size,
    pitch: u32,
    format: u32,
    gem_handle: u32,
    plane_count: u8,
    plane_handles: [u32; 4],
    plane_pitches: [u32; 4],
    _plane_offsets: [u32; 4],
) -> bool {
    size.width > 0
        && size.height > 0
        && size.width <= (u32::MAX / LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL) as i32
        && pitch >= minimum_xrgb8888_pitch(size.width)
        && gem_handle > 0
        && is_supported_scanout_format(format)
        && is_valid_scanout_planes(
            size.width,
            gem_handle,
            plane_count,
            plane_handles,
            plane_pitches,
        )
}

const LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL: u32 = 4;
pub const LIVE_RENDERER_SCANOUT_MAX_PLANES: usize = 4;

pub const fn is_supported_scanout_format(format: u32) -> bool {
    format == LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
        || format == LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888
}

const fn minimum_xrgb8888_pitch(width: i32) -> u32 {
    width as u32 * LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL
}

const fn is_valid_scanout_planes(
    width: i32,
    gem_handle: u32,
    plane_count: u8,
    plane_handles: [u32; 4],
    plane_pitches: [u32; 4],
) -> bool {
    if plane_count == 0 || plane_count as usize > LIVE_RENDERER_SCANOUT_MAX_PLANES {
        return false;
    }
    if plane_handles[0] != gem_handle || plane_pitches[0] < minimum_xrgb8888_pitch(width) {
        return false;
    }

    let mut index = 0;
    while index < LIVE_RENDERER_SCANOUT_MAX_PLANES {
        if index < plane_count as usize {
            if plane_handles[index] == 0 || plane_pitches[index] == 0 {
                return false;
            }
        } else if plane_handles[index] != 0 || plane_pitches[index] != 0 {
            return false;
        }
        index += 1;
    }

    true
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
    CpuLayerUploadFailed,
    DmaBufImageCreateFailed,
    DmaBufImageBindFailed,
    CompositionDrawFailed,
    CompositionFinishFailed,
    EglImageDestroyFailed,
    DmaBufImportFailed,
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

//! Live renderer boundary.
//!
//! This crate is the future home for renderer-private resources such as GBM,
//! EGL, DMA-BUF import, explicit sync fences, and upload caches. The current
//! implementation is dependency-free beyond Sophia's own data crates and only
//! models reduced import admission.

pub use sophia_engine::BufferImportPath;
pub use sophia_protocol::{BufferSource, Size};

pub const LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888: u32 = 875_713_112;

#[cfg(feature = "egl-probe")]
mod egl_probe;
#[cfg(feature = "gbm-probe")]
mod gbm_probe;

#[cfg(feature = "egl-probe")]
pub use egl_probe::{
    EglCapabilityProbeReport, EglCapabilityProbeStatus, EglContextProbeStatus, EglDrawSmokeReport,
    EglDrawSmokeStatus, EglPlatformStatus, FakeEglCapabilityProbe, FakeEglDrawSmoke,
    NativeEglCapabilityProbe, NativeEglDrawSmoke,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
pub use egl_probe::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglFrameTargetAllocator,
    NativeGbmBackedEglPlatformProbe, NativeGbmBackedEglPresentationSmoke,
};

#[cfg(feature = "gbm-probe")]
pub use gbm_probe::{
    FakeGbmCapabilityProbe, GbmCapabilityProbeReport, GbmCapabilityProbeStatus,
    GbmRenderDeviceToken, NativeGbmCapabilityProbe,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakePresentationSmoke {
    pub status: LiveRendererPresentationStatus,
}

impl FakePresentationSmoke {
    pub const fn new(status: LiveRendererPresentationStatus) -> Self {
        Self { status }
    }

    pub const fn smoke_report(self) -> LiveRendererPresentationReport {
        LiveRendererPresentationReport {
            status: self.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererPresentationReport {
    pub status: LiveRendererPresentationStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererPresentationStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetRecord {
    pub status: LiveGbmEglFrameTargetStatus,
    pub size: Size,
}

impl LiveGbmEglFrameTargetRecord {
    pub const fn new(size: Size) -> Self {
        Self {
            status: if size.width > 0 && size.height > 0 {
                LiveGbmEglFrameTargetStatus::Ready
            } else {
                LiveGbmEglFrameTargetStatus::InvalidSize
            },
            size,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGbmEglFrameTargetStatus {
    Ready,
    InvalidSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetLifecycleReport {
    pub status: LiveGbmEglFrameTargetLifecycleStatus,
    pub target: LiveGbmEglFrameTargetRecord,
}

impl LiveGbmEglFrameTargetLifecycleReport {
    pub const fn created(target: LiveGbmEglFrameTargetRecord) -> Self {
        Self {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target,
        }
    }

    pub const fn from_size_update(
        previous: Option<LiveGbmEglFrameTargetRecord>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> Self {
        let status = match (previous, target.status) {
            (_, LiveGbmEglFrameTargetStatus::InvalidSize) => {
                LiveGbmEglFrameTargetLifecycleStatus::Invalidated
            }
            (None, LiveGbmEglFrameTargetStatus::Ready) => {
                LiveGbmEglFrameTargetLifecycleStatus::Created
            }
            (Some(previous), LiveGbmEglFrameTargetStatus::Ready)
                if previous.status as u8 == target.status as u8
                    && previous.size.width == target.size.width
                    && previous.size.height == target.size.height =>
            {
                LiveGbmEglFrameTargetLifecycleStatus::Retained
            }
            (Some(_), LiveGbmEglFrameTargetStatus::Ready) => {
                LiveGbmEglFrameTargetLifecycleStatus::Resized
            }
        };

        Self { status, target }
    }

    pub const fn retired(target: LiveGbmEglFrameTargetRecord) -> Self {
        Self {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retired,
            target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGbmEglFrameTargetLifecycleStatus {
    Created,
    Retained,
    Resized,
    Invalidated,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetAllocationRequest {
    pub target: LiveGbmEglFrameTargetRecord,
}

impl LiveGbmEglFrameTargetAllocationRequest {
    pub const fn new(size: Size) -> Self {
        Self {
            target: LiveGbmEglFrameTargetRecord::new(size),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetAllocationReport {
    pub status: LiveGbmEglFrameTargetAllocationStatus,
    pub target: LiveGbmEglFrameTargetRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGbmEglFrameTargetAllocationStatus {
    Ready,
    InvalidTarget,
    Unavailable,
    Degraded,
}

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
        if target.status != LiveGbmEglFrameTargetStatus::Ready {
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

#[cfg(feature = "gbm-probe")]
#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBuffer {
    descriptor: LiveRendererScanoutBufferDescriptor,
    _buffer: sophia_renderer_native_egl::NativeGbmOwnedScanoutBuffer,
}

#[cfg(feature = "gbm-probe")]
impl NativeGbmOwnedScanoutBuffer {
    pub const fn descriptor(&self) -> LiveRendererScanoutBufferDescriptor {
        self.descriptor
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBufferExportReport {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGbmScanoutBufferExporter;

#[cfg(feature = "gbm-probe")]
impl NativeGbmScanoutBufferExporter {
    pub fn export_owned_scanout_buffer_from_backend_device_result<T: std::os::fd::AsFd>(
        device: std::io::Result<T>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        export_native_owned_scanout_buffer_from_backend_device_result(
            device,
            target,
            sophia_renderer_native_egl::export_gbm_scanout_buffer_from_backend_device_result,
        )
    }

    pub fn export_rendered_owned_scanout_buffer_from_backend_device_result<T: std::os::fd::AsFd>(
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

#[cfg(feature = "gbm-probe")]
fn export_native_owned_scanout_buffer_from_backend_device_result<T, F>(
    device: std::io::Result<T>,
    target: LiveGbmEglFrameTargetRecord,
    export: F,
) -> NativeGbmOwnedScanoutBufferExportReport
where
    T: std::os::fd::AsFd,
    F: FnOnce(
        std::io::Result<T>,
        u32,
        u32,
    ) -> sophia_renderer_native_egl::NativeGbmOwnedScanoutBufferExportReport,
{
    if target.status != LiveGbmEglFrameTargetStatus::Ready {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: LiveRendererScanoutBufferExportStatus::InvalidTarget,
            buffer: None,
        };
    }

    let report = export(device, target.size.width as u32, target.size.height as u32);
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
        (descriptor.status == LiveRendererScanoutBufferStatus::Ready).then_some(
            NativeGbmOwnedScanoutBuffer {
                descriptor,
                _buffer: buffer,
            },
        )
    });
    NativeGbmOwnedScanoutBufferExportReport { status, buffer }
}

pub trait LiveGbmEglFrameTargetAllocator {
    fn allocate_frame_target(
        &mut self,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeGbmEglFrameTargetAllocator {
    pub status: LiveGbmEglFrameTargetAllocationStatus,
}

impl FakeGbmEglFrameTargetAllocator {
    pub const fn new(status: LiveGbmEglFrameTargetAllocationStatus) -> Self {
        Self { status }
    }
}

impl LiveGbmEglFrameTargetAllocator for FakeGbmEglFrameTargetAllocator {
    fn allocate_frame_target(
        &mut self,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport {
        let status = if request.target.status == LiveGbmEglFrameTargetStatus::Ready {
            self.status
        } else {
            LiveGbmEglFrameTargetAllocationStatus::InvalidTarget
        };

        LiveGbmEglFrameTargetAllocationReport {
            status,
            target: request.target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererImportBoundary {
    pub import_xpixmap: bool,
    pub import_dmabuf: bool,
}

impl LiveRendererImportBoundary {
    pub const fn cpu_only() -> Self {
        Self {
            import_xpixmap: false,
            import_dmabuf: false,
        }
    }

    pub const fn with_native_imports(import_xpixmap: bool, import_dmabuf: bool) -> Self {
        Self {
            import_xpixmap,
            import_dmabuf,
        }
    }

    pub fn decide(self, source: BufferSource) -> LiveRendererImportDecision {
        match source {
            BufferSource::None => LiveRendererImportDecision::Rejected {
                reason: LiveRendererImportRejection::EmptySource,
            },
            BufferSource::CpuBuffer { .. } => LiveRendererImportDecision::Accepted {
                path: BufferImportPath::CpuReadback,
            },
            BufferSource::XPixmap { .. } if self.import_xpixmap => {
                LiveRendererImportDecision::Accepted {
                    path: BufferImportPath::XPixmap,
                }
            }
            BufferSource::DmaBuf { .. } if self.import_dmabuf => {
                LiveRendererImportDecision::Accepted {
                    path: BufferImportPath::DmaBuf,
                }
            }
            BufferSource::XPixmap { .. } => LiveRendererImportDecision::Deferred {
                requested: BufferImportPath::XPixmap,
                required_boundary: "live XPixmap renderer import",
            },
            BufferSource::DmaBuf { .. } => LiveRendererImportDecision::Deferred {
                requested: BufferImportPath::DmaBuf,
                required_boundary: "live DMA-BUF renderer import",
            },
        }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        LiveRendererImportStartupStatus::from_path_statuses(
            if self.import_xpixmap {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Disabled
            },
            if self.import_dmabuf {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Disabled
            },
        )
    }
}

impl Default for LiveRendererImportBoundary {
    fn default() -> Self {
        Self::cpu_only()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportDecision {
    Accepted {
        path: BufferImportPath,
    },
    Deferred {
        requested: BufferImportPath,
        required_boundary: &'static str,
    },
    Rejected {
        reason: LiveRendererImportRejection,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportRejection {
    EmptySource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererImportStartupStatus {
    pub health: LiveRendererImportHealth,
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
}

impl LiveRendererImportStartupStatus {
    pub fn from_path_statuses(
        xpixmap: LiveRendererImportPathStatus,
        dmabuf: LiveRendererImportPathStatus,
    ) -> Self {
        Self {
            health: renderer_health_from_path_statuses(xpixmap, dmabuf),
            xpixmap,
            dmabuf,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportHealth {
    CpuFallback,
    NativeImportCapable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportPathStatus {
    Disabled,
    Enabled,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeLiveRendererCapabilityProbe {
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
}

impl FakeLiveRendererCapabilityProbe {
    pub const fn new(
        xpixmap: LiveRendererImportPathStatus,
        dmabuf: LiveRendererImportPathStatus,
    ) -> Self {
        Self { xpixmap, dmabuf }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        LiveRendererImportStartupStatus::from_path_statuses(self.xpixmap, self.dmabuf)
    }
}

fn renderer_health_from_path_statuses(
    xpixmap: LiveRendererImportPathStatus,
    dmabuf: LiveRendererImportPathStatus,
) -> LiveRendererImportHealth {
    match (xpixmap, dmabuf) {
        (LiveRendererImportPathStatus::Degraded, _)
        | (_, LiveRendererImportPathStatus::Degraded) => LiveRendererImportHealth::Degraded,
        (LiveRendererImportPathStatus::Enabled, _) | (_, LiveRendererImportPathStatus::Enabled) => {
            LiveRendererImportHealth::NativeImportCapable
        }
        (LiveRendererImportPathStatus::Disabled, LiveRendererImportPathStatus::Disabled) => {
            LiveRendererImportHealth::CpuFallback
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererRuntimeObservation {
    pub health: LiveRendererImportHealth,
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
    pub selection: LiveRendererSelectionObservation,
}

impl LiveRendererRuntimeObservation {
    pub fn from_startup_status(
        status: LiveRendererImportStartupStatus,
        selection: LiveRendererSelectionObservation,
    ) -> Self {
        Self {
            health: status.health,
            xpixmap: status.xpixmap,
            dmabuf: status.dmabuf,
            selection,
        }
    }

    pub fn degraded_by_failed_import(self, requested: BufferImportPath) -> Self {
        let mut degraded = Self {
            health: LiveRendererImportHealth::Degraded,
            ..self
        };

        match requested {
            BufferImportPath::XPixmap => {
                degraded.xpixmap = LiveRendererImportPathStatus::Degraded;
            }
            BufferImportPath::DmaBuf => {
                degraded.dmabuf = LiveRendererImportPathStatus::Degraded;
            }
            BufferImportPath::CpuReadback => {}
        }

        degraded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererSelectionObservation {
    CpuFallback,
    NativeImportCapable,
}

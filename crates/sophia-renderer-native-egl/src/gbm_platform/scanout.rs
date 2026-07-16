use std::{
    ffi::c_void,
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
    process, ptr,
    time::Duration,
    time::Instant,
};

use crate::gbm_platform::{
    EGL_PLATFORM_GBM_KHR,
    config::{window_config_attributes, xrgb_window_config_attributes},
};
use crate::gl::{
    GlCompositionRect, PersistentXrgb8888GlPipeline, context_attributes,
    draw_xrgb8888_current_gl_context_with_loader, smoke_current_gl_context_with_loader,
};
use crate::{
    NativeGbmRenderedScanoutContextStatus, NativeGbmScanoutBufferExportDetail,
    NativeGbmScanoutBufferExportStatus,
};

#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBuffer {
    width: u32,
    height: u32,
    pitch: u32,
    format: u32,
    gem_handle: u32,
    plane_count: u8,
    plane_handles: [u32; 4],
    plane_pitches: [u32; 4],
    plane_offsets: [u32; 4],
    plane_fds: Option<[Option<OwnedFd>; 4]>,
    modifier: Option<u64>,
    // Drop explicitly releases the locked front buffer before its surface.
    _buffer: Option<gbm::BufferObject<()>>,
    _surface: Option<gbm::Surface<()>>,
}

impl Drop for NativeGbmOwnedScanoutBuffer {
    fn drop(&mut self) {
        trace_native_lifecycle("scanout_owner_drop_started");
        drop(self._buffer.take());
        trace_native_lifecycle("front_buffer_released");
        drop(self._surface.take());
        trace_native_lifecycle("originating_surface_released");
    }
}

impl NativeGbmOwnedScanoutBuffer {
    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn pitch(&self) -> u32 {
        self.pitch
    }

    pub const fn format(&self) -> u32 {
        self.format
    }

    pub const fn gem_handle(&self) -> u32 {
        self.gem_handle
    }

    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub const fn plane_handles(&self) -> [u32; 4] {
        self.plane_handles
    }

    pub const fn plane_pitches(&self) -> [u32; 4] {
        self.plane_pitches
    }

    pub const fn plane_offsets(&self) -> [u32; 4] {
        self.plane_offsets
    }

    pub const fn modifier(&self) -> Option<u64> {
        self.modifier
    }

    pub fn export_plane_fds(
        &self,
    ) -> Result<NativeGbmOwnedScanoutBufferPlaneFds, NativeGbmScanoutBufferExportDetail> {
        if self.plane_count == 0 || self.plane_count as usize > self.plane_handles.len() {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        }

        let Some(retained_plane_fds) = &self.plane_fds else {
            return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
        };

        let mut plane_fds = std::array::from_fn(|_| None);
        let mut index = 0;
        while index < self.plane_count as usize {
            let Some(fd) = &retained_plane_fds[index] else {
                return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
            };
            plane_fds[index] =
                Some(fd.try_clone().map_err(|_error| {
                    NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor
                })?);
            index += 1;
        }

        Ok(NativeGbmOwnedScanoutBufferPlaneFds {
            plane_count: self.plane_count,
            plane_fds,
        })
    }
}

pub struct NativeGbmOwnedScanoutBufferPlaneFds {
    plane_count: u8,
    plane_fds: [Option<OwnedFd>; 4],
}

impl NativeGbmOwnedScanoutBufferPlaneFds {
    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub fn into_plane_fds(self) -> [Option<OwnedFd>; 4] {
        self.plane_fds
    }
}

#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBufferExportReport {
    pub status: NativeGbmScanoutBufferExportStatus,
    pub detail: NativeGbmScanoutBufferExportDetail,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

pub struct NativeGbmRenderedScanoutContext<T: std::os::fd::AsFd> {
    egl: khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: gbm::Device<T>,
    target: Option<PersistentNativeFrameTarget>,
    stats: NativeGbmPersistentRenderStats,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeDmaBufFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeDmaBufPlane<'a> {
    pub fd: BorrowedFd<'a>,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeMultiPlaneDmaBufFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    pub plane_count: u8,
    pub planes: [Option<NativeDmaBufPlane<'a>>; 4],
}

impl NativeMultiPlaneDmaBufFrame<'_> {
    pub fn is_valid(&self) -> bool {
        const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258;
        const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
        self.width > 0
            && self.height > 0
            && matches!(self.format, DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888)
            && self.plane_count > 0
            && usize::from(self.plane_count) <= self.planes.len()
            && self.planes[..usize::from(self.plane_count)]
                .iter()
                .all(Option::is_some)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompositionRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<NativeCompositionRect> for GlCompositionRect {
    fn from(rect: NativeCompositionRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeCpuCompositionLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u32,
    pub pixels: &'a [u8],
    pub target: NativeCompositionRect,
    pub clip: Option<NativeCompositionRect>,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeDmaBufCompositionLayer<'a> {
    pub frame: NativeMultiPlaneDmaBufFrame<'a>,
    pub target: NativeCompositionRect,
    pub clip: Option<NativeCompositionRect>,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum NativeCompositionLayer<'a> {
    Cpu(NativeCpuCompositionLayer<'a>),
    DmaBuf(NativeDmaBufCompositionLayer<'a>),
}

#[derive(Clone, Copy, Debug)]
pub struct NativeCompositionFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub layers: &'a [NativeCompositionLayer<'a>],
}

impl NativeDmaBufFrame<'_> {
    pub fn is_valid(&self) -> bool {
        const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258;
        const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241;
        self.width > 0
            && self.height > 0
            && matches!(self.format, DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888)
            && self.stride >= self.width.saturating_mul(4)
            && matches!(self.modifier, 0 | u64::MAX)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeGbmPersistentRenderStats {
    pub target_creations: usize,
    pub target_recreations: usize,
    pub gl_pipeline_creations: usize,
    pub frame_uploads: usize,
    pub max_upload: Duration,
}

struct PersistentNativeFrameTarget {
    width: u32,
    height: u32,
    preferred_modifiers: Vec<u64>,
    config: khronos_egl::Config,
    candidate: RenderedScanoutCandidate,
    egl_context: khronos_egl::Context,
    pipeline: PersistentXrgb8888GlPipeline,
}

impl<T> NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    pub fn from_backend_device_result(
        device: std::io::Result<T>,
    ) -> NativeGbmRenderedScanoutContextReport<T> {
        match device {
            Ok(device) => match Self::new(device) {
                Ok(context) => NativeGbmRenderedScanoutContextReport {
                    status: NativeGbmRenderedScanoutContextStatus::Ready,
                    context: Some(context),
                },
                Err(status) => NativeGbmRenderedScanoutContextReport {
                    status,
                    context: None,
                },
            },
            Err(_error) => NativeGbmRenderedScanoutContextReport {
                status: NativeGbmRenderedScanoutContextStatus::Unavailable,
                context: None,
            },
        }
    }

    fn new(device: T) -> Result<Self, NativeGbmRenderedScanoutContextStatus> {
        use gbm::AsRaw as _;

        let gbm_device = gbm::Device::new(device)
            .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Unavailable)?;
        let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
            .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Unavailable)?;
        let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
        let display = unsafe {
            egl.get_platform_display(
                EGL_PLATFORM_GBM_KHR,
                native_display,
                &[khronos_egl::ATTRIB_NONE],
            )
        }
        .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Unavailable)?;

        egl.initialize(display)
            .map_err(|_error| NativeGbmRenderedScanoutContextStatus::Degraded)?;

        Ok(Self {
            egl,
            display,
            gbm_device,
            target: None,
            stats: NativeGbmPersistentRenderStats::default(),
        })
    }

    pub const fn persistent_render_stats(&self) -> NativeGbmPersistentRenderStats {
        self.stats
    }

    pub fn export_rendered_owned_scanout_buffer(
        &self,
        width: u32,
        height: u32,
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        self.export_rendered_owned_scanout_buffer_with_modifiers(width, height, &[])
    }

    pub fn export_rendered_owned_scanout_buffer_with_modifiers(
        &self,
        width: u32,
        height: u32,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if width == 0 || height == 0 {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }

        match render_initialized_gbm_scanout_front_buffer(
            &self.egl,
            self.display,
            &self.gbm_device,
            width,
            height,
            preferred_modifiers,
            None,
        ) {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn export_xrgb8888_owned_scanout_buffer_with_modifiers(
        &mut self,
        width: u32,
        height: u32,
        stride: u32,
        pixels: &[u8],
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if width == 0 || height == 0 {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }

        let expected_stride = width.saturating_mul(4);
        let expected_len = usize::try_from(expected_stride)
            .ok()
            .and_then(|stride| stride.checked_mul(usize::try_from(height).ok()?));
        if stride != expected_stride || expected_len != Some(pixels.len()) {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }
        let started = Instant::now();
        let result = self.render_persistent_xrgb8888(width, height, pixels, preferred_modifiers);
        self.stats.max_upload = self.stats.max_upload.max(started.elapsed());
        match result {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn export_dmabuf_owned_scanout_buffer_with_modifiers(
        &mut self,
        frame: NativeDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if !frame.is_valid() {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }
        let result = self.render_persistent_dmabuf(frame, preferred_modifiers);
        match result {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    pub fn export_composed_owned_scanout_buffer_with_modifiers(
        &mut self,
        frame: NativeCompositionFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> NativeGbmOwnedScanoutBufferExportReport {
        if frame.width == 0
            || frame.height == 0
            || frame.layers.iter().any(|layer| match layer {
                NativeCompositionLayer::Cpu(layer) => {
                    layer.width == 0
                        || layer.height == 0
                        || !matches!(layer.format, 0x3432_5258 | 0x3432_5241)
                        || layer.target.width <= 0
                        || layer.target.height <= 0
                        || !layer.alpha.is_finite()
                }
                NativeCompositionLayer::DmaBuf(layer) => {
                    !layer.frame.is_valid()
                        || layer.target.width <= 0
                        || layer.target.height <= 0
                        || !layer.alpha.is_finite()
                }
            })
        {
            return NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
                detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
                buffer: None,
            };
        }
        match self.render_persistent_composition(frame, preferred_modifiers) {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }

    fn render_persistent_composition(
        &mut self,
        frame: NativeCompositionFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let preferred_modifiers = preferred_modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != u64::MAX)
            .collect::<Vec<_>>();
        let reusable = self.target.as_ref().is_some_and(|target| {
            target.width == frame.width
                && target.height == frame.height
                && target.preferred_modifiers == preferred_modifiers
        });
        if !reusable && let Some(target) = self.target.take() {
            self.destroy_persistent_target(target);
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
        }
        if let Some(mut target) = self.target.take() {
            let result = render_persistent_target_composition(
                &self.egl,
                self.display,
                &self.gbm_device,
                &mut target,
                frame,
            );
            // The exported GBM owner keeps the scanout surface alive. Retire
            // the context here so Radeon cannot carry imported-image command
            // stream state into the next CPU upload.
            if result.is_ok() {
                self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
            }
            self.destroy_persistent_target(target);
            return result;
        }

        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let reduced = reduced_gbm_scanout_modifiers(&preferred_modifiers);
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&reduced) {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let target = create_persistent_target(
                &self.egl,
                self.display,
                &self.gbm_device,
                frame.width,
                frame.height,
                preferred_modifiers.clone(),
                config,
                candidate,
            );
            let mut target = match target {
                Ok(target) => target,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            match render_persistent_target_composition(
                &self.egl,
                self.display,
                &self.gbm_device,
                &mut target,
                frame,
            ) {
                Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.target_creations = self.stats.target_creations.saturating_add(1);
                    self.stats.gl_pipeline_creations =
                        self.stats.gl_pipeline_creations.saturating_add(1);
                    self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
                    self.destroy_persistent_target(target);
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_persistent_target(target);
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_persistent_target(target);
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
        }
        Err(last_detail)
    }

    fn render_persistent_dmabuf(
        &mut self,
        frame: NativeDmaBufFrame<'_>,
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let preferred_modifiers = preferred_modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != u64::MAX)
            .collect::<Vec<_>>();
        let reusable = self.target.as_ref().is_some_and(|target| {
            target.width == frame.width
                && target.height == frame.height
                && target.preferred_modifiers == preferred_modifiers
        });
        if !reusable && let Some(target) = self.target.take() {
            self.destroy_persistent_target(target);
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
        }
        if let Some(mut target) = self.target.take() {
            let result = render_persistent_target_dmabuf(
                &self.egl,
                self.display,
                &self.gbm_device,
                &mut target,
                frame,
            );
            if result.is_ok() {
                self.target = Some(target);
            } else {
                self.destroy_persistent_target(target);
            }
            return result;
        }

        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let reduced = reduced_gbm_scanout_modifiers(&preferred_modifiers);
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&reduced) {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let target = create_persistent_target(
                &self.egl,
                self.display,
                &self.gbm_device,
                frame.width,
                frame.height,
                preferred_modifiers.clone(),
                config,
                candidate,
            );
            let mut target = match target {
                Ok(target) => target,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            match render_persistent_target_dmabuf(
                &self.egl,
                self.display,
                &self.gbm_device,
                &mut target,
                frame,
            ) {
                Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.target_creations = self.stats.target_creations.saturating_add(1);
                    self.stats.gl_pipeline_creations =
                        self.stats.gl_pipeline_creations.saturating_add(1);
                    self.target = Some(target);
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_persistent_target(target);
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_persistent_target(target);
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
        }
        Err(last_detail)
    }

    fn render_persistent_xrgb8888(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        preferred_modifiers: &[u64],
    ) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
        let preferred_modifiers = preferred_modifiers
            .iter()
            .copied()
            .filter(|modifier| *modifier != u64::MAX)
            .collect::<Vec<_>>();
        let reusable = self.target.as_ref().is_some_and(|target| {
            target.width == width
                && target.height == height
                && target.preferred_modifiers == preferred_modifiers
        });
        if !reusable && let Some(target) = self.target.take() {
            self.destroy_persistent_target(target);
            self.stats.target_recreations = self.stats.target_recreations.saturating_add(1);
        }
        if let Some(mut target) = self.target.take() {
            let result = render_persistent_target_frame(
                &self.egl,
                self.display,
                &self.gbm_device,
                &mut target,
                pixels,
            );
            if result.is_ok() {
                self.stats.frame_uploads = self.stats.frame_uploads.saturating_add(1);
                self.target = Some(target);
            } else {
                self.destroy_persistent_target(target);
            }
            return result;
        }

        self.egl
            .bind_api(khronos_egl::OPENGL_API)
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;
        let reduced = reduced_gbm_scanout_modifiers(&preferred_modifiers);
        let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
        for candidate in rendered_scanout_candidates(&reduced) {
            let Some(config) = choose_scanout_config_for_format(
                &self.egl,
                self.display,
                candidate.config_attributes,
                candidate.format,
            ) else {
                continue;
            };
            let target = create_persistent_target(
                &self.egl,
                self.display,
                &self.gbm_device,
                width,
                height,
                preferred_modifiers.clone(),
                config,
                candidate,
            );
            let mut target = match target {
                Ok(target) => target,
                Err(detail) => {
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                    continue;
                }
            };
            match render_persistent_target_frame(
                &self.egl,
                self.display,
                &self.gbm_device,
                &mut target,
                pixels,
            ) {
                Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                    self.stats.target_creations = self.stats.target_creations.saturating_add(1);
                    self.stats.gl_pipeline_creations =
                        self.stats.gl_pipeline_creations.saturating_add(1);
                    self.stats.frame_uploads = self.stats.frame_uploads.saturating_add(1);
                    self.target = Some(target);
                    return Ok(buffer);
                }
                Ok(_) => {
                    self.destroy_persistent_target(target);
                    last_detail = NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor;
                }
                Err(detail) => {
                    self.destroy_persistent_target(target);
                    last_detail = preferred_scanout_failure_detail(last_detail, detail);
                }
            }
        }
        Err(last_detail)
    }

    fn destroy_persistent_target(&self, target: PersistentNativeFrameTarget) {
        trace_native_lifecycle("persistent_target_destroy_started");
        let _ = self.egl.make_current(self.display, None, None, None);
        drop(target.pipeline);
        let _ = self.egl.destroy_context(self.display, target.egl_context);
        trace_native_lifecycle("egl_context_destroyed");
    }
}

impl<T> Drop for NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    fn drop(&mut self) {
        if let Some(target) = self.target.take() {
            self.destroy_persistent_target(target);
        }
        let _ = self.egl.terminate(self.display);
        trace_native_lifecycle("egl_display_terminated");
    }
}

pub struct NativeGbmRenderedScanoutContextReport<T: std::os::fd::AsFd> {
    pub status: NativeGbmRenderedScanoutContextStatus,
    pub context: Option<NativeGbmRenderedScanoutContext<T>>,
}

pub fn export_gbm_scanout_buffer_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
    width: u32,
    height: u32,
) -> NativeGbmOwnedScanoutBufferExportReport {
    if width == 0 || height == 0 {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
            detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
            buffer: None,
        };
    }

    let Ok(device) = device else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
            detail: NativeGbmScanoutBufferExportDetail::BackendDeviceUnavailable,
            buffer: None,
        };
    };
    let Ok(device) = gbm::Device::new(device) else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
            detail: NativeGbmScanoutBufferExportDetail::GbmDeviceUnavailable,
            buffer: None,
        };
    };
    let Ok(buffer) = device.create_buffer_object::<()>(
        width,
        height,
        gbm::Format::Xrgb8888,
        gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
    ) else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
            detail: NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable,
            buffer: None,
        };
    };

    match native_owned_scanout_buffer_from_bo(width, height, buffer, None) {
        Ok(buffer) => exported_scanout_buffer_report(buffer),
        Err(detail) => failed_scanout_buffer_report(detail),
    }
}

pub fn export_rendered_gbm_scanout_buffer_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
    width: u32,
    height: u32,
) -> NativeGbmOwnedScanoutBufferExportReport {
    export_rendered_gbm_scanout_buffer_with_modifiers_from_backend_device_result(
        device,
        width,
        height,
        &[],
    )
}

pub fn export_rendered_gbm_scanout_buffer_with_modifiers_from_backend_device_result<
    T: std::os::fd::AsFd,
>(
    device: std::io::Result<T>,
    width: u32,
    height: u32,
    preferred_modifiers: &[u64],
) -> NativeGbmOwnedScanoutBufferExportReport {
    if width == 0 || height == 0 {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
            detail: NativeGbmScanoutBufferExportDetail::InvalidTarget,
            buffer: None,
        };
    }

    let Ok(device) = device else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
            detail: NativeGbmScanoutBufferExportDetail::BackendDeviceUnavailable,
            buffer: None,
        };
    };

    match render_gbm_scanout_front_buffer(device, width, height, preferred_modifiers) {
        Ok(buffer) => exported_scanout_buffer_report(buffer),
        Err(detail) => failed_scanout_buffer_report(detail),
    }
}

fn native_owned_scanout_buffer_from_bo(
    width: u32,
    height: u32,
    buffer: gbm::BufferObject<()>,
    surface: Option<gbm::Surface<()>>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    let pitch = buffer.stride();
    let format = buffer.format() as u32;
    let gem_handle = unsafe { buffer.handle().u32_ };
    let plane_count = buffer.plane_count();
    let plane_handles = scanout_plane_handles(&buffer, plane_count);
    let plane_pitches = scanout_plane_pitches(&buffer, plane_count);
    let plane_offsets = scanout_plane_offsets(&buffer, plane_count);
    let plane_fds = capture_scanout_plane_fds(&buffer, plane_count).ok();
    if pitch == 0
        || gem_handle == 0
        || !is_supported_scanout_format(format)
        || !is_valid_scanout_planes(gem_handle, plane_count, plane_handles, plane_pitches)
    {
        return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
    }

    Ok(NativeGbmOwnedScanoutBuffer {
        width,
        height,
        pitch,
        format,
        gem_handle,
        plane_count: plane_count as u8,
        plane_handles,
        plane_pitches,
        plane_offsets,
        plane_fds,
        modifier: normalized_scanout_modifier(buffer.modifier()),
        _buffer: Some(buffer),
        _surface: surface,
    })
}

fn normalized_scanout_modifier(modifier: gbm::Modifier) -> Option<u64> {
    (!matches!(modifier, gbm::Modifier::Invalid)).then(|| modifier.into())
}

fn scanout_plane_handles(buffer: &gbm::BufferObject<()>, plane_count: u32) -> [u32; 4] {
    [
        plane_handle(buffer, plane_count, 0),
        plane_handle(buffer, plane_count, 1),
        plane_handle(buffer, plane_count, 2),
        plane_handle(buffer, plane_count, 3),
    ]
}

fn scanout_plane_pitches(buffer: &gbm::BufferObject<()>, plane_count: u32) -> [u32; 4] {
    [
        plane_pitch(buffer, plane_count, 0),
        plane_pitch(buffer, plane_count, 1),
        plane_pitch(buffer, plane_count, 2),
        plane_pitch(buffer, plane_count, 3),
    ]
}

fn scanout_plane_offsets(buffer: &gbm::BufferObject<()>, plane_count: u32) -> [u32; 4] {
    [
        plane_offset(buffer, plane_count, 0),
        plane_offset(buffer, plane_count, 1),
        plane_offset(buffer, plane_count, 2),
        plane_offset(buffer, plane_count, 3),
    ]
}

fn plane_handle(buffer: &gbm::BufferObject<()>, plane_count: u32, plane: i32) -> u32 {
    (plane < plane_count as i32)
        .then(|| unsafe { buffer.handle_for_plane(plane).u32_ })
        .unwrap_or(0)
}

fn plane_pitch(buffer: &gbm::BufferObject<()>, plane_count: u32, plane: i32) -> u32 {
    (plane < plane_count as i32)
        .then(|| buffer.stride_for_plane(plane))
        .unwrap_or(0)
}

fn plane_offset(buffer: &gbm::BufferObject<()>, plane_count: u32, plane: i32) -> u32 {
    (plane < plane_count as i32)
        .then(|| buffer.offset(plane))
        .unwrap_or(0)
}

fn capture_scanout_plane_fds(
    buffer: &gbm::BufferObject<()>,
    plane_count: u32,
) -> Result<[Option<OwnedFd>; 4], NativeGbmScanoutBufferExportDetail> {
    let mut plane_fds = std::array::from_fn(|_| None);
    let mut index = 0;
    while index < plane_count as usize {
        plane_fds[index] = Some(
            buffer
                .fd_for_plane(index as i32)
                .map_err(|_error| NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor)?,
        );
        index += 1;
    }
    Ok(plane_fds)
}

fn is_valid_scanout_planes(
    gem_handle: u32,
    plane_count: u32,
    plane_handles: [u32; 4],
    plane_pitches: [u32; 4],
) -> bool {
    plane_count > 0
        && plane_count <= 4
        && plane_handles[0] == gem_handle
        && plane_handles
            .iter()
            .zip(plane_pitches)
            .enumerate()
            .all(|(index, (handle, pitch))| {
                if index < plane_count as usize {
                    *handle != 0 && pitch != 0
                } else {
                    *handle == 0 && pitch == 0
                }
            })
}

fn render_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    device: T,
    width: u32,
    height: u32,
    preferred_modifiers: &[u64],
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_device = gbm::Device::new(device)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::GbmDeviceUnavailable)?;
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglUnavailable)?;

    let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
    let display = unsafe {
        egl.get_platform_display(
            EGL_PLATFORM_GBM_KHR,
            native_display,
            &[khronos_egl::ATTRIB_NONE],
        )
    }
    .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglDisplayUnavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglInitializeFailed)?;
    let result = render_initialized_gbm_scanout_front_buffer(
        &egl,
        display,
        &gbm_device,
        width,
        height,
        preferred_modifiers,
        None,
    );
    let _ = egl.terminate(display);
    result
}

fn render_initialized_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
    preferred_modifiers: &[u64],
    frame: Option<NativeXrgb8888Frame<'_>>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;

    let preferred_modifiers = reduced_gbm_scanout_modifiers(preferred_modifiers);
    let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
    for candidate in rendered_scanout_candidates(&preferred_modifiers) {
        let Some(config) = choose_scanout_config_for_format(
            egl,
            display,
            candidate.config_attributes,
            candidate.format,
        ) else {
            continue;
        };

        match render_initialized_gbm_scanout_front_buffer_with_config(
            egl,
            display,
            gbm_device,
            width,
            height,
            config,
            candidate.format,
            &candidate.modifiers,
            candidate.usage,
            frame,
        ) {
            Ok(buffer) if is_supported_rendered_scanout_candidate_buffer(&buffer) => {
                return Ok(buffer);
            }
            Ok(_buffer) => {
                last_detail = preferred_scanout_failure_detail(
                    last_detail,
                    NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor,
                );
            }
            Err(detail) => last_detail = preferred_scanout_failure_detail(last_detail, detail),
        }
    }

    Err(last_detail)
}

fn create_persistent_target<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
    preferred_modifiers: Vec<u64>,
    config: khronos_egl::Config,
    candidate: RenderedScanoutCandidate,
) -> Result<PersistentNativeFrameTarget, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        width,
        height,
        candidate.format,
        &candidate.modifiers,
        candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface = unsafe { egl.create_window_surface(display, config, native_window, None) }
        .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    let egl_context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_) => {
            let _ = egl.destroy_surface(display, egl_surface);
            return Err(NativeGbmScanoutBufferExportDetail::EglContextUnavailable);
        }
    };
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_context(display, egl_context);
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }
    let loader = |name: &str| {
        egl.get_proc_address(name)
            .map_or(ptr::null(), |proc| proc as *const c_void)
    };
    let gl = unsafe { glow::Context::from_loader_function(loader) };
    let pipeline = match unsafe { PersistentXrgb8888GlPipeline::new(gl, width, height) } {
        Ok(pipeline) => pipeline,
        Err(_) => {
            let _ = egl.make_current(display, None, None, None);
            let _ = egl.destroy_context(display, egl_context);
            let _ = egl.destroy_surface(display, egl_surface);
            return Err(NativeGbmScanoutBufferExportDetail::GlSmokeFailed);
        }
    };
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_surface(display, egl_surface);
    drop(gbm_surface);
    trace_native_lifecycle("persistent_context_created");
    Ok(PersistentNativeFrameTarget {
        width,
        height,
        preferred_modifiers,
        config,
        candidate,
        egl_context,
        pipeline,
    })
}

fn render_persistent_target_frame<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    target: &mut PersistentNativeFrameTarget,
    pixels: &[u8],
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        target.width,
        target.height,
        target.candidate.format,
        &target.candidate.modifiers,
        target.candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface =
        unsafe { egl.create_window_surface(display, target.config, native_window, None) }
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(target.egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }
    trace_native_lifecycle("egl_surface_current");
    let result = target
        .pipeline
        .upload(pixels)
        .map_err(|_| NativeGbmScanoutBufferExportDetail::GlSmokeFailed)
        .and_then(|()| {
            trace_native_lifecycle("cpu_frame_uploaded");
            egl.swap_buffers(display, egl_surface)
                .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
        })
        .and_then(|()| {
            trace_native_lifecycle("egl_surface_swapped");
            let buffer = unsafe { gbm_surface.lock_front_buffer() }
                .map_err(|_| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
            trace_native_lifecycle("scanout_front_buffer_locked");
            native_owned_scanout_buffer_from_bo(
                target.width,
                target.height,
                buffer,
                Some(gbm_surface),
            )
        });
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_surface(display, egl_surface);
    result
}

fn render_persistent_target_dmabuf<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    target: &mut PersistentNativeFrameTarget,
    frame: NativeDmaBufFrame<'_>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    const EGL_LINUX_DMA_BUF_EXT: khronos_egl::Enum = 0x3270;
    const EGL_WIDTH: khronos_egl::Attrib = 0x3057;
    const EGL_HEIGHT: khronos_egl::Attrib = 0x3056;
    const EGL_LINUX_DRM_FOURCC_EXT: khronos_egl::Attrib = 0x3271;
    const EGL_DMA_BUF_PLANE0_FD_EXT: khronos_egl::Attrib = 0x3272;
    const EGL_DMA_BUF_PLANE0_OFFSET_EXT: khronos_egl::Attrib = 0x3273;
    const EGL_DMA_BUF_PLANE0_PITCH_EXT: khronos_egl::Attrib = 0x3274;
    const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: khronos_egl::Attrib = 0x3443;
    const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: khronos_egl::Attrib = 0x3444;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        target.width,
        target.height,
        target.candidate.format,
        &target.candidate.modifiers,
        target.candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface =
        unsafe { egl.create_window_surface(display, target.config, native_window, None) }
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(target.egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }
    trace_dmabuf_lifecycle("egl_surface_current");

    let mut attributes = vec![
        EGL_WIDTH,
        frame.width as khronos_egl::Attrib,
        EGL_HEIGHT,
        frame.height as khronos_egl::Attrib,
        EGL_LINUX_DRM_FOURCC_EXT,
        frame.format as khronos_egl::Attrib,
        EGL_DMA_BUF_PLANE0_FD_EXT,
        frame.fd.as_raw_fd() as khronos_egl::Attrib,
        EGL_DMA_BUF_PLANE0_OFFSET_EXT,
        frame.offset as khronos_egl::Attrib,
        EGL_DMA_BUF_PLANE0_PITCH_EXT,
        frame.stride as khronos_egl::Attrib,
    ];
    if frame.modifier != u64::MAX {
        attributes.extend_from_slice(&[
            EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
            (frame.modifier & u64::from(u32::MAX)) as khronos_egl::Attrib,
            EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
            (frame.modifier >> 32) as khronos_egl::Attrib,
        ]);
    }
    attributes.push(khronos_egl::ATTRIB_NONE);
    let no_context = unsafe { khronos_egl::Context::from_ptr(khronos_egl::NO_CONTEXT) };
    let no_buffer = unsafe { khronos_egl::ClientBuffer::from_ptr(ptr::null_mut()) };
    let image = egl
        .create_image(
            display,
            no_context,
            EGL_LINUX_DMA_BUF_EXT,
            no_buffer,
            &attributes,
        )
        .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImportFailed);
    let result = match image {
        Ok(image) => {
            trace_dmabuf_lifecycle("egl_image_created");
            let result = egl
                .get_proc_address("glEGLImageTargetTexture2DOES")
                .ok_or(NativeGbmScanoutBufferExportDetail::DmaBufImportFailed)
                .map(|image_target| unsafe {
                    std::mem::transmute::<
                        extern "system" fn(),
                        unsafe extern "system" fn(u32, *const c_void),
                    >(image_target)
                })
                .and_then(|image_target| {
                    unsafe { target.pipeline.draw_egl_image(image_target, image.as_ptr()) }
                        .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImportFailed)
                })
                .and_then(|()| {
                    trace_dmabuf_lifecycle("egl_image_texture_released");
                    trace_dmabuf_lifecycle("egl_image_rendered");
                    egl.swap_buffers(display, egl_surface)
                        .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
                })
                .and_then(|()| {
                    trace_dmabuf_lifecycle("egl_surface_swapped");
                    let buffer = unsafe { gbm_surface.lock_front_buffer() }
                        .map_err(|_| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
                    trace_dmabuf_lifecycle("scanout_front_buffer_locked");
                    native_owned_scanout_buffer_from_bo(
                        target.width,
                        target.height,
                        buffer,
                        Some(gbm_surface),
                    )
                });
            let image_destroyed = egl.destroy_image(display, image).is_ok();
            if image_destroyed {
                trace_dmabuf_lifecycle("egl_image_destroyed");
            }
            match result {
                Ok(buffer) if image_destroyed => {
                    trace_dmabuf_lifecycle("scanout_owner_returned");
                    Ok(buffer)
                }
                Ok(_) => Err(NativeGbmScanoutBufferExportDetail::DmaBufImportFailed),
                Err(detail) => Err(detail),
            }
        }
        Err(detail) => Err(detail),
    };
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_surface(display, egl_surface);
    result
}

fn render_persistent_target_composition<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    target: &mut PersistentNativeFrameTarget,
    frame: NativeCompositionFrame<'_>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        target.width,
        target.height,
        target.candidate.format,
        &target.candidate.modifiers,
        target.candidate.usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let egl_surface =
        unsafe { egl.create_window_surface(display, target.config, native_window, None) }
            .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    if egl
        .make_current(
            display,
            Some(egl_surface),
            Some(egl_surface),
            Some(target.egl_context),
        )
        .is_err()
    {
        let _ = egl.destroy_surface(display, egl_surface);
        return Err(NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed);
    }

    trace_native_lifecycle("composition_surface_current");
    target.pipeline.begin_composition();
    trace_native_lifecycle("composition_started");
    let mut draw_result = Ok(());
    for layer in frame.layers {
        if draw_result.is_err() {
            break;
        }
        draw_result = match layer {
            NativeCompositionLayer::Cpu(layer) => {
                trace_native_lifecycle("composition_cpu_layer_started");
                let result = target
                    .pipeline
                    .draw_cpu_layer(
                        layer.width,
                        layer.height,
                        layer.stride,
                        layer.pixels,
                        layer.target.into(),
                        layer.clip.map(Into::into),
                        layer.alpha,
                        layer.format == 0x3432_5241,
                    )
                    .map_err(|_| NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed);
                if result.is_ok() {
                    trace_native_lifecycle("composition_cpu_layer_finished");
                }
                result
            }
            NativeCompositionLayer::DmaBuf(layer) => {
                trace_native_lifecycle("composition_dmabuf_layer_started");
                let result = draw_composition_dmabuf_layer(egl, display, &target.pipeline, *layer);
                if result.is_ok() {
                    trace_native_lifecycle("composition_dmabuf_layer_finished");
                }
                result
            }
        };
    }
    let result = draw_result
        .and_then(|()| {
            target
                .pipeline
                .finish_composition()
                .map_err(|_| NativeGbmScanoutBufferExportDetail::CompositionFinishFailed)
        })
        .and_then(|()| {
            trace_native_lifecycle("composition_finished");
            egl.swap_buffers(display, egl_surface)
                .map_err(|_| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
        })
        .and_then(|()| {
            trace_native_lifecycle("composition_surface_swapped");
            let buffer = unsafe { gbm_surface.lock_front_buffer() }
                .map_err(|_| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
            trace_native_lifecycle("composition_front_buffer_locked");
            native_owned_scanout_buffer_from_bo(
                target.width,
                target.height,
                buffer,
                Some(gbm_surface),
            )
        });
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_surface(display, egl_surface);
    result
}

fn draw_composition_dmabuf_layer(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    pipeline: &PersistentXrgb8888GlPipeline,
    layer: NativeDmaBufCompositionLayer<'_>,
) -> Result<(), NativeGbmScanoutBufferExportDetail> {
    const EGL_LINUX_DMA_BUF_EXT: khronos_egl::Enum = 0x3270;
    const EGL_WIDTH: khronos_egl::Attrib = 0x3057;
    const EGL_HEIGHT: khronos_egl::Attrib = 0x3056;
    const EGL_LINUX_DRM_FOURCC_EXT: khronos_egl::Attrib = 0x3271;
    const PLANE_ATTRIBUTES: [[khronos_egl::Attrib; 5]; 4] = [
        [0x3272, 0x3273, 0x3274, 0x3443, 0x3444],
        [0x3275, 0x3276, 0x3277, 0x3445, 0x3446],
        [0x3278, 0x3279, 0x327A, 0x3447, 0x3448],
        [0x3440, 0x3441, 0x3442, 0x3449, 0x344A],
    ];

    let mut attributes = vec![
        EGL_WIDTH,
        layer.frame.width as khronos_egl::Attrib,
        EGL_HEIGHT,
        layer.frame.height as khronos_egl::Attrib,
        EGL_LINUX_DRM_FOURCC_EXT,
        layer.frame.format as khronos_egl::Attrib,
    ];
    for index in 0..usize::from(layer.frame.plane_count) {
        let plane = layer.frame.planes[index]
            .ok_or(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor)?;
        let keys = PLANE_ATTRIBUTES[index];
        attributes.extend_from_slice(&[
            keys[0],
            plane.fd.as_raw_fd() as khronos_egl::Attrib,
            keys[1],
            plane.offset as khronos_egl::Attrib,
            keys[2],
            plane.stride as khronos_egl::Attrib,
        ]);
        if layer.frame.modifier != u64::MAX {
            attributes.extend_from_slice(&[
                keys[3],
                (layer.frame.modifier & u64::from(u32::MAX)) as khronos_egl::Attrib,
                keys[4],
                (layer.frame.modifier >> 32) as khronos_egl::Attrib,
            ]);
        }
    }
    attributes.push(khronos_egl::ATTRIB_NONE);
    let no_context = unsafe { khronos_egl::Context::from_ptr(khronos_egl::NO_CONTEXT) };
    let no_buffer = unsafe { khronos_egl::ClientBuffer::from_ptr(ptr::null_mut()) };
    let image = egl
        .create_image(
            display,
            no_context,
            EGL_LINUX_DMA_BUF_EXT,
            no_buffer,
            &attributes,
        )
        .map_err(|_| NativeGbmScanoutBufferExportDetail::DmaBufImageCreateFailed)?;
    let draw =
        egl.get_proc_address("glEGLImageTargetTexture2DOES")
            .ok_or(NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed)
            .map(|image_target| unsafe {
                std::mem::transmute::<
                    extern "system" fn(),
                    unsafe extern "system" fn(u32, *const c_void),
                >(image_target)
            })
            .and_then(|image_target| {
                unsafe {
                    pipeline.draw_egl_image_layer(
                        image_target,
                        image.as_ptr(),
                        layer.target.into(),
                        layer.clip.map(Into::into),
                        layer.alpha,
                        layer.frame.format == 0x3432_5241,
                    )
                }
                .map_err(|_| NativeGbmScanoutBufferExportDetail::CompositionDrawFailed)
            });
    let destroyed = egl.destroy_image(display, image).is_ok();
    match (draw, destroyed) {
        (Ok(()), true) => Ok(()),
        (Err(detail), _) => Err(detail),
        (Ok(()), false) => Err(NativeGbmScanoutBufferExportDetail::EglImageDestroyFailed),
    }
}

fn trace_dmabuf_lifecycle(stage: &str) {
    if std::env::var_os("SOPHIA_WAYLAND_DMABUF_DIAGNOSTIC").is_some() {
        eprintln!(
            "sophia_dmabuf_lifecycle schema=1 pid={} stage={stage}",
            process::id()
        );
    }
}

fn trace_native_lifecycle(stage: &str) {
    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
        eprintln!("sophia_native_lifecycle schema=1 stage={stage}");
    }
}

fn render_initialized_gbm_scanout_front_buffer_with_config<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
    config: khronos_egl::Config,
    surface_format: gbm::Format,
    surface_modifiers: &[gbm::Modifier],
    surface_usage: gbm::BufferObjectFlags,
    frame: Option<NativeXrgb8888Frame<'_>>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_surface = create_rendered_scanout_surface(
        gbm_device,
        width,
        height,
        surface_format,
        surface_modifiers,
        surface_usage,
    )?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let surface = unsafe { egl.create_window_surface(display, config, native_window, None) }
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglSurfaceUnavailable)?;
    let context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_error) => {
            let _ = egl.destroy_surface(display, surface);
            return Err(NativeGbmScanoutBufferExportDetail::EglContextUnavailable);
        }
    };

    let result = egl
        .make_current(display, Some(surface), Some(surface), Some(context))
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglMakeCurrentFailed)
        .and_then(|()| {
            let loader = |name: &str| {
                egl.get_proc_address(name)
                    .map_or(ptr::null(), |proc| proc as *const c_void)
            };
            match frame {
                Some(frame) => draw_xrgb8888_current_gl_context_with_loader(
                    loader,
                    width,
                    height,
                    frame.stride,
                    frame.pixels,
                ),
                None => smoke_current_gl_context_with_loader(loader),
            }
            .map_err(|_error| NativeGbmScanoutBufferExportDetail::GlSmokeFailed)
        })
        .and_then(|()| {
            egl.swap_buffers(display, surface)
                .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglSwapBuffersFailed)
        })
        .and_then(|()| {
            // `gbm` releases this lock when the returned BufferObject is
            // dropped. The owner retains the surface so the release callback
            // remains valid until KMS scanout has retired the buffer.
            let buffer = unsafe { gbm_surface.lock_front_buffer() }
                .map_err(|_error| NativeGbmScanoutBufferExportDetail::FrontBufferLockFailed)?;
            native_owned_scanout_buffer_from_bo(width, height, buffer, Some(gbm_surface))
        });
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_context(display, context);
    let _ = egl.destroy_surface(display, surface);

    result
}

#[derive(Clone, Copy)]
struct NativeXrgb8888Frame<'a> {
    stride: u32,
    pixels: &'a [u8],
}

#[derive(Clone)]
struct RenderedScanoutCandidate {
    format: gbm::Format,
    modifiers: Vec<gbm::Modifier>,
    usage: gbm::BufferObjectFlags,
    config_attributes: [khronos_egl::Int; 13],
}

fn create_rendered_scanout_surface<T: std::os::fd::AsFd>(
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
    format: gbm::Format,
    modifiers: &[gbm::Modifier],
    usage: gbm::BufferObjectFlags,
) -> Result<gbm::Surface<()>, NativeGbmScanoutBufferExportDetail> {
    if modifiers.is_empty() {
        gbm_device
            .create_surface::<()>(width, height, format, usage)
            .map_err(|_error| NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable)
    } else {
        gbm_device
            .create_surface_with_modifiers2::<()>(
                width,
                height,
                format,
                modifiers.iter().copied(),
                usage,
            )
            .map_err(|_error| NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable)
    }
}

fn choose_scanout_config_for_format(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    config_attributes: [khronos_egl::Int; 13],
    format: gbm::Format,
) -> Option<khronos_egl::Config> {
    let count = egl
        .matching_config_count(display, &config_attributes)
        .ok()?;
    let mut configs = Vec::with_capacity(count);
    egl.choose_config(display, &config_attributes, &mut configs)
        .ok()?;
    configs.into_iter().find(|config| {
        egl.get_config_attrib(display, *config, khronos_egl::NATIVE_VISUAL_ID)
            .ok()
            == Some(format as khronos_egl::Int)
    })
}

fn rendered_scanout_candidates(
    preferred_modifiers: &[gbm::Modifier],
) -> Vec<RenderedScanoutCandidate> {
    let mut candidates = Vec::with_capacity(6);
    if !preferred_modifiers.is_empty() {
        candidates.push(RenderedScanoutCandidate {
            format: gbm::Format::Xrgb8888,
            modifiers: preferred_modifiers.to_vec(),
            usage: rendered_scanout_usage(),
            config_attributes: xrgb_window_config_attributes(),
        });
    }
    candidates.extend([
        RenderedScanoutCandidate {
            format: gbm::Format::Xrgb8888,
            modifiers: Vec::from(LINEAR_SCANOUT_MODIFIERS),
            usage: rendered_scanout_usage().union(gbm::BufferObjectFlags::LINEAR),
            config_attributes: xrgb_window_config_attributes(),
        },
        RenderedScanoutCandidate {
            format: gbm::Format::Xrgb8888,
            modifiers: Vec::new(),
            usage: rendered_scanout_usage().union(gbm::BufferObjectFlags::LINEAR),
            config_attributes: xrgb_window_config_attributes(),
        },
        RenderedScanoutCandidate {
            format: gbm::Format::Xrgb8888,
            modifiers: Vec::new(),
            usage: rendered_scanout_usage(),
            config_attributes: xrgb_window_config_attributes(),
        },
        RenderedScanoutCandidate {
            format: gbm::Format::Argb8888,
            modifiers: Vec::from(LINEAR_SCANOUT_MODIFIERS),
            usage: rendered_scanout_usage().union(gbm::BufferObjectFlags::LINEAR),
            config_attributes: window_config_attributes(),
        },
        RenderedScanoutCandidate {
            format: gbm::Format::Argb8888,
            modifiers: Vec::new(),
            usage: rendered_scanout_usage(),
            config_attributes: window_config_attributes(),
        },
    ]);
    candidates
}

const LINEAR_SCANOUT_MODIFIERS: [gbm::Modifier; 1] = [gbm::Modifier::Linear];

fn reduced_gbm_scanout_modifiers(modifiers: &[u64]) -> Vec<gbm::Modifier> {
    let mut reduced = Vec::new();
    for modifier in modifiers.iter().copied().map(gbm::Modifier::from) {
        if matches!(modifier, gbm::Modifier::Invalid) || reduced.contains(&modifier) {
            continue;
        }
        reduced.push(modifier);
        if reduced.len() >= MAX_PREFERRED_SCANOUT_MODIFIERS {
            break;
        }
    }
    reduced
}

const MAX_PREFERRED_SCANOUT_MODIFIERS: usize = 16;

fn is_supported_rendered_scanout_candidate_buffer(buffer: &NativeGbmOwnedScanoutBuffer) -> bool {
    is_supported_rendered_scanout_candidate_shape(buffer.plane_count())
}

const fn is_supported_rendered_scanout_candidate_shape(plane_count: u8) -> bool {
    plane_count == 1
}

fn rendered_scanout_usage() -> gbm::BufferObjectFlags {
    gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING
}

fn preferred_scanout_failure_detail(
    current: NativeGbmScanoutBufferExportDetail,
    next: NativeGbmScanoutBufferExportDetail,
) -> NativeGbmScanoutBufferExportDetail {
    if current == NativeGbmScanoutBufferExportDetail::EglConfigUnavailable {
        next
    } else {
        current
    }
}

#[cfg(test)]
mod dmabuf_tests {
    use std::fs::File;
    use std::os::fd::AsFd;

    use super::NativeDmaBufFrame;

    #[test]
    fn validates_bounded_linear_xrgb_descriptor() {
        let file = File::open("/dev/null").unwrap();
        let valid = NativeDmaBufFrame {
            width: 64,
            height: 32,
            format: 0x3432_5258,
            modifier: 0,
            fd: file.as_fd(),
            offset: 0,
            stride: 256,
        };
        assert!(valid.is_valid());
        assert!(
            !NativeDmaBufFrame {
                stride: 64,
                ..valid
            }
            .is_valid()
        );
    }
}

fn is_supported_scanout_format(format: u32) -> bool {
    format == gbm::Format::Xrgb8888 as u32 || format == gbm::Format::Argb8888 as u32
}

fn exported_scanout_buffer_report(
    buffer: NativeGbmOwnedScanoutBuffer,
) -> NativeGbmOwnedScanoutBufferExportReport {
    NativeGbmOwnedScanoutBufferExportReport {
        status: NativeGbmScanoutBufferExportStatus::Exported,
        detail: NativeGbmScanoutBufferExportDetail::Exported,
        buffer: Some(buffer),
    }
}

fn failed_scanout_buffer_report(
    detail: NativeGbmScanoutBufferExportDetail,
) -> NativeGbmOwnedScanoutBufferExportReport {
    NativeGbmOwnedScanoutBufferExportReport {
        status: detail.status(),
        detail,
        buffer: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_scanout_candidate_shape_requires_single_plane() {
        assert!(is_supported_rendered_scanout_candidate_shape(1));
        assert!(!is_supported_rendered_scanout_candidate_shape(0));
        assert!(!is_supported_rendered_scanout_candidate_shape(2));
        assert!(!is_supported_rendered_scanout_candidate_shape(4));
    }
}

use std::{ffi::c_void, ptr};

use crate::gbm_platform::{
    EGL_PLATFORM_GBM_KHR,
    config::{window_config_attributes, xrgb_window_config_attributes},
};
use crate::gl::{context_attributes, smoke_current_gl_context_with_loader};
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
    modifier: Option<u64>,
    // Drop order matters: the locked front buffer must release before the
    // surface it was locked from is destroyed.
    _buffer: gbm::BufferObject<()>,
    _surface: Option<gbm::Surface<()>>,
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
        })
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
        ) {
            Ok(buffer) => exported_scanout_buffer_report(buffer),
            Err(detail) => failed_scanout_buffer_report(detail),
        }
    }
}

impl<T> Drop for NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    fn drop(&mut self) {
        let _ = self.egl.terminate(self.display);
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
        modifier: normalized_scanout_modifier(buffer.modifier()),
        _buffer: buffer,
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
        ) {
            Ok(buffer) => return Ok(buffer),
            Err(detail) => last_detail = preferred_scanout_failure_detail(last_detail, detail),
        }
    }

    Err(last_detail)
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
            smoke_current_gl_context_with_loader(|name| {
                egl.get_proc_address(name)
                    .map_or(ptr::null(), |proc| proc as *const c_void)
            })
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

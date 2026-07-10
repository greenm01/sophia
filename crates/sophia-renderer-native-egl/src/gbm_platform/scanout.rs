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

    match render_gbm_scanout_front_buffer(device, width, height) {
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
    if pitch == 0 || gem_handle == 0 || !is_supported_scanout_format(format) {
        return Err(NativeGbmScanoutBufferExportDetail::InvalidBufferDescriptor);
    }

    Ok(NativeGbmOwnedScanoutBuffer {
        width,
        height,
        pitch,
        format,
        gem_handle,
        _buffer: buffer,
        _surface: surface,
    })
}

fn render_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    device: T,
    width: u32,
    height: u32,
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
    let result =
        render_initialized_gbm_scanout_front_buffer(&egl, display, &gbm_device, width, height);
    let _ = egl.terminate(display);
    result
}

fn render_initialized_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::EglBindApiFailed)?;

    let mut last_detail = NativeGbmScanoutBufferExportDetail::EglConfigUnavailable;
    for candidate in rendered_scanout_candidates() {
        let config = match egl.choose_first_config(display, &candidate.config_attributes) {
            Ok(Some(config)) => config,
            Ok(None) | Err(_) => continue,
        };

        match render_initialized_gbm_scanout_front_buffer_with_config(
            egl,
            display,
            gbm_device,
            width,
            height,
            config,
            candidate.format,
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
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportDetail> {
    use gbm::AsRaw as _;

    let gbm_surface = gbm_device
        .create_surface::<()>(
            width,
            height,
            surface_format,
            gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
        )
        .map_err(|_error| NativeGbmScanoutBufferExportDetail::GbmSurfaceUnavailable)?;
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

#[derive(Clone, Copy)]
struct RenderedScanoutCandidate {
    format: gbm::Format,
    config_attributes: [khronos_egl::Int; 13],
}

fn rendered_scanout_candidates() -> [RenderedScanoutCandidate; 2] {
    [
        RenderedScanoutCandidate {
            format: gbm::Format::Xrgb8888,
            config_attributes: xrgb_window_config_attributes(),
        },
        RenderedScanoutCandidate {
            format: gbm::Format::Argb8888,
            config_attributes: window_config_attributes(),
        },
    ]
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

use glow::HasContext;
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

#[cfg(feature = "gbm-platform")]
const EGL_PLATFORM_GBM_KHR: khronos_egl::Enum = 0x31D7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEglProbeStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEglDrawSmokeStatus {
    ClearColorReady,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
    SurfaceUnavailable,
    MakeCurrentUnavailable,
    GlUnavailable,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmBackedEglPlatformStatus {
    NativePlatformCapable,
    PlatformUnavailable,
    PlatformDegraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePresentationSmokeStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmEglFrameTargetAllocationStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmScanoutBufferExportStatus {
    Exported,
    InvalidTarget,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBuffer {
    width: u32,
    height: u32,
    pitch: u32,
    format: u32,
    gem_handle: u32,
    _buffer: gbm::BufferObject<()>,
    _surface: Option<gbm::Surface<()>>,
}

#[cfg(feature = "gbm-platform")]
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

#[cfg(feature = "gbm-platform")]
#[derive(Debug)]
pub struct NativeGbmOwnedScanoutBufferExportReport {
    pub status: NativeGbmScanoutBufferExportStatus,
    pub buffer: Option<NativeGbmOwnedScanoutBuffer>,
}

#[cfg(feature = "gbm-platform")]
pub struct NativeGbmRenderedScanoutContext<T: std::os::fd::AsFd> {
    egl: khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: gbm::Device<T>,
}

#[cfg(feature = "gbm-platform")]
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
            Ok(buffer) => NativeGbmOwnedScanoutBufferExportReport {
                status: NativeGbmScanoutBufferExportStatus::Exported,
                buffer: Some(buffer),
            },
            Err(status) => NativeGbmOwnedScanoutBufferExportReport {
                status,
                buffer: None,
            },
        }
    }
}

#[cfg(feature = "gbm-platform")]
impl<T> Drop for NativeGbmRenderedScanoutContext<T>
where
    T: std::os::fd::AsFd,
{
    fn drop(&mut self) {
        let _ = self.egl.terminate(self.display);
    }
}

#[cfg(feature = "gbm-platform")]
pub struct NativeGbmRenderedScanoutContextReport<T: std::os::fd::AsFd> {
    pub status: NativeGbmRenderedScanoutContextStatus,
    pub context: Option<NativeGbmRenderedScanoutContext<T>>,
}

pub fn probe_default_display_context() -> NativeEglProbeStatus {
    match probe_context() {
        Ok(()) => NativeEglProbeStatus::NativeDrawingCapable,
        Err(error) => error,
    }
}

pub fn smoke_default_display_pbuffer() -> NativeEglDrawSmokeStatus {
    match smoke_pbuffer() {
        Ok(()) => NativeEglDrawSmokeStatus::ClearColorReady,
        Err(error) => error,
    }
}

#[cfg(feature = "gbm-platform")]
pub fn probe_gbm_backed_platform_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
) -> NativeGbmBackedEglPlatformStatus {
    match device {
        Ok(device) => match probe_gbm_backed_platform(device) {
            Ok(()) => NativeGbmBackedEglPlatformStatus::NativePlatformCapable,
            Err(error) => error,
        },
        Err(_error) => NativeGbmBackedEglPlatformStatus::PlatformUnavailable,
    }
}

#[cfg(feature = "gbm-platform")]
pub fn smoke_gbm_backed_private_target_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
) -> NativeEglDrawSmokeStatus {
    match device {
        Ok(device) => match smoke_gbm_backed_private_target(device, GbmTargetAction::ClearOnly) {
            Ok(()) => NativeEglDrawSmokeStatus::ClearColorReady,
            Err(error) => error,
        },
        Err(_error) => NativeEglDrawSmokeStatus::PlatformUnavailable,
    }
}

#[cfg(feature = "gbm-platform")]
pub fn present_gbm_backed_offscreen_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
) -> NativePresentationSmokeStatus {
    match device {
        Ok(device) => {
            match smoke_gbm_backed_private_target(device, GbmTargetAction::SwapAfterClear) {
                Ok(()) => NativePresentationSmokeStatus::Ready,
                Err(NativeEglDrawSmokeStatus::PlatformUnavailable) => {
                    NativePresentationSmokeStatus::Unavailable
                }
                Err(_error) => NativePresentationSmokeStatus::Degraded,
            }
        }
        Err(_error) => NativePresentationSmokeStatus::Unavailable,
    }
}

#[cfg(feature = "gbm-platform")]
pub fn allocate_gbm_backed_frame_target_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
    width: u32,
    height: u32,
) -> NativeGbmEglFrameTargetAllocationStatus {
    match device {
        Ok(device) => match smoke_gbm_backed_private_target_with_size(
            device,
            GbmTargetAction::ClearOnly,
            width,
            height,
        ) {
            Ok(()) => NativeGbmEglFrameTargetAllocationStatus::Ready,
            Err(NativeEglDrawSmokeStatus::PlatformUnavailable) => {
                NativeGbmEglFrameTargetAllocationStatus::Unavailable
            }
            Err(_error) => NativeGbmEglFrameTargetAllocationStatus::Degraded,
        },
        Err(_error) => NativeGbmEglFrameTargetAllocationStatus::Unavailable,
    }
}

#[cfg(feature = "gbm-platform")]
pub fn export_gbm_scanout_buffer_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
    width: u32,
    height: u32,
) -> NativeGbmOwnedScanoutBufferExportReport {
    if width == 0 || height == 0 {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
            buffer: None,
        };
    }

    let Ok(device) = device else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
            buffer: None,
        };
    };
    let Ok(device) = gbm::Device::new(device) else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
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
            buffer: None,
        };
    };

    match native_owned_scanout_buffer_from_bo(width, height, buffer, None) {
        Ok(buffer) => NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Exported,
            buffer: Some(buffer),
        },
        Err(status) => NativeGbmOwnedScanoutBufferExportReport {
            status,
            buffer: None,
        },
    }
}

#[cfg(feature = "gbm-platform")]
pub fn export_rendered_gbm_scanout_buffer_from_backend_device_result<T: std::os::fd::AsFd>(
    device: std::io::Result<T>,
    width: u32,
    height: u32,
) -> NativeGbmOwnedScanoutBufferExportReport {
    if width == 0 || height == 0 {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::InvalidTarget,
            buffer: None,
        };
    }

    let Ok(device) = device else {
        return NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Unavailable,
            buffer: None,
        };
    };

    match render_gbm_scanout_front_buffer(device, width, height) {
        Ok(buffer) => NativeGbmOwnedScanoutBufferExportReport {
            status: NativeGbmScanoutBufferExportStatus::Exported,
            buffer: Some(buffer),
        },
        Err(status) => NativeGbmOwnedScanoutBufferExportReport {
            status,
            buffer: None,
        },
    }
}

#[cfg(feature = "gbm-platform")]
fn native_owned_scanout_buffer_from_bo(
    width: u32,
    height: u32,
    buffer: gbm::BufferObject<()>,
    surface: Option<gbm::Surface<()>>,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportStatus> {
    let pitch = buffer.stride();
    let format = buffer.format() as u32;
    let gem_handle = unsafe { buffer.handle().u32_ };
    if pitch == 0 || gem_handle == 0 || format != gbm::Format::Xrgb8888 as u32 {
        return Err(NativeGbmScanoutBufferExportStatus::Degraded);
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

#[cfg(feature = "gbm-platform")]
fn render_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    device: T,
    width: u32,
    height: u32,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportStatus> {
    use gbm::AsRaw as _;

    let gbm_device = gbm::Device::new(device)
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Unavailable)?;
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Unavailable)?;

    let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
    let display = unsafe {
        egl.get_platform_display(
            EGL_PLATFORM_GBM_KHR,
            native_display,
            &[khronos_egl::ATTRIB_NONE],
        )
    }
    .map_err(|_error| NativeGbmScanoutBufferExportStatus::Unavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)?;
    let result =
        render_initialized_gbm_scanout_front_buffer(&egl, display, &gbm_device, width, height);
    let _ = egl.terminate(display);
    result
}

#[cfg(feature = "gbm-platform")]
fn render_initialized_gbm_scanout_front_buffer<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    width: u32,
    height: u32,
) -> Result<NativeGbmOwnedScanoutBuffer, NativeGbmScanoutBufferExportStatus> {
    use gbm::AsRaw as _;

    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)?;

    let config = egl
        .choose_first_config(display, &xrgb_window_config_attributes())
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)?
        .ok_or(NativeGbmScanoutBufferExportStatus::Degraded)?;
    let gbm_surface = gbm_device
        .create_surface::<()>(
            width,
            height,
            gbm::Format::Xrgb8888,
            gbm::BufferObjectFlags::SCANOUT | gbm::BufferObjectFlags::RENDERING,
        )
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Unavailable)?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let surface = unsafe { egl.create_window_surface(display, config, native_window, None) }
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)?;
    let context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_error) => {
            let _ = egl.destroy_surface(display, surface);
            return Err(NativeGbmScanoutBufferExportStatus::Degraded);
        }
    };

    let result = egl
        .make_current(display, Some(surface), Some(surface), Some(context))
        .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)
        .and_then(|()| {
            smoke_current_gl_context_with_loader(|name| {
                egl.get_proc_address(name)
                    .map_or(ptr::null(), |proc| proc as *const c_void)
            })
            .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)
        })
        .and_then(|()| {
            egl.swap_buffers(display, surface)
                .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)
        })
        .and_then(|()| {
            let buffer = unsafe { gbm_surface.lock_front_buffer() }
                .map_err(|_error| NativeGbmScanoutBufferExportStatus::Degraded)?;
            native_owned_scanout_buffer_from_bo(width, height, buffer, Some(gbm_surface))
        });
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_context(display, context);
    let _ = egl.destroy_surface(display, surface);

    result
}

fn probe_context() -> Result<(), NativeEglProbeStatus> {
    // The loaded library is not trusted beyond this adapter; all failures reduce
    // to NativeEglProbeStatus before returning to safe Sophia crates.
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required() }
        .map_err(|_error| NativeEglProbeStatus::PlatformUnavailable)?;
    // DEFAULT_DISPLAY is the only native display token used by this probe and is
    // never exposed outside the adapter.
    let display = unsafe { egl.get_display(khronos_egl::DEFAULT_DISPLAY) }
        .ok_or(NativeEglProbeStatus::PlatformUnavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeEglProbeStatus::PlatformDegraded)?;
    let result = probe_initialized_context(&egl, display);
    let _ = egl.terminate(display);
    result
}

fn smoke_pbuffer() -> Result<(), NativeEglDrawSmokeStatus> {
    // The loaded library is not trusted beyond this adapter; all failures reduce
    // to NativeEglDrawSmokeStatus before returning to safe Sophia crates.
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required() }
        .map_err(|_error| NativeEglDrawSmokeStatus::PlatformUnavailable)?;
    // DEFAULT_DISPLAY is the only native display token used by this smoke and
    // is never exposed outside the adapter.
    let display = unsafe { egl.get_display(khronos_egl::DEFAULT_DISPLAY) }
        .ok_or(NativeEglDrawSmokeStatus::PlatformUnavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeEglDrawSmokeStatus::PlatformDegraded)?;
    let result = smoke_initialized_pbuffer(&egl, display);
    let _ = egl.terminate(display);
    result
}

fn probe_initialized_context(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_4>,
    display: khronos_egl::Display,
) -> Result<(), NativeEglProbeStatus> {
    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeEglProbeStatus::ContextUnavailable)?;

    let config = egl
        .choose_first_config(display, &config_attributes())
        .map_err(|_error| NativeEglProbeStatus::ContextUnavailable)?
        .ok_or(NativeEglProbeStatus::ContextUnavailable)?;
    let context = egl
        .create_context(display, config, None, &context_attributes())
        .map_err(|_error| NativeEglProbeStatus::ContextUnavailable)?;
    let _ = egl.destroy_context(display, context);

    Ok(())
}

fn smoke_initialized_pbuffer(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_4>,
    display: khronos_egl::Display,
) -> Result<(), NativeEglDrawSmokeStatus> {
    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeEglDrawSmokeStatus::ContextUnavailable)?;

    let config = egl
        .choose_first_config(display, &config_attributes())
        .map_err(|_error| NativeEglDrawSmokeStatus::ContextUnavailable)?
        .ok_or(NativeEglDrawSmokeStatus::ContextUnavailable)?;
    let surface = egl
        .create_pbuffer_surface(display, config, &pbuffer_attributes())
        .map_err(|_error| NativeEglDrawSmokeStatus::SurfaceUnavailable)?;
    let context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_error) => {
            let _ = egl.destroy_surface(display, surface);
            return Err(NativeEglDrawSmokeStatus::ContextUnavailable);
        }
    };

    let result = egl
        .make_current(display, Some(surface), Some(surface), Some(context))
        .map_err(|_error| NativeEglDrawSmokeStatus::MakeCurrentUnavailable)
        .and_then(|()| smoke_current_gl_context(egl));
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_context(display, context);
    let _ = egl.destroy_surface(display, surface);

    result
}

fn smoke_current_gl_context(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_4>,
) -> Result<(), NativeEglDrawSmokeStatus> {
    smoke_current_gl_context_with_loader(|name| {
        egl.get_proc_address(name)
            .map_or(ptr::null(), |proc| proc as *const c_void)
    })
}

fn smoke_current_gl_context_with_loader<F>(mut loader: F) -> Result<(), NativeEglDrawSmokeStatus>
where
    F: FnMut(&str) -> *const c_void,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        // GL function pointers are loaded only after the EGL context is current
        // and never escape this adapter.
        let gl = unsafe { glow::Context::from_loader_function(|name| loader(name)) };

        unsafe {
            gl.clear_color(0.02, 0.03, 0.05, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.finish();
            gl.get_error()
        }
    }))
    .map_err(|_panic| NativeEglDrawSmokeStatus::GlUnavailable)?;

    if result == glow::NO_ERROR {
        Ok(())
    } else {
        Err(NativeEglDrawSmokeStatus::GlUnavailable)
    }
}

#[cfg(feature = "gbm-platform")]
fn probe_gbm_backed_platform<T: std::os::fd::AsFd>(
    device: T,
) -> Result<(), NativeGbmBackedEglPlatformStatus> {
    use gbm::AsRaw as _;

    let gbm_device = gbm::Device::new(device)
        .map_err(|_error| NativeGbmBackedEglPlatformStatus::PlatformDegraded)?;
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
        .map_err(|_error| NativeGbmBackedEglPlatformStatus::PlatformUnavailable)?;

    let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
    let display = unsafe {
        egl.get_platform_display(
            EGL_PLATFORM_GBM_KHR,
            native_display,
            &[khronos_egl::ATTRIB_NONE],
        )
    }
    .map_err(|_error| NativeGbmBackedEglPlatformStatus::PlatformUnavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeGbmBackedEglPlatformStatus::PlatformDegraded)?;
    let _ = egl.terminate(display);

    Ok(())
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GbmTargetAction {
    ClearOnly,
    SwapAfterClear,
}

#[cfg(feature = "gbm-platform")]
fn smoke_gbm_backed_private_target<T: std::os::fd::AsFd>(
    device: T,
    action: GbmTargetAction,
) -> Result<(), NativeEglDrawSmokeStatus> {
    smoke_gbm_backed_private_target_with_size(device, action, 1, 1)
}

#[cfg(feature = "gbm-platform")]
fn smoke_gbm_backed_private_target_with_size<T: std::os::fd::AsFd>(
    device: T,
    action: GbmTargetAction,
    width: u32,
    height: u32,
) -> Result<(), NativeEglDrawSmokeStatus> {
    use gbm::AsRaw as _;

    let gbm_device =
        gbm::Device::new(device).map_err(|_error| NativeEglDrawSmokeStatus::PlatformDegraded)?;
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_5>::load_required() }
        .map_err(|_error| NativeEglDrawSmokeStatus::PlatformUnavailable)?;

    let native_display = gbm_device.as_raw() as khronos_egl::NativeDisplayType;
    let display = unsafe {
        egl.get_platform_display(
            EGL_PLATFORM_GBM_KHR,
            native_display,
            &[khronos_egl::ATTRIB_NONE],
        )
    }
    .map_err(|_error| NativeEglDrawSmokeStatus::PlatformUnavailable)?;

    egl.initialize(display)
        .map_err(|_error| NativeEglDrawSmokeStatus::PlatformDegraded)?;
    let result =
        smoke_initialized_gbm_private_target(&egl, display, &gbm_device, action, width, height);
    let _ = egl.terminate(display);
    result
}

#[cfg(feature = "gbm-platform")]
fn smoke_initialized_gbm_private_target<T: std::os::fd::AsFd>(
    egl: &khronos_egl::DynamicInstance<khronos_egl::EGL1_5>,
    display: khronos_egl::Display,
    gbm_device: &gbm::Device<T>,
    action: GbmTargetAction,
    width: u32,
    height: u32,
) -> Result<(), NativeEglDrawSmokeStatus> {
    use gbm::AsRaw as _;

    egl.bind_api(khronos_egl::OPENGL_API)
        .map_err(|_error| NativeEglDrawSmokeStatus::ContextUnavailable)?;

    let config = egl
        .choose_first_config(display, &window_config_attributes())
        .map_err(|_error| NativeEglDrawSmokeStatus::ContextUnavailable)?
        .ok_or(NativeEglDrawSmokeStatus::ContextUnavailable)?;
    let gbm_surface = gbm_device
        .create_surface::<()>(
            width,
            height,
            gbm::Format::Argb8888,
            gbm::BufferObjectFlags::RENDERING,
        )
        .map_err(|_error| NativeEglDrawSmokeStatus::SurfaceUnavailable)?;
    let native_window = gbm_surface.as_raw() as khronos_egl::NativeWindowType;
    let surface = unsafe { egl.create_window_surface(display, config, native_window, None) }
        .map_err(|_error| NativeEglDrawSmokeStatus::SurfaceUnavailable)?;
    let context = match egl.create_context(display, config, None, &context_attributes()) {
        Ok(context) => context,
        Err(_error) => {
            let _ = egl.destroy_surface(display, surface);
            return Err(NativeEglDrawSmokeStatus::ContextUnavailable);
        }
    };

    let result = egl
        .make_current(display, Some(surface), Some(surface), Some(context))
        .map_err(|_error| NativeEglDrawSmokeStatus::MakeCurrentUnavailable)
        .and_then(|()| {
            smoke_current_gl_context_with_loader(|name| {
                egl.get_proc_address(name)
                    .map_or(ptr::null(), |proc| proc as *const c_void)
            })
        })
        .and_then(|()| {
            if action == GbmTargetAction::SwapAfterClear {
                egl.swap_buffers(display, surface)
                    .map_err(|_error| NativeEglDrawSmokeStatus::SurfaceUnavailable)
            } else {
                Ok(())
            }
        });
    let _ = egl.make_current(display, None, None, None);
    let _ = egl.destroy_context(display, context);
    let _ = egl.destroy_surface(display, surface);

    result
}

fn config_attributes() -> [khronos_egl::Int; 13] {
    [
        khronos_egl::SURFACE_TYPE,
        khronos_egl::PBUFFER_BIT,
        khronos_egl::RENDERABLE_TYPE,
        khronos_egl::OPENGL_BIT,
        khronos_egl::RED_SIZE,
        8,
        khronos_egl::GREEN_SIZE,
        8,
        khronos_egl::BLUE_SIZE,
        8,
        khronos_egl::ALPHA_SIZE,
        8,
        khronos_egl::NONE,
    ]
}

#[cfg(feature = "gbm-platform")]
fn window_config_attributes() -> [khronos_egl::Int; 13] {
    [
        khronos_egl::SURFACE_TYPE,
        khronos_egl::WINDOW_BIT,
        khronos_egl::RENDERABLE_TYPE,
        khronos_egl::OPENGL_BIT,
        khronos_egl::RED_SIZE,
        8,
        khronos_egl::GREEN_SIZE,
        8,
        khronos_egl::BLUE_SIZE,
        8,
        khronos_egl::ALPHA_SIZE,
        8,
        khronos_egl::NONE,
    ]
}

#[cfg(feature = "gbm-platform")]
fn xrgb_window_config_attributes() -> [khronos_egl::Int; 13] {
    [
        khronos_egl::SURFACE_TYPE,
        khronos_egl::WINDOW_BIT,
        khronos_egl::RENDERABLE_TYPE,
        khronos_egl::OPENGL_BIT,
        khronos_egl::RED_SIZE,
        8,
        khronos_egl::GREEN_SIZE,
        8,
        khronos_egl::BLUE_SIZE,
        8,
        khronos_egl::ALPHA_SIZE,
        0,
        khronos_egl::NONE,
    ]
}

fn context_attributes() -> [khronos_egl::Int; 1] {
    [khronos_egl::NONE]
}

fn pbuffer_attributes() -> [khronos_egl::Int; 5] {
    [
        khronos_egl::WIDTH,
        1,
        khronos_egl::HEIGHT,
        1,
        khronos_egl::NONE,
    ]
}

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

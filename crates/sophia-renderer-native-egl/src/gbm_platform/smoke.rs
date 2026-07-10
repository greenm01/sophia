use std::{ffi::c_void, ptr};

use crate::gbm_platform::{EGL_PLATFORM_GBM_KHR, config::window_config_attributes};
use crate::gl::{context_attributes, smoke_current_gl_context_with_loader};
use crate::{
    NativeEglDrawSmokeStatus, NativeGbmBackedEglPlatformStatus,
    NativeGbmEglFrameTargetAllocationStatus, NativePresentationSmokeStatus,
};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GbmTargetAction {
    ClearOnly,
    SwapAfterClear,
}

fn smoke_gbm_backed_private_target<T: std::os::fd::AsFd>(
    device: T,
    action: GbmTargetAction,
) -> Result<(), NativeEglDrawSmokeStatus> {
    smoke_gbm_backed_private_target_with_size(device, action, 1, 1)
}

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

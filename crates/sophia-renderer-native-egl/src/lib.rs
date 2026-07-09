#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEglProbeStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

pub fn probe_default_display_context() -> NativeEglProbeStatus {
    match probe_context() {
        Ok(()) => NativeEglProbeStatus::NativeDrawingCapable,
        Err(error) => error,
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

fn context_attributes() -> [khronos_egl::Int; 1] {
    [khronos_egl::NONE]
}

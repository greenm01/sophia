use glow::HasContext;
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};

use crate::NativeEglDrawSmokeStatus;

pub(crate) fn smoke_current_gl_context_with_loader<F>(
    mut loader: F,
) -> Result<(), NativeEglDrawSmokeStatus>
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

pub(crate) fn context_attributes() -> [khronos_egl::Int; 1] {
    [khronos_egl::NONE]
}

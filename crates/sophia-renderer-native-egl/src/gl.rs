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

#[cfg(feature = "gbm-platform")]
pub(crate) fn draw_xrgb8888_current_gl_context_with_loader<F>(
    mut loader: F,
    width: u32,
    height: u32,
    stride: u32,
    pixels: &[u8],
) -> Result<(), NativeEglDrawSmokeStatus>
where
    F: FnMut(&str) -> *const c_void,
{
    let expected_stride = width
        .checked_mul(4)
        .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
    let expected_len = usize::try_from(expected_stride)
        .ok()
        .and_then(|stride| stride.checked_mul(usize::try_from(height).ok()?))
        .ok_or(NativeEglDrawSmokeStatus::GlUnavailable)?;
    if width == 0 || height == 0 || stride != expected_stride || pixels.len() != expected_len {
        return Err(NativeEglDrawSmokeStatus::GlUnavailable);
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let gl = unsafe { glow::Context::from_loader_function(|name| loader(name)) };
        unsafe { draw_xrgb8888_frame(&gl, width, height, pixels) }
    }))
    .map_err(|_panic| NativeEglDrawSmokeStatus::GlUnavailable)?;

    result.map_err(|_error| NativeEglDrawSmokeStatus::GlUnavailable)
}

#[cfg(feature = "gbm-platform")]
unsafe fn draw_xrgb8888_frame(
    gl: &glow::Context,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), String> {
    let vertex_shader = unsafe { compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)? };
    let fragment_shader =
        match unsafe { compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER) } {
            Ok(shader) => shader,
            Err(error) => {
                unsafe { gl.delete_shader(vertex_shader) };
                return Err(error);
            }
        };
    let program = unsafe { gl.create_program()? };
    unsafe {
        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.bind_attrib_location(program, 0, "position");
        gl.bind_attrib_location(program, 1, "texture_coordinate");
        gl.link_program(program);
    }
    if !unsafe { gl.get_program_link_status(program) } {
        let error = unsafe { gl.get_program_info_log(program) };
        unsafe {
            gl.delete_program(program);
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);
        }
        return Err(error);
    }

    let texture = unsafe { gl.create_texture()? };
    let vertex_buffer = unsafe { gl.create_buffer()? };
    let vertices: [f32; 16] = [
        -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0,
    ];
    let vertex_bytes = unsafe {
        std::slice::from_raw_parts(
            vertices.as_ptr().cast::<u8>(),
            vertices.len() * std::mem::size_of::<f32>(),
        )
    };

    unsafe {
        gl.viewport(0, 0, width as i32, height as i32);
        gl.disable(glow::BLEND);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width as i32,
            height as i32,
            0,
            glow::BGRA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(pixels)),
        );
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STATIC_DRAW);
        gl.use_program(Some(program));
        gl.uniform_1_i32(gl.get_uniform_location(program, "frame").as_ref(), 0);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        gl.finish();
    }
    let gl_error = unsafe { gl.get_error() };
    unsafe {
        gl.disable_vertex_attrib_array(0);
        gl.disable_vertex_attrib_array(1);
        gl.bind_buffer(glow::ARRAY_BUFFER, None);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.use_program(None);
        gl.delete_buffer(vertex_buffer);
        gl.delete_texture(texture);
        gl.delete_program(program);
        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);
    }
    if gl_error == glow::NO_ERROR {
        Ok(())
    } else {
        Err(format!(
            "OpenGL frame upload failed with error {gl_error:#x}"
        ))
    }
}

#[cfg(feature = "gbm-platform")]
unsafe fn compile_shader(
    gl: &glow::Context,
    shader_type: u32,
    source: &str,
) -> Result<glow::Shader, String> {
    let shader = unsafe { gl.create_shader(shader_type)? };
    unsafe {
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
    }
    if unsafe { gl.get_shader_compile_status(shader) } {
        Ok(shader)
    } else {
        let error = unsafe { gl.get_shader_info_log(shader) };
        unsafe { gl.delete_shader(shader) };
        Err(error)
    }
}

#[cfg(feature = "gbm-platform")]
const VERTEX_SHADER: &str = r#"#version 110
attribute vec2 position;
attribute vec2 texture_coordinate;
varying vec2 texture_position;
void main() {
    texture_position = texture_coordinate;
    gl_Position = vec4(position, 0.0, 1.0);
}
"#;

#[cfg(feature = "gbm-platform")]
const FRAGMENT_SHADER: &str = r#"#version 110
uniform sampler2D frame;
varying vec2 texture_position;
void main() {
    gl_FragColor = texture2D(frame, texture_position);
}
"#;

pub(crate) fn context_attributes() -> [khronos_egl::Int; 1] {
    [khronos_egl::NONE]
}

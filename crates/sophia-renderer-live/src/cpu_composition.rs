use sophia_protocol::{Rect, Size};

use crate::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuBufferSource {
    pub handle: u64,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuCompositionLayer {
    pub geometry: Rect,
    pub buffer: LiveCpuBufferSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuComposedFrame {
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuCompositionReport {
    pub frame: LiveCpuComposedFrame,
    pub layers_input: usize,
    pub layers_composed: usize,
    pub nonzero_pixel_bytes: usize,
    pub checksum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveCpuCompositionError {
    InvalidOutputSize,
    OutputTooLarge,
}

pub fn compose_live_cpu_frame(
    output_size: Size,
    layers: &[LiveCpuCompositionLayer],
) -> Result<LiveCpuCompositionReport, LiveCpuCompositionError> {
    let width = usize::try_from(output_size.width)
        .ok()
        .filter(|width| *width > 0)
        .ok_or(LiveCpuCompositionError::InvalidOutputSize)?;
    let height = usize::try_from(output_size.height)
        .ok()
        .filter(|height| *height > 0)
        .ok_or(LiveCpuCompositionError::InvalidOutputSize)?;
    let stride = width
        .checked_mul(4)
        .ok_or(LiveCpuCompositionError::OutputTooLarge)?;
    let byte_len = stride
        .checked_mul(height)
        .filter(|len| *len <= 64 * 1024 * 1024)
        .ok_or(LiveCpuCompositionError::OutputTooLarge)?;
    let mut frame = LiveCpuComposedFrame {
        size: output_size,
        stride: u32::try_from(stride).map_err(|_| LiveCpuCompositionError::OutputTooLarge)?,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        bytes: vec![0; byte_len],
    };
    let mut layers_composed = 0usize;
    for layer in layers {
        if compose_layer(&mut frame, layer) {
            layers_composed = layers_composed.saturating_add(1);
        }
    }
    let nonzero_pixel_bytes = frame.bytes.iter().filter(|byte| **byte != 0).count();
    let checksum = frame
        .bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        });
    Ok(LiveCpuCompositionReport {
        frame,
        layers_input: layers.len(),
        layers_composed,
        nonzero_pixel_bytes,
        checksum,
    })
}

fn compose_layer(frame: &mut LiveCpuComposedFrame, layer: &LiveCpuCompositionLayer) -> bool {
    if layer.buffer.format != LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
        || layer.geometry.width <= 0
        || layer.geometry.height <= 0
        || layer.buffer.size.width <= 0
        || layer.buffer.size.height <= 0
    {
        return false;
    }
    let source_width = usize::try_from(layer.buffer.size.width).unwrap_or(0);
    let source_height = usize::try_from(layer.buffer.size.height).unwrap_or(0);
    let source_stride = usize::try_from(layer.buffer.stride).unwrap_or(0);
    if source_stride < source_width.saturating_mul(4)
        || layer.buffer.bytes.len() < source_stride.saturating_mul(source_height)
    {
        return false;
    }
    let frame_width = usize::try_from(frame.size.width).unwrap_or(0);
    let frame_height = usize::try_from(frame.size.height).unwrap_or(0);
    let target_stride = usize::try_from(frame.stride).unwrap_or(0);
    let copy_width = usize::try_from(layer.geometry.width)
        .unwrap_or(0)
        .min(source_width);
    let copy_height = usize::try_from(layer.geometry.height)
        .unwrap_or(0)
        .min(source_height);
    let mut copied = false;
    for source_y in 0..copy_height {
        let target_y = layer
            .geometry
            .y
            .saturating_add(i32::try_from(source_y).unwrap_or(i32::MAX));
        if target_y < 0 || usize::try_from(target_y).unwrap_or(frame_height) >= frame_height {
            continue;
        }
        for source_x in 0..copy_width {
            let target_x = layer
                .geometry
                .x
                .saturating_add(i32::try_from(source_x).unwrap_or(i32::MAX));
            if target_x < 0 || usize::try_from(target_x).unwrap_or(frame_width) >= frame_width {
                continue;
            }
            let source_offset = source_y
                .saturating_mul(source_stride)
                .saturating_add(source_x.saturating_mul(4));
            let target_offset = usize::try_from(target_y)
                .unwrap_or(0)
                .saturating_mul(target_stride)
                .saturating_add(usize::try_from(target_x).unwrap_or(0).saturating_mul(4));
            let Some(source) = layer
                .buffer
                .bytes
                .get(source_offset..source_offset.saturating_add(4))
            else {
                continue;
            };
            let Some(target) = frame
                .bytes
                .get_mut(target_offset..target_offset.saturating_add(4))
            else {
                continue;
            };
            target.copy_from_slice(source);
            copied = true;
        }
    }
    copied
}

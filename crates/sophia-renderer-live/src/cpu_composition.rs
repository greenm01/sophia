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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveCpuBufferSourceRef<'a> {
    pub handle: u64,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveCpuCompositionLayerRef<'a> {
    pub geometry: Rect,
    pub buffer: LiveCpuBufferSourceRef<'a>,
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
    let borrowed = layers
        .iter()
        .map(|layer| LiveCpuCompositionLayerRef {
            geometry: layer.geometry,
            buffer: LiveCpuBufferSourceRef {
                handle: layer.buffer.handle,
                size: layer.buffer.size,
                stride: layer.buffer.stride,
                format: layer.buffer.format,
                generation: layer.buffer.generation,
                bytes: &layer.buffer.bytes,
            },
        })
        .collect::<Vec<_>>();
    compose_live_cpu_frame_ref(output_size, &borrowed)
}

pub fn compose_live_cpu_frame_ref(
    output_size: Size,
    layers: &[LiveCpuCompositionLayerRef<'_>],
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
    let frame_stride =
        u32::try_from(stride).map_err(|_| LiveCpuCompositionError::OutputTooLarge)?;
    let direct = layers.first().filter(|layer| {
        layers.len() == 1
            && layer.geometry
                == (Rect {
                    x: 0,
                    y: 0,
                    width: output_size.width,
                    height: output_size.height,
                })
            && layer.buffer.size == output_size
            && layer.buffer.stride == frame_stride
            && layer.buffer.format == LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
            && layer.buffer.bytes.len() == byte_len
    });
    let mut frame = LiveCpuComposedFrame {
        size: output_size,
        stride: frame_stride,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        bytes: direct.map_or_else(|| vec![0; byte_len], |layer| layer.buffer.bytes.to_vec()),
    };
    let mut layers_composed = 0usize;
    if direct.is_some() {
        layers_composed = 1;
    } else {
        for layer in layers {
            if compose_layer(&mut frame, layer) {
                layers_composed = layers_composed.saturating_add(1);
            }
        }
    }
    let (nonzero_pixel_bytes, checksum) = frame.bytes.iter().fold(
        (0usize, 0xcbf2_9ce4_8422_2325u64),
        |(nonzero, hash), byte| {
            (
                nonzero.saturating_add(usize::from(*byte != 0)),
                (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3),
            )
        },
    );
    Ok(LiveCpuCompositionReport {
        frame,
        layers_input: layers.len(),
        layers_composed,
        nonzero_pixel_bytes,
        checksum,
    })
}

fn compose_layer(frame: &mut LiveCpuComposedFrame, layer: &LiveCpuCompositionLayerRef<'_>) -> bool {
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
    let source_x = usize::try_from(layer.geometry.x.saturating_neg()).unwrap_or(0);
    let source_y = usize::try_from(layer.geometry.y.saturating_neg()).unwrap_or(0);
    let target_x = usize::try_from(layer.geometry.x.max(0)).unwrap_or(frame_width);
    let target_y = usize::try_from(layer.geometry.y.max(0)).unwrap_or(frame_height);
    if source_x >= source_width
        || source_y >= source_height
        || target_x >= frame_width
        || target_y >= frame_height
    {
        return false;
    }
    let copy_width = usize::try_from(layer.geometry.width)
        .unwrap_or(0)
        .saturating_sub(source_x)
        .min(source_width.saturating_sub(source_x))
        .min(frame_width.saturating_sub(target_x));
    let copy_height = usize::try_from(layer.geometry.height)
        .unwrap_or(0)
        .saturating_sub(source_y)
        .min(source_height.saturating_sub(source_y))
        .min(frame_height.saturating_sub(target_y));
    if copy_width == 0 || copy_height == 0 {
        return false;
    }
    let mut copied = false;
    let row_bytes = copy_width.saturating_mul(4);
    for row in 0..copy_height {
        let source_offset = source_y
            .saturating_add(row)
            .saturating_mul(source_stride)
            .saturating_add(source_x.saturating_mul(4));
        let target_offset = target_y
            .saturating_add(row)
            .saturating_mul(target_stride)
            .saturating_add(target_x.saturating_mul(4));
        let Some(source) = layer
            .buffer
            .bytes
            .get(source_offset..source_offset.saturating_add(row_bytes))
        else {
            continue;
        };
        let Some(target) = frame
            .bytes
            .get_mut(target_offset..target_offset.saturating_add(row_bytes))
        else {
            continue;
        };
        target.copy_from_slice(source);
        copied = true;
    }
    copied
}

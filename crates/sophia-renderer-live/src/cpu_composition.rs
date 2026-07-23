use sophia_protocol::{Point, Rect, Size};

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
    compose_live_cpu_frame_ref_with_cursor(output_size, layers, None)
}

/// Composes CPU-backed client layers and, when present, a compositor-owned
/// software cursor. The cursor is part of the scanout frame, so moving it
/// produces a frame even when the client itself has not committed new pixels.
pub fn compose_live_cpu_frame_ref_with_cursor(
    output_size: Size,
    layers: &[LiveCpuCompositionLayerRef<'_>],
    cursor_position: Option<Point>,
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
    if let Some(position) = cursor_position {
        compose_software_cursor(&mut frame, position);
    }
    let (nonzero_pixel_bytes, checksum) = cpu_frame_metrics(&frame.bytes);
    Ok(LiveCpuCompositionReport {
        frame,
        layers_input: layers.len(),
        layers_composed,
        nonzero_pixel_bytes,
        checksum,
    })
}

pub const CLASSIC_X11_CURSOR_EDGE: usize = 16;
pub const CLASSIC_X11_CURSOR_HOTSPOT: (i32, i32) = (0, 0);
pub const CLASSIC_X11_CURSOR_SHAPE: [&[u8]; CLASSIC_X11_CURSOR_EDGE] = [
    b"##..............",
    b"#W#.............",
    b"#WW#............",
    b"#WWW#...........",
    b"#WWWW#..........",
    b"#WWWWW#.........",
    b"#WWWWWW#........",
    b"#WWWWWWW#.......",
    b"#WWWWWWWW#......",
    b"#WWWWW#####.....",
    b"#WWW#W#.........",
    b"#WW#.#W#........",
    b"#W#..#W#........",
    b"##...#WW#.......",
    b"#....#WW#.......",
    b".....#WW#.......",
];

fn compose_software_cursor(frame: &mut LiveCpuComposedFrame, position: Point) {
    if !position.x.is_finite()
        || !position.y.is_finite()
        || position.x < f64::from(i32::MIN)
        || position.x > f64::from(i32::MAX)
        || position.y < f64::from(i32::MIN)
        || position.y > f64::from(i32::MAX)
    {
        return;
    }
    let origin_x = position.x.floor() as i32;
    let origin_y = position.y.floor() as i32;

    for (row, pixels) in CLASSIC_X11_CURSOR_SHAPE.iter().enumerate() {
        for (column, pixel) in pixels.iter().enumerate() {
            let color = match pixel {
                b'W' => [0xff, 0xff, 0xff, 0xff],
                b'#' => [0, 0, 0, 0xff],
                _ => continue,
            };
            let x = origin_x.saturating_add(i32::try_from(column).unwrap_or(i32::MAX));
            let y = origin_y.saturating_add(i32::try_from(row).unwrap_or(i32::MAX));
            put_pixel(frame, x, y, color);
        }
    }
}

fn put_pixel(frame: &mut LiveCpuComposedFrame, x: i32, y: i32, pixel: [u8; 4]) {
    let Ok(x) = usize::try_from(x) else {
        return;
    };
    let Ok(y) = usize::try_from(y) else {
        return;
    };
    let width = usize::try_from(frame.size.width).unwrap_or(0);
    let height = usize::try_from(frame.size.height).unwrap_or(0);
    let stride = usize::try_from(frame.stride).unwrap_or(0);
    if x >= width || y >= height {
        return;
    }
    let Some(offset) = y
        .checked_mul(stride)
        .and_then(|offset| offset.checked_add(x.saturating_mul(4)))
    else {
        return;
    };
    let Some(target) = frame.bytes.get_mut(offset..offset.saturating_add(4)) else {
        return;
    };
    target.copy_from_slice(&pixel);
}

fn cpu_frame_metrics(bytes: &[u8]) -> (usize, u64) {
    // The checksum is an in-process change detector, not a wire format. Hash
    // whole pixels' storage words so full-screen terminal frames do not pay a
    // serial multiply for every byte. Keep the exact nonzero-byte count for
    // the existing presentation evidence.
    let mut checksum = 0xcbf2_9ce4_8422_2325u64;
    let mut nonzero_pixel_bytes = 0usize;
    let mut words = bytes.chunks_exact(std::mem::size_of::<u64>());
    for word_bytes in words.by_ref() {
        nonzero_pixel_bytes = nonzero_pixel_bytes.saturating_add(
            word_bytes
                .iter()
                .map(|byte| usize::from(*byte != 0))
                .sum::<usize>(),
        );
        let word = u64::from_le_bytes(word_bytes.try_into().expect("exact u64 chunk"));
        checksum = (checksum ^ word).wrapping_mul(0x100_0000_01b3);
    }
    for byte in words.remainder() {
        nonzero_pixel_bytes = nonzero_pixel_bytes.saturating_add(usize::from(*byte != 0));
        checksum = (checksum ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
    }
    (nonzero_pixel_bytes, checksum)
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

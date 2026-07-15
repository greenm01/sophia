use std::collections::BTreeMap;

use sophia_protocol::{Rect, Size};

use crate::{XGraphicsContextValues, XPoint, XResourceId};

pub const X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
pub const X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAuthorityCpuBufferSnapshot {
    pub handle: u64,
    pub drawable: XResourceId,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAuthorityCpuBufferPatch {
    pub handle: u64,
    pub drawable: XResourceId,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub rect: Rect,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XAuthorityCpuBufferUpdate {
    Replace(XAuthorityCpuBufferSnapshot),
    Patch(XAuthorityCpuBufferPatch),
}

impl XAuthorityCpuBufferUpdate {
    pub const fn handle(&self) -> u64 {
        match self {
            Self::Replace(snapshot) => snapshot.handle,
            Self::Patch(patch) => patch.handle,
        }
    }

    pub const fn generation(&self) -> u64 {
        match self {
            Self::Replace(snapshot) => snapshot.generation,
            Self::Patch(patch) => patch.generation,
        }
    }

    pub fn apply_to(
        &self,
        buffers: &mut BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
    ) -> Result<(), &'static str> {
        match self {
            Self::Replace(snapshot) => {
                buffers.insert(snapshot.handle, snapshot.clone());
                Ok(())
            }
            Self::Patch(patch) => {
                let buffer = buffers
                    .get_mut(&patch.handle)
                    .ok_or("CPU buffer patch has no replacement base")?;
                if buffer.drawable != patch.drawable
                    || buffer.size != patch.size
                    || buffer.stride != patch.stride
                    || buffer.format != patch.format
                    || patch.generation < buffer.generation
                {
                    return Err("CPU buffer patch metadata does not match its base");
                }
                apply_packed_patch(buffer, patch)?;
                buffer.generation = patch.generation;
                Ok(())
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct XSoftwareBufferStore {
    next_handle: u64,
    buffers: BTreeMap<XResourceId, XAuthorityCpuBufferSnapshot>,
}

impl XSoftwareBufferStore {
    pub fn remove(&mut self, drawable: XResourceId) -> Option<XAuthorityCpuBufferSnapshot> {
        self.buffers.remove(&drawable)
    }

    pub fn paint_damage(
        &mut self,
        drawable: XResourceId,
        size: Size,
        damage: &[Rect],
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        for rect in damage {
            fill_rect(buffer, *rect, gc.foreground, gc);
        }
        finish_immutable_update(buffer, handle, replaced, union_rects(damage))
    }

    pub fn clear(
        &mut self,
        drawable: XResourceId,
        size: Size,
        rect: Rect,
        pixel: u32,
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        fill_rect(buffer, rect, pixel, &XGraphicsContextValues::default());
        finish_immutable_update(buffer, handle, replaced, Some(rect))
    }

    pub fn draw_text(
        &mut self,
        drawable: XResourceId,
        size: Size,
        x: i16,
        baseline: i16,
        text: &[u8],
        opaque: bool,
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        let top = i32::from(baseline).saturating_sub(10);
        for (index, byte) in text.iter().copied().enumerate() {
            let cell_x = i32::from(x)
                .saturating_add(i32::try_from(index.saturating_mul(8)).unwrap_or(i32::MAX));
            if opaque {
                fill_rect(
                    buffer,
                    Rect {
                        x: cell_x,
                        y: top,
                        width: 8,
                        height: 12,
                    },
                    gc.background,
                    gc,
                );
            }
            draw_fixed_glyph(buffer, cell_x, top, byte, gc.foreground, gc);
        }
        finish_immutable_update(
            buffer,
            handle,
            replaced,
            Some(Rect {
                x: i32::from(x),
                y: top,
                width: i32::try_from(text.len().saturating_mul(8)).unwrap_or(i32::MAX),
                height: 12,
            }),
        )
    }

    pub fn put_image(
        &mut self,
        drawable: XResourceId,
        size: Size,
        destination: Rect,
        data: &[u8],
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        copy_xrgb8888(buffer, destination, data);
        finish_immutable_update(buffer, handle, replaced, Some(destination))
    }

    pub fn draw_lines(
        &mut self,
        drawable: XResourceId,
        size: Size,
        points: &[XPoint],
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let damage = point_bounds(points, gc.line_width)?;
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        let width = i32::from(gc.line_width.max(1));
        for pair in points.windows(2) {
            draw_line(buffer, pair[0], pair[1], width, gc);
        }
        finish_immutable_update(buffer, handle, replaced, Some(damage))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn copy_area(
        &mut self,
        source: XResourceId,
        destination: XResourceId,
        destination_size: Size,
        source_rect: Rect,
        dst_x: i16,
        dst_y: i16,
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let source = self.buffers.get(&source)?.clone();
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(destination, destination_size, handle)?;
        let (left, top, right, bottom) = clipped_bounds(source.size, source_rect)?;
        let source_stride = usize::try_from(source.stride).ok()?;
        for source_y in top..bottom {
            for source_x in left..right {
                let offset = source_y
                    .saturating_mul(source_stride)
                    .saturating_add(source_x.saturating_mul(4));
                let pixel = u32::from_le_bytes(
                    source
                        .bytes
                        .get(offset..offset.saturating_add(4))?
                        .try_into()
                        .ok()?,
                );
                let x_offset = source_x.saturating_sub(left);
                let y_offset = source_y.saturating_sub(top);
                let target_x =
                    i32::from(dst_x).saturating_add(i32::try_from(x_offset).unwrap_or(i32::MAX));
                let target_y =
                    i32::from(dst_y).saturating_add(i32::try_from(y_offset).unwrap_or(i32::MAX));
                set_pixel(buffer, target_x, target_y, pixel, gc);
            }
        }
        finish_immutable_update(
            buffer,
            handle,
            replaced,
            Some(Rect {
                x: i32::from(dst_x),
                y: i32::from(dst_y),
                width: i32::try_from(right.saturating_sub(left)).ok()?,
                height: i32::try_from(bottom.saturating_sub(top)).ok()?,
            }),
        )
    }

    fn ensure(
        &mut self,
        drawable: XResourceId,
        size: Size,
        handle: u64,
    ) -> Option<(&mut XAuthorityCpuBufferSnapshot, bool)> {
        let width = usize::try_from(size.width).ok()?;
        let height = usize::try_from(size.height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let stride = width.checked_mul(4)?;
        let byte_len = stride.checked_mul(height)?;
        if byte_len > X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES {
            return None;
        }

        let replace = self
            .buffers
            .get(&drawable)
            .is_none_or(|buffer| buffer.size != size);
        if replace {
            let previous = self.buffers.get(&drawable);
            let generation = previous.map_or(0, |buffer| buffer.generation);
            self.buffers.insert(
                drawable,
                XAuthorityCpuBufferSnapshot {
                    handle,
                    drawable,
                    size,
                    stride: u32::try_from(stride).ok()?,
                    format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                    generation,
                    bytes: vec![0; byte_len],
                },
            );
        }
        self.buffers
            .get_mut(&drawable)
            .map(|buffer| (buffer, replace))
    }

    fn allocate_handle(&mut self) -> u64 {
        let handle = self.next_handle.max(1);
        self.next_handle = handle.saturating_add(1).max(1);
        handle
    }
}

fn finish_immutable_update(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    handle: u64,
    replaced: bool,
    damage: Option<Rect>,
) -> Option<XAuthorityCpuBufferUpdate> {
    if !replaced {
        packed_patch(buffer, damage?)?;
    }
    buffer.generation = buffer.generation.checked_add(1)?;
    buffer.handle = handle;
    Some(XAuthorityCpuBufferUpdate::Replace(buffer.clone()))
}

fn fill_rect(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    rect: Rect,
    pixel: u32,
    gc: &XGraphicsContextValues,
) {
    let Some((left, top, right, bottom)) = clipped_bounds(buffer.size, rect) else {
        return;
    };
    let stride = usize::try_from(buffer.stride).unwrap_or(0);
    for y in top..bottom {
        for x in left..right {
            if !pixel_in_clip(x, y, gc) {
                continue;
            }
            let offset = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
            if let Some(target) = buffer.bytes.get_mut(offset..offset.saturating_add(4)) {
                let destination = u32::from_le_bytes(target.try_into().unwrap_or([0; 4]));
                let output = apply_raster_function(pixel, destination, gc);
                target.copy_from_slice(&output.to_le_bytes());
            }
        }
    }
}

fn set_pixel(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    x: i32,
    y: i32,
    pixel: u32,
    gc: &XGraphicsContextValues,
) {
    if x < 0 || y < 0 || x >= buffer.size.width || y >= buffer.size.height {
        return;
    }
    let Ok(x) = usize::try_from(x) else {
        return;
    };
    let Ok(y) = usize::try_from(y) else {
        return;
    };
    if !pixel_in_clip(x, y, gc) {
        return;
    }
    let stride = usize::try_from(buffer.stride).unwrap_or(0);
    let offset = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
    if let Some(target) = buffer.bytes.get_mut(offset..offset.saturating_add(4)) {
        let destination = u32::from_le_bytes(target.try_into().unwrap_or([0; 4]));
        target.copy_from_slice(&apply_raster_function(pixel, destination, gc).to_le_bytes());
    }
}

fn draw_line(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    from: XPoint,
    to: XPoint,
    width: i32,
    gc: &XGraphicsContextValues,
) {
    let mut x = i32::from(from.x);
    let mut y = i32::from(from.y);
    let target_x = i32::from(to.x);
    let target_y = i32::from(to.y);
    let dx = (target_x - x).abs();
    let sx = if x < target_x { 1 } else { -1 };
    let dy = -(target_y - y).abs();
    let sy = if y < target_y { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        let offset = width / 2;
        fill_rect(
            buffer,
            Rect {
                x: x.saturating_sub(offset),
                y: y.saturating_sub(offset),
                width,
                height: width,
            },
            gc.foreground,
            gc,
        );
        if x == target_x && y == target_y {
            break;
        }
        let doubled = error.saturating_mul(2);
        if doubled >= dy {
            error += dy;
            x += sx;
        }
        if doubled <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn point_bounds(points: &[XPoint], line_width: u16) -> Option<Rect> {
    let first = *points.first()?;
    let mut left = i32::from(first.x);
    let mut top = i32::from(first.y);
    let mut right = left;
    let mut bottom = top;
    for point in &points[1..] {
        let x = i32::from(point.x);
        let y = i32::from(point.y);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    let width = i32::from(line_width.max(1));
    let half = width / 2;
    Some(Rect {
        x: left.saturating_sub(half),
        y: top.saturating_sub(half),
        width: right.saturating_sub(left).saturating_add(width),
        height: bottom.saturating_sub(top).saturating_add(width),
    })
}

fn copy_xrgb8888(buffer: &mut XAuthorityCpuBufferSnapshot, rect: Rect, data: &[u8]) {
    let Some((left, top, right, bottom)) = clipped_bounds(buffer.size, rect) else {
        return;
    };
    let source_width = usize::try_from(rect.width.max(0)).unwrap_or(0);
    let source_height = usize::try_from(rect.height.max(0)).unwrap_or(0);
    let Some(source_stride) = source_width.checked_mul(4) else {
        return;
    };
    if data.len() < source_stride.saturating_mul(source_height) {
        return;
    }
    let target_stride = usize::try_from(buffer.stride).unwrap_or(0);
    for y in top..bottom {
        let source_y = y.saturating_sub(usize::try_from(rect.y.max(0)).unwrap_or(0));
        let source_x = left.saturating_sub(usize::try_from(rect.x.max(0)).unwrap_or(0));
        let width = right.saturating_sub(left);
        let source_offset = source_y
            .saturating_mul(source_stride)
            .saturating_add(source_x.saturating_mul(4));
        let target_offset = y
            .saturating_mul(target_stride)
            .saturating_add(left.saturating_mul(4));
        let byte_len = width.saturating_mul(4);
        let Some(source) = data.get(source_offset..source_offset.saturating_add(byte_len)) else {
            continue;
        };
        if let Some(target) = buffer
            .bytes
            .get_mut(target_offset..target_offset.saturating_add(byte_len))
        {
            target.copy_from_slice(source);
        }
    }
}

fn draw_fixed_glyph(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    cell_x: i32,
    cell_y: i32,
    byte: u8,
    pixel: u32,
    gc: &XGraphicsContextValues,
) {
    let rows = x_fixed_glyph_rows(byte);
    for (row, bits) in rows.into_iter().enumerate() {
        for column in 0..5 {
            if bits & (1 << (4 - column)) == 0 {
                continue;
            }
            fill_rect(
                buffer,
                Rect {
                    x: cell_x.saturating_add(column + 1),
                    y: cell_y.saturating_add(i32::try_from(row).unwrap_or(0) + 2),
                    width: 1,
                    height: 1,
                },
                pixel,
                gc,
            );
        }
    }
}

pub fn x_fixed_glyph_rows(byte: u8) -> [u8; 7] {
    match byte.to_ascii_uppercase() {
        b' ' => [0; 7],
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 31],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'=' => [0, 31, 0, 31, 0, 0, 0],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'"' => [10, 10, 10, 0, 0, 0, 0],
        b'#' => [10, 31, 10, 10, 31, 10, 0],
        b'$' => [4, 15, 20, 14, 5, 30, 4],
        b'%' => [24, 25, 2, 4, 8, 19, 3],
        b'&' => [12, 18, 20, 8, 21, 18, 13],
        b'\'' => [4, 4, 8, 0, 0, 0, 0],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'*' => [0, 21, 14, 31, 14, 21, 0],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b',' => [0, 0, 0, 0, 4, 4, 8],
        b';' => [0, 4, 4, 0, 4, 4, 8],
        b'<' => [2, 4, 8, 16, 8, 4, 2],
        b'>' => [8, 4, 2, 1, 2, 4, 8],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'@' => [14, 17, 23, 21, 23, 16, 14],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b'\\' => [16, 8, 8, 4, 2, 2, 1],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        b'^' => [4, 10, 17, 0, 0, 0, 0],
        b'`' => [8, 4, 2, 0, 0, 0, 0],
        b'{' => [3, 4, 4, 8, 4, 4, 3],
        b'|' => [4, 4, 4, 4, 4, 4, 4],
        b'}' => [24, 4, 4, 2, 4, 4, 24],
        b'~' => [0, 0, 9, 22, 0, 0, 0],
        _ => [31, 17, 1, 2, 4, 0, 4],
    }
}

fn clipped_bounds(size: Size, rect: Rect) -> Option<(usize, usize, usize, usize)> {
    if size.width <= 0 || size.height <= 0 || rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    let left = rect.x.max(0).min(size.width);
    let top = rect.y.max(0).min(size.height);
    let right = rect.x.saturating_add(rect.width).max(0).min(size.width);
    let bottom = rect.y.saturating_add(rect.height).max(0).min(size.height);
    if right <= left || bottom <= top {
        return None;
    }
    Some((
        usize::try_from(left).ok()?,
        usize::try_from(top).ok()?,
        usize::try_from(right).ok()?,
        usize::try_from(bottom).ok()?,
    ))
}

fn union_rects(rectangles: &[Rect]) -> Option<Rect> {
    let first = *rectangles.first()?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x.saturating_add(first.width);
    let mut bottom = first.y.saturating_add(first.height);
    for rect in &rectangles[1..] {
        left = left.min(rect.x);
        top = top.min(rect.y);
        right = right.max(rect.x.saturating_add(rect.width));
        bottom = bottom.max(rect.y.saturating_add(rect.height));
    }
    Some(Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}

fn pixel_in_clip(x: usize, y: usize, gc: &XGraphicsContextValues) -> bool {
    if gc.clip_rectangles.is_empty() {
        return true;
    }
    let x = i32::try_from(x).unwrap_or(i32::MAX);
    let y = i32::try_from(y).unwrap_or(i32::MAX);
    gc.clip_rectangles.iter().any(|rect| {
        let left = rect.x.saturating_add(i32::from(gc.clip_x_origin));
        let top = rect.y.saturating_add(i32::from(gc.clip_y_origin));
        x >= left
            && y >= top
            && x < left.saturating_add(rect.width)
            && y < top.saturating_add(rect.height)
    })
}

fn apply_raster_function(source: u32, destination: u32, gc: &XGraphicsContextValues) -> u32 {
    let source = source & 0x00ff_ffff;
    let destination = destination & 0x00ff_ffff;
    let result = match gc.function {
        0 => 0,
        1 => source & destination,
        2 => source & !destination,
        3 => source,
        4 => !source & destination,
        5 => destination,
        6 => source ^ destination,
        7 => source | destination,
        8 => !(source | destination),
        9 => !(source ^ destination),
        10 => !destination,
        11 => source | !destination,
        12 => !source,
        13 => !source | destination,
        14 => !(source & destination),
        15 => u32::MAX,
        _ => source,
    } & 0x00ff_ffff;
    let mask = gc.plane_mask & 0x00ff_ffff;
    ((result & mask) | (destination & !mask)) & 0x00ff_ffff
}

fn packed_patch(
    buffer: &XAuthorityCpuBufferSnapshot,
    rect: Rect,
) -> Option<XAuthorityCpuBufferPatch> {
    let (left, top, right, bottom) = clipped_bounds(buffer.size, rect)?;
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    let row_bytes = width.checked_mul(4)?;
    let source_stride = usize::try_from(buffer.stride).ok()?;
    let mut bytes = Vec::with_capacity(row_bytes.checked_mul(height)?);
    for y in top..bottom {
        let offset = y
            .checked_mul(source_stride)?
            .checked_add(left.checked_mul(4)?)?;
        bytes.extend_from_slice(buffer.bytes.get(offset..offset.checked_add(row_bytes)?)?);
    }
    Some(XAuthorityCpuBufferPatch {
        handle: buffer.handle,
        drawable: buffer.drawable,
        size: buffer.size,
        stride: buffer.stride,
        format: buffer.format,
        generation: buffer.generation,
        rect: Rect {
            x: i32::try_from(left).ok()?,
            y: i32::try_from(top).ok()?,
            width: i32::try_from(width).ok()?,
            height: i32::try_from(height).ok()?,
        },
        bytes,
    })
}

fn apply_packed_patch(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    patch: &XAuthorityCpuBufferPatch,
) -> Result<(), &'static str> {
    let (left, top, right, bottom) =
        clipped_bounds(buffer.size, patch.rect).ok_or("CPU buffer patch is empty")?;
    if patch.rect.x != i32::try_from(left).unwrap_or(i32::MAX)
        || patch.rect.y != i32::try_from(top).unwrap_or(i32::MAX)
        || patch.rect.width != i32::try_from(right.saturating_sub(left)).unwrap_or(i32::MAX)
        || patch.rect.height != i32::try_from(bottom.saturating_sub(top)).unwrap_or(i32::MAX)
    {
        return Err("CPU buffer patch lies outside its buffer");
    }
    let row_bytes = right.saturating_sub(left).saturating_mul(4);
    let expected = row_bytes.saturating_mul(bottom.saturating_sub(top));
    if patch.bytes.len() != expected {
        return Err("CPU buffer patch byte length is invalid");
    }
    let target_stride = usize::try_from(buffer.stride).map_err(|_| "invalid target stride")?;
    for (row, y) in (top..bottom).enumerate() {
        let source_offset = row.saturating_mul(row_bytes);
        let target_offset = y
            .saturating_mul(target_stride)
            .saturating_add(left.saturating_mul(4));
        let source = patch
            .bytes
            .get(source_offset..source_offset.saturating_add(row_bytes))
            .ok_or("CPU buffer patch source row is invalid")?;
        let target = buffer
            .bytes
            .get_mut(target_offset..target_offset.saturating_add(row_bytes))
            .ok_or("CPU buffer patch target row is invalid")?;
        target.copy_from_slice(source);
    }
    Ok(())
}

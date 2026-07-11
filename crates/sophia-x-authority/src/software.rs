use std::collections::BTreeMap;

use sophia_protocol::{Rect, Size};

use crate::XResourceId;

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

#[derive(Debug, Default)]
pub(crate) struct XSoftwareBufferStore {
    next_handle: u64,
    buffers: BTreeMap<XResourceId, XAuthorityCpuBufferSnapshot>,
}

impl XSoftwareBufferStore {
    pub fn paint_damage(
        &mut self,
        drawable: XResourceId,
        size: Size,
        damage: &[Rect],
    ) -> Option<XAuthorityCpuBufferSnapshot> {
        let buffer = self.ensure(drawable, size)?;
        for rect in damage {
            fill_rect(buffer, *rect, 0x00ff_ffff);
        }
        bump_and_clone(buffer)
    }

    pub fn clear(
        &mut self,
        drawable: XResourceId,
        size: Size,
        rect: Rect,
    ) -> Option<XAuthorityCpuBufferSnapshot> {
        let buffer = self.ensure(drawable, size)?;
        fill_rect(buffer, rect, 0);
        bump_and_clone(buffer)
    }

    pub fn draw_text(
        &mut self,
        drawable: XResourceId,
        size: Size,
        x: i16,
        baseline: i16,
        text: &[u8],
        opaque: bool,
    ) -> Option<XAuthorityCpuBufferSnapshot> {
        let buffer = self.ensure(drawable, size)?;
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
                    0,
                );
            }
            draw_fixed_glyph(buffer, cell_x, top, byte, 0x00ff_ffff);
        }
        bump_and_clone(buffer)
    }

    pub fn put_image(
        &mut self,
        drawable: XResourceId,
        size: Size,
        destination: Rect,
        data: &[u8],
    ) -> Option<XAuthorityCpuBufferSnapshot> {
        let buffer = self.ensure(drawable, size)?;
        copy_xrgb8888(buffer, destination, data);
        bump_and_clone(buffer)
    }

    fn ensure(
        &mut self,
        drawable: XResourceId,
        size: Size,
    ) -> Option<&mut XAuthorityCpuBufferSnapshot> {
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
            let handle = self
                .buffers
                .get(&drawable)
                .map(|buffer| buffer.handle)
                .unwrap_or_else(|| {
                    let handle = self.next_handle.max(1);
                    self.next_handle = handle.saturating_add(1).max(1);
                    handle
                });
            self.buffers.insert(
                drawable,
                XAuthorityCpuBufferSnapshot {
                    handle,
                    drawable,
                    size,
                    stride: u32::try_from(stride).ok()?,
                    format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                    generation: 0,
                    bytes: vec![0; byte_len],
                },
            );
        }
        self.buffers.get_mut(&drawable)
    }
}

fn bump_and_clone(buffer: &mut XAuthorityCpuBufferSnapshot) -> Option<XAuthorityCpuBufferSnapshot> {
    buffer.generation = buffer.generation.checked_add(1)?;
    Some(buffer.clone())
}

fn fill_rect(buffer: &mut XAuthorityCpuBufferSnapshot, rect: Rect, pixel: u32) {
    let Some((left, top, right, bottom)) = clipped_bounds(buffer.size, rect) else {
        return;
    };
    let stride = usize::try_from(buffer.stride).unwrap_or(0);
    let pixel = pixel.to_le_bytes();
    for y in top..bottom {
        for x in left..right {
            let offset = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
            if let Some(target) = buffer.bytes.get_mut(offset..offset.saturating_add(4)) {
                target.copy_from_slice(&pixel);
            }
        }
    }
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
) {
    let rows = fixed_glyph_rows(byte);
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
            );
        }
    }
}

fn fixed_glyph_rows(byte: u8) -> [u8; 7] {
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

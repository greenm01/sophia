use crate::{Point, Size};

/// Maximum cursor image edge accepted by the Engine.
pub const CURSOR_IMAGE_MAX_EDGE: i32 = 256;

/// Protocol-neutral compositor cursor state.
///
/// Frontends may select the image, but presentation remains an Engine concern.
/// Pixels are unpremultiplied ARGB8888 in row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct CursorSnapshot {
    pub visible: bool,
    pub position: Point,
    pub hotspot: Point,
    pub image_size: Size,
    pub argb8888: Vec<u8>,
    pub generation: u64,
}

impl CursorSnapshot {
    pub fn image_is_valid(&self) -> bool {
        let Size { width, height } = self.image_size;
        width > 0
            && height > 0
            && width <= CURSOR_IMAGE_MAX_EDGE
            && height <= CURSOR_IMAGE_MAX_EDGE
            && usize::try_from(width)
                .ok()
                .and_then(|width| {
                    usize::try_from(height)
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .and_then(|pixels| pixels.checked_mul(4))
                == Some(self.argb8888.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_or_truncated_images() {
        let mut snapshot = CursorSnapshot {
            visible: true,
            position: Point::default(),
            hotspot: Point::default(),
            image_size: Size {
                width: 16,
                height: 16,
            },
            argb8888: vec![0; 16 * 16 * 4],
            generation: 1,
        };
        assert!(snapshot.image_is_valid());
        snapshot.argb8888.pop();
        assert!(!snapshot.image_is_valid());
        snapshot.image_size.width = CURSOR_IMAGE_MAX_EDGE + 1;
        assert!(!snapshot.image_is_valid());
    }
}

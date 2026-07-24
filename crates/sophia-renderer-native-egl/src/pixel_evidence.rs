#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeCompositionPixelMetrics {
    pub pixels: usize,
    pub nonzero_rgb_pixels: usize,
    pub alpha_zero_pixels: usize,
    pub alpha_partial_pixels: usize,
    pub alpha_opaque_pixels: usize,
    pub checksum: u64,
}

pub fn native_composition_pixel_metrics(rgba: &[u8]) -> NativeCompositionPixelMetrics {
    let mut metrics = NativeCompositionPixelMetrics {
        pixels: rgba.len() / 4,
        checksum: 0xcbf2_9ce4_8422_2325,
        ..NativeCompositionPixelMetrics::default()
    };
    for pixel in rgba.chunks_exact(4) {
        if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
            metrics.nonzero_rgb_pixels = metrics.nonzero_rgb_pixels.saturating_add(1);
        }
        match pixel[3] {
            0 => metrics.alpha_zero_pixels = metrics.alpha_zero_pixels.saturating_add(1),
            255 => metrics.alpha_opaque_pixels = metrics.alpha_opaque_pixels.saturating_add(1),
            _ => metrics.alpha_partial_pixels = metrics.alpha_partial_pixels.saturating_add(1),
        }
        for byte in pixel {
            metrics.checksum ^= u64::from(*byte);
            metrics.checksum = metrics.checksum.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    metrics
}

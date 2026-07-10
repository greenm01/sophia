use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmRendererScanoutBuffer {
    size: Size,
    pitch: u32,
    format: u32,
    handle: drm::buffer::Handle,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmRendererScanoutBuffer {
    pub fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Option<Self> {
        if !descriptor.is_valid_scanout_buffer() {
            return None;
        }

        Some(Self {
            size: descriptor.size,
            pitch: descriptor.pitch,
            format: descriptor.format,
            handle: drm::control::from_u32(descriptor.gem_handle)?,
        })
    }
}

#[cfg(feature = "libdrm-events")]
impl drm::buffer::Buffer for LibdrmRendererScanoutBuffer {
    fn size(&self) -> (u32, u32) {
        (self.size.width as u32, self.size.height as u32)
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        drm::buffer::DrmFourcc::Xrgb8888
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn handle(&self) -> drm::buffer::Handle {
        self.handle
    }
}

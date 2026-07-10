use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmRendererScanoutBuffer {
    size: Size,
    pitch: u32,
    format: u32,
    handle: drm::buffer::Handle,
    plane_handles: [Option<drm::buffer::Handle>; 4],
    plane_pitches: [u32; 4],
    plane_offsets: [u32; 4],
    modifier: Option<drm::buffer::DrmModifier>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmRendererScanoutBuffer {
    pub fn from_descriptor(descriptor: LiveRendererScanoutBufferDescriptor) -> Option<Self> {
        if !descriptor.is_valid_scanout_buffer() {
            return None;
        }

        let plane_handles = scanout_plane_handles_from_descriptor(descriptor)?;

        Some(Self {
            size: descriptor.size,
            pitch: descriptor.pitch,
            format: descriptor.format,
            handle: drm::control::from_u32(descriptor.gem_handle)?,
            plane_handles,
            plane_pitches: descriptor.plane_pitches,
            plane_offsets: descriptor.plane_offsets,
            modifier: descriptor.modifier.map(drm::buffer::DrmModifier::from),
        })
    }
}

#[cfg(feature = "libdrm-events")]
fn scanout_plane_handles_from_descriptor(
    descriptor: LiveRendererScanoutBufferDescriptor,
) -> Option<[Option<drm::buffer::Handle>; 4]> {
    let mut handles = [None, None, None, None];
    let mut index = 0;
    while index < handles.len() {
        if index < descriptor.plane_count as usize {
            handles[index] = Some(drm::control::from_u32(descriptor.plane_handles[index])?);
        }
        index += 1;
    }

    Some(handles)
}

#[cfg(feature = "libdrm-events")]
impl drm::buffer::Buffer for LibdrmRendererScanoutBuffer {
    fn size(&self) -> (u32, u32) {
        (self.size.width as u32, self.size.height as u32)
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        match self.format {
            LIVE_RENDERER_SCANOUT_FORMAT_ARGB8888 => drm::buffer::DrmFourcc::Argb8888,
            _ => drm::buffer::DrmFourcc::Xrgb8888,
        }
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn handle(&self) -> drm::buffer::Handle {
        self.handle
    }
}

#[cfg(feature = "libdrm-events")]
impl drm::buffer::PlanarBuffer for LibdrmRendererScanoutBuffer {
    fn size(&self) -> (u32, u32) {
        drm::buffer::Buffer::size(self)
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        drm::buffer::Buffer::format(self)
    }

    fn modifier(&self) -> Option<drm::buffer::DrmModifier> {
        self.modifier.filter(|modifier| {
            !matches!(
                modifier,
                drm::buffer::DrmModifier::Invalid | drm::buffer::DrmModifier::Linear
            )
        })
    }

    fn pitches(&self) -> [u32; 4] {
        self.plane_pitches
    }

    fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
        self.plane_handles
    }

    fn offsets(&self) -> [u32; 4] {
        self.plane_offsets
    }
}

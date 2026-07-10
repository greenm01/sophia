use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64>;

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized;

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()>;

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativePrimaryPlaneResourceDevice for D
where
    D: drm::control::Device,
{
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        let Some(mode) = selection.mode else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "selected KMS target does not carry a native mode",
            ));
        };
        match self.create_property_blob(&mode)? {
            drm::control::property::Value::Blob(blob) => Ok(blob),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "DRM mode blob creation returned a non-blob value",
            )),
        }
    }

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        self.add_planar_framebuffer(buffer, scanout_framebuffer_flags(buffer))
    }

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.destroy_framebuffer(framebuffer)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.destroy_property_blob(mode_blob)
    }
}

#[cfg(feature = "libdrm-events")]
fn scanout_framebuffer_flags<B>(buffer: &B) -> drm::control::FbCmd2Flags
where
    B: drm::buffer::PlanarBuffer + ?Sized,
{
    if buffer.modifier().is_some_and(|modifier| {
        !matches!(
            modifier,
            drm::buffer::DrmModifier::Invalid | drm::buffer::DrmModifier::Linear
        )
    }) {
        drm::control::FbCmd2Flags::MODIFIERS
    } else {
        drm::control::FbCmd2Flags::empty()
    }
}

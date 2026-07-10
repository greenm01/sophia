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
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized;

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
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        self.add_framebuffer(buffer, depth, bpp)
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

use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::os::fd::BorrowedFd;

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64>;

    fn add_scanout_framebuffer_with_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized;

    fn add_scanout_framebuffer_without_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized;

    fn add_legacy_scanout_framebuffer<B>(
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

    fn import_scanout_dma_buf(&self, _fd: BorrowedFd<'_>) -> io::Result<drm::buffer::Handle> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "scanout DMA-BUF import is unavailable",
        ))
    }

    fn close_scanout_buffer(&self, _handle: drm::buffer::Handle) -> io::Result<()> {
        Ok(())
    }

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

    fn add_scanout_framebuffer_with_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        self.add_planar_framebuffer(buffer, drm::control::FbCmd2Flags::MODIFIERS)
    }

    fn add_scanout_framebuffer_without_modifiers<B>(
        &self,
        buffer: &B,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::PlanarBuffer + ?Sized,
    {
        self.add_planar_framebuffer(buffer, drm::control::FbCmd2Flags::empty())
    }

    fn add_legacy_scanout_framebuffer<B>(
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

    fn import_scanout_dma_buf(&self, fd: BorrowedFd<'_>) -> io::Result<drm::buffer::Handle> {
        self.prime_fd_to_buffer(fd)
    }

    fn close_scanout_buffer(&self, handle: drm::buffer::Handle) -> io::Result<()> {
        self.close_buffer(handle)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.destroy_property_blob(mode_blob)
    }
}

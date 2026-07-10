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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceBundle {
    framebuffer: drm::control::framebuffer::Handle,
    mode_blob: Option<u64>,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceBundle {
    pub const fn into_objects(
        self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> LibdrmNativePrimaryPlaneObjects {
        selection.into_objects(self.framebuffer, self.mode_blob)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneResourceCreateResult {
    pub status: LibdrmNativePrimaryPlaneResourceCreateStatus,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceBundle>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceCreateStatus {
    Created,
    InvalidSelectionSize,
    BufferSizeMismatch,
    MissingMode,
    ModeBlobCreateFailed,
    FramebufferCreateFailed,
}

#[cfg(feature = "libdrm-events")]
pub fn create_native_primary_plane_resources<D, B>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    buffer: &B,
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + ?Sized,
{
    if selection.size.width <= 0 || selection.size.height <= 0 {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            resources: None,
            cleanup: None,
        };
    }

    let (buffer_width, buffer_height) = buffer.size();
    if buffer_width != selection.size.width as u32 || buffer_height != selection.size.height as u32
    {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch,
            resources: None,
            cleanup: None,
        };
    }

    let mode_blob = match device.create_mode_blob_for_selection(selection) {
        Ok(mode_blob) => mode_blob,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode,
                resources: None,
                cleanup: None,
            };
        }
        Err(_) => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed,
                resources: None,
                cleanup: None,
            };
        }
    };
    let Ok(framebuffer) = device.add_scanout_framebuffer(buffer, 24, 32) else {
        let cleanup = device.destroy_mode_blob(mode_blob).is_err().then(|| {
            LibdrmNativePrimaryPlaneResourceCleanup::from_mode_blob(mode_blob, selection.size)
        });
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed,
            resources: None,
            cleanup,
        };
    };

    LibdrmNativePrimaryPlaneResourceCreateResult {
        status: LibdrmNativePrimaryPlaneResourceCreateStatus::Created,
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle {
            framebuffer,
            mode_blob: Some(mode_blob),
            size: selection.size,
        }),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
pub fn create_native_primary_plane_page_flip_resources<D, B>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    buffer: &B,
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + ?Sized,
{
    if selection.size.width <= 0 || selection.size.height <= 0 {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            resources: None,
            cleanup: None,
        };
    }

    let (buffer_width, buffer_height) = buffer.size();
    if buffer_width != selection.size.width as u32 || buffer_height != selection.size.height as u32
    {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch,
            resources: None,
            cleanup: None,
        };
    }

    let Ok(framebuffer) = device.add_scanout_framebuffer(buffer, 24, 32) else {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed,
            resources: None,
            cleanup: None,
        };
    };

    LibdrmNativePrimaryPlaneResourceCreateResult {
        status: LibdrmNativePrimaryPlaneResourceCreateStatus::Created,
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle {
            framebuffer,
            mode_blob: None,
            size: selection.size,
        }),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceDestroyReport {
    pub status: LibdrmNativePrimaryPlaneResourceDestroyStatus,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceDestroyStatus {
    Destroyed,
    FramebufferDestroyFailed,
    ModeBlobDestroyFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceCleanup {
    framebuffer: Option<drm::control::framebuffer::Handle>,
    mode_blob: Option<u64>,
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceCleanup {
    pub const fn from_bundle(resources: LibdrmNativePrimaryPlaneResourceBundle) -> Self {
        Self {
            framebuffer: Some(resources.framebuffer),
            mode_blob: resources.mode_blob,
            size: resources.size,
        }
    }

    pub const fn from_mode_blob(mode_blob: u64, size: Size) -> Self {
        Self {
            framebuffer: None,
            mode_blob: Some(mode_blob),
            size,
        }
    }

    pub fn retry<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resource_cleanup(device, self)
    }
}

#[cfg(feature = "libdrm-events")]
pub fn destroy_native_primary_plane_resources<D>(
    device: &D,
    resources: LibdrmNativePrimaryPlaneResourceBundle,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    destroy_native_primary_plane_resource_cleanup(
        device,
        LibdrmNativePrimaryPlaneResourceCleanup::from_bundle(resources),
    )
}

#[cfg(feature = "libdrm-events")]
pub fn destroy_native_primary_plane_resource_cleanup<D>(
    device: &D,
    mut cleanup: LibdrmNativePrimaryPlaneResourceCleanup,
) -> LibdrmNativePrimaryPlaneResourceDestroyReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    if let Some(framebuffer) = cleanup.framebuffer {
        if device.destroy_scanout_framebuffer(framebuffer).is_err() {
            return LibdrmNativePrimaryPlaneResourceDestroyReport {
                status: LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed,
                cleanup: Some(cleanup),
            };
        }
        cleanup.framebuffer = None;
    }

    if let Some(mode_blob) = cleanup.mode_blob {
        if device.destroy_mode_blob(mode_blob).is_err() {
            return LibdrmNativePrimaryPlaneResourceDestroyReport {
                status: LibdrmNativePrimaryPlaneResourceDestroyStatus::ModeBlobDestroyFailed,
                cleanup: Some(cleanup),
            };
        }
        cleanup.mode_blob = None;
    }

    if cleanup.framebuffer.is_some() || cleanup.mode_blob.is_some() {
        return LibdrmNativePrimaryPlaneResourceDestroyReport {
            status: LibdrmNativePrimaryPlaneResourceDestroyStatus::ModeBlobDestroyFailed,
            cleanup: Some(cleanup),
        };
    }

    LibdrmNativePrimaryPlaneResourceDestroyReport {
        status: LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed,
        cleanup: None,
    }
}

use crate::prelude::*;

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
    ImportedBufferCloseFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceCleanup {
    framebuffer: Option<drm::control::framebuffer::Handle>,
    mode_blob: Option<u64>,
    imported_buffers: [Option<drm::buffer::Handle>; 4],
    size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceCleanup {
    pub const fn from_bundle(resources: LibdrmNativePrimaryPlaneResourceBundle) -> Self {
        Self {
            framebuffer: Some(resources.framebuffer),
            mode_blob: resources.mode_blob,
            imported_buffers: resources.imported_buffers,
            size: resources.size,
        }
    }

    pub const fn from_mode_blob(mode_blob: u64, size: Size) -> Self {
        Self {
            framebuffer: None,
            mode_blob: Some(mode_blob),
            imported_buffers: [None, None, None, None],
            size,
        }
    }

    pub const fn from_imported_buffers(
        imported_buffers: [Option<drm::buffer::Handle>; 4],
        size: Size,
    ) -> Self {
        Self {
            framebuffer: None,
            mode_blob: None,
            imported_buffers,
            size,
        }
    }

    pub const fn from_mode_blob_and_imported_buffers(
        mode_blob: u64,
        imported_buffers: [Option<drm::buffer::Handle>; 4],
        size: Size,
    ) -> Self {
        Self {
            framebuffer: None,
            mode_blob: Some(mode_blob),
            imported_buffers,
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

    let mut index = 0;
    while index < cleanup.imported_buffers.len() {
        if let Some(handle) = cleanup.imported_buffers[index] {
            match device.close_scanout_buffer(handle) {
                Ok(()) => {}
                // PRIME imports may resolve to a GEM handle the driver already
                // released while removing the framebuffer. GEM_CLOSE reports
                // EINVAL for that idempotent-cleanup case.
                Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
                Err(_) => {
                    return LibdrmNativePrimaryPlaneResourceDestroyReport {
                        status:
                            LibdrmNativePrimaryPlaneResourceDestroyStatus::ImportedBufferCloseFailed,
                        cleanup: Some(cleanup),
                    };
                }
            }
            cleanup.imported_buffers[index] = None;
        }
        index += 1;
    }

    if cleanup.framebuffer.is_some()
        || cleanup.mode_blob.is_some()
        || cleanup.imported_buffers.iter().any(Option::is_some)
    {
        return LibdrmNativePrimaryPlaneResourceDestroyReport {
            status: LibdrmNativePrimaryPlaneResourceDestroyStatus::ImportedBufferCloseFailed,
            cleanup: Some(cleanup),
        };
    }

    LibdrmNativePrimaryPlaneResourceDestroyReport {
        status: LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed,
        cleanup: None,
    }
}

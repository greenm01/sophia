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

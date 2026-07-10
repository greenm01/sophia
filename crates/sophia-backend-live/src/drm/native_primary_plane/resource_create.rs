use crate::prelude::*;

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
    if !is_valid_native_primary_plane_scanout_size(selection.size) {
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
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle::new(
            framebuffer,
            Some(mode_blob),
            selection.size,
        )),
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
    if !is_valid_native_primary_plane_scanout_size(selection.size) {
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
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle::new(
            framebuffer,
            None,
            selection.size,
        )),
        cleanup: None,
    }
}

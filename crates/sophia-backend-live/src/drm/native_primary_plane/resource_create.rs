use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneResourceCreateResult {
    pub status: LibdrmNativePrimaryPlaneResourceCreateStatus,
    pub framebuffer: Option<LibdrmNativePrimaryPlaneFramebufferCreateDetail>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceBundle>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneResourceCreateStatus {
    Created,
    InvalidSelectionSize,
    BufferSizeMismatch,
    InvalidBuffer,
    MissingMode,
    ModeBlobCreateFailed,
    FramebufferCreateFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneFramebufferCreateDetail {
    CreatedWithAddFb2,
    CreatedWithAddFb2Modifiers,
    CreatedWithLegacyAddFb,
    AddFb2Failed,
    AddFb2ModifiersFailed,
    AddFb2ModifiersThenAddFb2ThenLegacyAddFbFailed,
    AddFb2ThenLegacyAddFbFailed,
    NotAttempted,
}

#[cfg(feature = "libdrm-events")]
pub fn create_native_primary_plane_resources<D, B>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    buffer: &B,
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + drm::buffer::PlanarBuffer + ?Sized,
{
    if !is_valid_native_primary_plane_scanout_size(selection.size) {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }

    if let Some(status) = invalid_native_primary_plane_scanout_buffer_status(selection, buffer) {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }

    let mode_blob = match device.create_mode_blob_for_selection(selection) {
        Ok(mode_blob) => mode_blob,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode,
                framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
                resources: None,
                cleanup: None,
            };
        }
        Err(_) => {
            return LibdrmNativePrimaryPlaneResourceCreateResult {
                status: LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed,
                framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
                resources: None,
                cleanup: None,
            };
        }
    };
    if mode_blob == 0 {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }
    let framebuffer_create = create_scanout_framebuffer(device, buffer);
    let Some(framebuffer) = framebuffer_create.framebuffer else {
        let cleanup = device.destroy_mode_blob(mode_blob).is_err().then(|| {
            LibdrmNativePrimaryPlaneResourceCleanup::from_mode_blob(mode_blob, selection.size)
        });
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed,
            framebuffer: Some(framebuffer_create.detail),
            resources: None,
            cleanup,
        };
    };

    LibdrmNativePrimaryPlaneResourceCreateResult {
        status: LibdrmNativePrimaryPlaneResourceCreateStatus::Created,
        framebuffer: Some(framebuffer_create.detail),
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
    B: drm::buffer::Buffer + drm::buffer::PlanarBuffer + ?Sized,
{
    if !is_valid_native_primary_plane_scanout_size(selection.size) {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }

    if let Some(status) = invalid_native_primary_plane_scanout_buffer_status(selection, buffer) {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }

    let framebuffer_create = create_scanout_framebuffer(device, buffer);
    let Some(framebuffer) = framebuffer_create.framebuffer else {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed,
            framebuffer: Some(framebuffer_create.detail),
            resources: None,
            cleanup: None,
        };
    };

    LibdrmNativePrimaryPlaneResourceCreateResult {
        status: LibdrmNativePrimaryPlaneResourceCreateStatus::Created,
        framebuffer: Some(framebuffer_create.detail),
        resources: Some(LibdrmNativePrimaryPlaneResourceBundle::new(
            framebuffer,
            None,
            selection.size,
        )),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
struct LibdrmNativePrimaryPlaneFramebufferCreateResult {
    detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail,
    framebuffer: Option<drm::control::framebuffer::Handle>,
}

#[cfg(feature = "libdrm-events")]
fn create_scanout_framebuffer<D, B>(
    device: &D,
    buffer: &B,
) -> LibdrmNativePrimaryPlaneFramebufferCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + drm::buffer::PlanarBuffer + ?Sized,
{
    if has_explicit_modifier(buffer) {
        match device.add_scanout_framebuffer_with_modifiers(buffer) {
            Ok(framebuffer) => LibdrmNativePrimaryPlaneFramebufferCreateResult {
                detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2Modifiers,
                framebuffer: Some(framebuffer),
            },
            Err(_) if has_non_linear_modifier(buffer) => LibdrmNativePrimaryPlaneFramebufferCreateResult {
                detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail::AddFb2ModifiersFailed,
                framebuffer: None,
            },
            Err(_) => create_implicit_or_legacy_scanout_framebuffer(
                device,
                buffer,
                LibdrmNativePrimaryPlaneFramebufferCreateDetail::AddFb2ModifiersThenAddFb2ThenLegacyAddFbFailed,
            ),
        }
    } else {
        create_implicit_or_legacy_scanout_framebuffer(
            device,
            buffer,
            LibdrmNativePrimaryPlaneFramebufferCreateDetail::AddFb2ThenLegacyAddFbFailed,
        )
    }
}

#[cfg(feature = "libdrm-events")]
fn create_implicit_or_legacy_scanout_framebuffer<D, B>(
    device: &D,
    buffer: &B,
    failed_detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail,
) -> LibdrmNativePrimaryPlaneFramebufferCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
    B: drm::buffer::Buffer + drm::buffer::PlanarBuffer + ?Sized,
{
    if let Ok(framebuffer) = device.add_scanout_framebuffer_without_modifiers(buffer) {
        return LibdrmNativePrimaryPlaneFramebufferCreateResult {
            detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithAddFb2,
            framebuffer: Some(framebuffer),
        };
    }

    match device.add_legacy_scanout_framebuffer(buffer, 24, 32) {
        Ok(framebuffer) => LibdrmNativePrimaryPlaneFramebufferCreateResult {
            detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail::CreatedWithLegacyAddFb,
            framebuffer: Some(framebuffer),
        },
        Err(_) => LibdrmNativePrimaryPlaneFramebufferCreateResult {
            detail: failed_detail,
            framebuffer: None,
        },
    }
}

#[cfg(feature = "libdrm-events")]
fn has_explicit_modifier<B>(buffer: &B) -> bool
where
    B: drm::buffer::PlanarBuffer + ?Sized,
{
    buffer.modifier().is_some()
}

#[cfg(feature = "libdrm-events")]
fn has_non_linear_modifier<B>(buffer: &B) -> bool
where
    B: drm::buffer::PlanarBuffer + ?Sized,
{
    buffer.modifier().is_some_and(|modifier| {
        !matches!(
            modifier,
            drm::buffer::DrmModifier::Invalid | drm::buffer::DrmModifier::Linear
        )
    })
}

#[cfg(feature = "libdrm-events")]
fn invalid_native_primary_plane_scanout_buffer_status<B>(
    selection: LibdrmNativePrimaryPlaneSelection,
    buffer: &B,
) -> Option<LibdrmNativePrimaryPlaneResourceCreateStatus>
where
    B: drm::buffer::Buffer + ?Sized,
{
    let (buffer_width, buffer_height) = buffer.size();
    if buffer_width != selection.size.width as u32 || buffer_height != selection.size.height as u32
    {
        return Some(LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch);
    }

    if !is_supported_native_scanout_format(buffer.format())
        || buffer.pitch() < buffer_width * LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL
    {
        return Some(LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer);
    }

    None
}

#[cfg(feature = "libdrm-events")]
const LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL: u32 = 4;

#[cfg(feature = "libdrm-events")]
const fn is_supported_native_scanout_format(format: drm::buffer::DrmFourcc) -> bool {
    matches!(
        format,
        drm::buffer::DrmFourcc::Xrgb8888 | drm::buffer::DrmFourcc::Argb8888
    )
}

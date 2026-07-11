use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::os::fd::{AsFd, OwnedFd};

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
    BufferImportFailed,
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
        let cleanup = cleanup_after_framebuffer_create_failure(
            device,
            Some(mode_blob),
            [None; 4],
            selection.size,
        );
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
pub fn create_native_primary_plane_resources_from_dma_bufs<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: [Option<OwnedFd>; 4],
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    create_native_primary_plane_resources_from_dma_bufs_with_policy(
        device, selection, descriptor, plane_fds, true,
    )
}

#[cfg(feature = "libdrm-events")]
pub fn create_native_primary_plane_page_flip_resources_from_dma_bufs<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: [Option<OwnedFd>; 4],
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    create_native_primary_plane_resources_from_dma_bufs_with_policy(
        device, selection, descriptor, plane_fds, false,
    )
}

#[cfg(feature = "libdrm-events")]
fn create_native_primary_plane_resources_from_dma_bufs_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelection,
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: [Option<OwnedFd>; 4],
    create_mode_blob: bool,
) -> LibdrmNativePrimaryPlaneResourceCreateResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    if !is_valid_native_primary_plane_scanout_size(selection.size) {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }

    let Some(source_buffer) = LibdrmRendererScanoutBuffer::from_descriptor(descriptor) else {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    };

    if let Some(status) =
        invalid_native_primary_plane_scanout_buffer_status(selection, &source_buffer)
    {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: None,
        };
    }

    let imported = import_scanout_dma_bufs(device, descriptor, plane_fds);
    let Some(imported) = imported.imported else {
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::BufferImportFailed,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup: imported.cleanup,
        };
    };

    let Some(buffer) = LibdrmRendererScanoutBuffer::from_descriptor_and_imported_plane_handles(
        descriptor,
        imported.plane_handles,
    ) else {
        let cleanup = cleanup_imported_buffers_after_failure(
            device,
            imported.cleanup_handles,
            selection.size,
        );
        return LibdrmNativePrimaryPlaneResourceCreateResult {
            status: LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer,
            framebuffer: Some(LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted),
            resources: None,
            cleanup,
        };
    };

    let mode_blob = if create_mode_blob {
        match device.create_mode_blob_for_selection(selection) {
            Ok(mode_blob) if mode_blob != 0 => Some(mode_blob),
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                let cleanup = cleanup_imported_buffers_after_failure(
                    device,
                    imported.cleanup_handles,
                    selection.size,
                );
                return LibdrmNativePrimaryPlaneResourceCreateResult {
                    status: LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode,
                    framebuffer: Some(
                        LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted,
                    ),
                    resources: None,
                    cleanup,
                };
            }
            Err(_) | Ok(_) => {
                let cleanup = cleanup_imported_buffers_after_failure(
                    device,
                    imported.cleanup_handles,
                    selection.size,
                );
                return LibdrmNativePrimaryPlaneResourceCreateResult {
                    status: LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed,
                    framebuffer: Some(
                        LibdrmNativePrimaryPlaneFramebufferCreateDetail::NotAttempted,
                    ),
                    resources: None,
                    cleanup,
                };
            }
        }
    } else {
        None
    };

    let framebuffer_create = create_scanout_framebuffer(device, &buffer);
    let Some(framebuffer) = framebuffer_create.framebuffer else {
        let cleanup = cleanup_after_framebuffer_create_failure(
            device,
            mode_blob,
            imported.cleanup_handles,
            selection.size,
        );
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
        resources: Some(
            LibdrmNativePrimaryPlaneResourceBundle::new_with_imported_buffers(
                framebuffer,
                mode_blob,
                imported.cleanup_handles,
                selection.size,
            ),
        ),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
struct LibdrmNativePrimaryPlaneImportedBuffer {
    plane_handles: [Option<drm::buffer::Handle>; 4],
    cleanup_handles: [Option<drm::buffer::Handle>; 4],
}

#[cfg(feature = "libdrm-events")]
struct LibdrmNativePrimaryPlaneImportResult {
    imported: Option<LibdrmNativePrimaryPlaneImportedBuffer>,
    cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[cfg(feature = "libdrm-events")]
fn import_scanout_dma_bufs<D>(
    device: &D,
    descriptor: LiveRendererScanoutBufferDescriptor,
    plane_fds: [Option<OwnedFd>; 4],
) -> LibdrmNativePrimaryPlaneImportResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    let mut plane_handles = [None; 4];
    let mut cleanup_handles = [None; 4];
    let mut index = 0;
    while index < descriptor.plane_count as usize {
        let Some(fd) = plane_fds[index].as_ref() else {
            let cleanup =
                cleanup_imported_buffers_after_failure(device, cleanup_handles, descriptor.size);
            return LibdrmNativePrimaryPlaneImportResult {
                imported: None,
                cleanup,
            };
        };
        match device.import_scanout_dma_buf(fd.as_fd()) {
            Ok(handle) => {
                plane_handles[index] = Some(handle);
                add_unique_imported_buffer_for_cleanup(&mut cleanup_handles, handle);
            }
            Err(_) => {
                let cleanup = cleanup_imported_buffers_after_failure(
                    device,
                    cleanup_handles,
                    descriptor.size,
                );
                return LibdrmNativePrimaryPlaneImportResult {
                    imported: None,
                    cleanup,
                };
            }
        }
        index += 1;
    }

    LibdrmNativePrimaryPlaneImportResult {
        imported: Some(LibdrmNativePrimaryPlaneImportedBuffer {
            plane_handles,
            cleanup_handles,
        }),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
fn add_unique_imported_buffer_for_cleanup(
    cleanup_handles: &mut [Option<drm::buffer::Handle>; 4],
    handle: drm::buffer::Handle,
) {
    if cleanup_handles.contains(&Some(handle)) {
        return;
    }
    if let Some(slot) = cleanup_handles.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(handle);
    }
}

#[cfg(feature = "libdrm-events")]
fn cleanup_imported_buffers_after_failure<D>(
    device: &D,
    imported_buffers: [Option<drm::buffer::Handle>; 4],
    size: Size,
) -> Option<LibdrmNativePrimaryPlaneResourceCleanup>
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    destroy_native_primary_plane_resource_cleanup(
        device,
        LibdrmNativePrimaryPlaneResourceCleanup::from_imported_buffers(imported_buffers, size),
    )
    .cleanup
}

#[cfg(feature = "libdrm-events")]
fn cleanup_after_framebuffer_create_failure<D>(
    device: &D,
    mode_blob: Option<u64>,
    imported_buffers: [Option<drm::buffer::Handle>; 4],
    size: Size,
) -> Option<LibdrmNativePrimaryPlaneResourceCleanup>
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    let cleanup = match mode_blob {
        Some(mode_blob) => {
            LibdrmNativePrimaryPlaneResourceCleanup::from_mode_blob_and_imported_buffers(
                mode_blob,
                imported_buffers,
                size,
            )
        }
        None => {
            LibdrmNativePrimaryPlaneResourceCleanup::from_imported_buffers(imported_buffers, size)
        }
    };
    destroy_native_primary_plane_resource_cleanup(device, cleanup).cleanup
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
            Err(_) if has_non_linear_modifier(buffer) => {
                LibdrmNativePrimaryPlaneFramebufferCreateResult {
                    detail: LibdrmNativePrimaryPlaneFramebufferCreateDetail::AddFb2ModifiersFailed,
                    framebuffer: None,
                }
            }
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
    let implicit_buffer = LibdrmImplicitPlanarBuffer(buffer);
    if let Ok(framebuffer) = device.add_scanout_framebuffer_without_modifiers(&implicit_buffer) {
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
struct LibdrmImplicitPlanarBuffer<'a, B: drm::buffer::PlanarBuffer + ?Sized>(&'a B);

#[cfg(feature = "libdrm-events")]
impl<B> drm::buffer::PlanarBuffer for LibdrmImplicitPlanarBuffer<'_, B>
where
    B: drm::buffer::PlanarBuffer + ?Sized,
{
    fn size(&self) -> (u32, u32) {
        self.0.size()
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        self.0.format()
    }

    fn modifier(&self) -> Option<drm::buffer::DrmModifier> {
        None
    }

    fn pitches(&self) -> [u32; 4] {
        self.0.pitches()
    }

    fn handles(&self) -> [Option<drm::buffer::Handle>; 4] {
        self.0.handles()
    }

    fn offsets(&self) -> [u32; 4] {
        self.0.offsets()
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
    B: drm::buffer::Buffer + drm::buffer::PlanarBuffer + ?Sized,
{
    let (buffer_width, buffer_height) = drm::buffer::Buffer::size(buffer);
    if buffer_width != selection.size.width as u32 || buffer_height != selection.size.height as u32
    {
        return Some(LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch);
    }

    if !is_supported_native_scanout_format(drm::buffer::Buffer::format(buffer))
        || !is_supported_native_scanout_buffer_planes(buffer)
        || buffer.pitch() < buffer_width * LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL
    {
        return Some(LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer);
    }

    None
}

#[cfg(feature = "libdrm-events")]
const LIVE_RENDERER_SCANOUT_BYTES_PER_XRGB8888_PIXEL: u32 = 4;

#[cfg(feature = "libdrm-events")]
fn active_native_scanout_buffer_planes<B>(buffer: &B) -> usize
where
    B: drm::buffer::PlanarBuffer + ?Sized,
{
    buffer
        .handles()
        .iter()
        .filter(|handle| handle.is_some())
        .count()
}

#[cfg(feature = "libdrm-events")]
fn is_supported_native_scanout_buffer_planes<B>(buffer: &B) -> bool
where
    B: drm::buffer::PlanarBuffer + ?Sized,
{
    match active_native_scanout_buffer_planes(buffer) {
        1 => true,
        2..=4 => has_non_linear_modifier(buffer),
        _ => false,
    }
}

#[cfg(feature = "libdrm-events")]
const fn is_supported_native_scanout_format(format: drm::buffer::DrmFourcc) -> bool {
    matches!(
        format,
        drm::buffer::DrmFourcc::Xrgb8888 | drm::buffer::DrmFourcc::Argb8888
    )
}

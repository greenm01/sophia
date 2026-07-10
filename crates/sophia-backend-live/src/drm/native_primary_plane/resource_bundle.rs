use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneResourceBundle {
    pub(crate) framebuffer: drm::control::framebuffer::Handle,
    pub(crate) mode_blob: Option<u64>,
    pub(crate) imported_buffers: [Option<drm::buffer::Handle>; 4],
    pub(crate) size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneResourceBundle {
    pub(crate) const fn new(
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: Option<u64>,
        size: Size,
    ) -> Self {
        Self::new_with_imported_buffers(framebuffer, mode_blob, [None, None, None, None], size)
    }

    pub(crate) const fn new_with_imported_buffers(
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: Option<u64>,
        imported_buffers: [Option<drm::buffer::Handle>; 4],
        size: Size,
    ) -> Self {
        Self {
            framebuffer,
            mode_blob,
            imported_buffers,
            size,
        }
    }

    pub const fn into_objects(
        self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> LibdrmNativePrimaryPlaneObjects {
        selection.into_objects(self.framebuffer, self.mode_blob)
    }
}

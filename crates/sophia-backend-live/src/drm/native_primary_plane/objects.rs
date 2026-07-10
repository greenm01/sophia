use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneObjects {
    pub(crate) connector: drm::control::connector::Handle,
    pub(crate) crtc: drm::control::crtc::Handle,
    pub(crate) plane: drm::control::plane::Handle,
    pub(crate) framebuffer: drm::control::framebuffer::Handle,
    pub(crate) mode_blob: Option<u64>,
    pub(crate) size: Size,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneObjects {
    pub const fn new(
        connector: drm::control::connector::Handle,
        crtc: drm::control::crtc::Handle,
        plane: drm::control::plane::Handle,
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: u64,
        size: Size,
    ) -> Self {
        Self::new_with_optional_mode_blob(
            connector,
            crtc,
            plane,
            framebuffer,
            Some(mode_blob),
            size,
        )
    }

    pub const fn new_with_optional_mode_blob(
        connector: drm::control::connector::Handle,
        crtc: drm::control::crtc::Handle,
        plane: drm::control::plane::Handle,
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: Option<u64>,
        size: Size,
    ) -> Self {
        Self {
            connector,
            crtc,
            plane,
            framebuffer,
            mode_blob,
            size,
        }
    }
}

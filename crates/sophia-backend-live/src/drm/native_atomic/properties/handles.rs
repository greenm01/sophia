#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlanePropertyHandles {
    pub(crate) connector_crtc_id: drm::control::property::Handle,
    pub(crate) crtc_mode_id: drm::control::property::Handle,
    pub(crate) crtc_active: drm::control::property::Handle,
    pub(crate) crtc_vrr_enabled: Option<drm::control::property::Handle>,
    pub(crate) plane_fb_id: drm::control::property::Handle,
    pub(crate) plane_crtc_id: drm::control::property::Handle,
    pub(crate) plane_src_x: drm::control::property::Handle,
    pub(crate) plane_src_y: drm::control::property::Handle,
    pub(crate) plane_src_w: drm::control::property::Handle,
    pub(crate) plane_src_h: drm::control::property::Handle,
    pub(crate) plane_crtc_x: drm::control::property::Handle,
    pub(crate) plane_crtc_y: drm::control::property::Handle,
    pub(crate) plane_crtc_w: drm::control::property::Handle,
    pub(crate) plane_crtc_h: drm::control::property::Handle,
    pub(crate) plane_in_formats: Option<drm::control::property::Handle>,
}

impl LibdrmNativePrimaryPlanePropertyHandles {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        connector_crtc_id: drm::control::property::Handle,
        crtc_mode_id: drm::control::property::Handle,
        crtc_active: drm::control::property::Handle,
        plane_fb_id: drm::control::property::Handle,
        plane_crtc_id: drm::control::property::Handle,
        plane_src_x: drm::control::property::Handle,
        plane_src_y: drm::control::property::Handle,
        plane_src_w: drm::control::property::Handle,
        plane_src_h: drm::control::property::Handle,
        plane_crtc_x: drm::control::property::Handle,
        plane_crtc_y: drm::control::property::Handle,
        plane_crtc_w: drm::control::property::Handle,
        plane_crtc_h: drm::control::property::Handle,
    ) -> Self {
        Self {
            connector_crtc_id,
            crtc_mode_id,
            crtc_active,
            crtc_vrr_enabled: None,
            plane_fb_id,
            plane_crtc_id,
            plane_src_x,
            plane_src_y,
            plane_src_w,
            plane_src_h,
            plane_crtc_x,
            plane_crtc_y,
            plane_crtc_w,
            plane_crtc_h,
            plane_in_formats: None,
        }
    }

    pub const fn with_plane_in_formats(
        mut self,
        plane_in_formats: Option<drm::control::property::Handle>,
    ) -> Self {
        self.plane_in_formats = plane_in_formats;
        self
    }

    pub const fn with_crtc_vrr_enabled(
        mut self,
        crtc_vrr_enabled: Option<drm::control::property::Handle>,
    ) -> Self {
        self.crtc_vrr_enabled = crtc_vrr_enabled;
        self
    }

    pub const fn crtc_vrr_enabled(&self) -> Option<drm::control::property::Handle> {
        self.crtc_vrr_enabled
    }

    pub const fn plane_in_formats(&self) -> Option<drm::control::property::Handle> {
        self.plane_in_formats
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibdrmNativePropertyHandleSet {
    handles: Vec<(String, drm::control::property::Handle)>,
}

impl LibdrmNativePropertyHandleSet {
    pub fn new(
        handles: impl IntoIterator<Item = (impl Into<String>, drm::control::property::Handle)>,
    ) -> Self {
        Self {
            handles: handles
                .into_iter()
                .map(|(name, handle)| (name.into(), handle))
                .collect(),
        }
    }

    pub(super) fn from_property_info_map(
        map: std::collections::HashMap<String, drm::control::property::Info>,
    ) -> Self {
        Self {
            handles: map
                .into_iter()
                .map(|(name, info)| (name, info.handle()))
                .collect(),
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<drm::control::property::Handle> {
        self.handles
            .iter()
            .find_map(|(candidate, handle)| (candidate == name).then_some(*handle))
    }
}

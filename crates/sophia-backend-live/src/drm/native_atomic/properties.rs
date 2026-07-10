use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlanePropertyHandles {
    pub(crate) connector_crtc_id: drm::control::property::Handle,
    pub(crate) crtc_mode_id: drm::control::property::Handle,
    pub(crate) crtc_active: drm::control::property::Handle,
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
        }
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

    pub fn from_property_info_map(
        map: std::collections::HashMap<String, drm::control::property::Info>,
    ) -> Self {
        Self {
            handles: map
                .into_iter()
                .map(|(name, info)| (name, info.handle()))
                .collect(),
        }
    }

    fn get(&self, name: &str) -> Option<drm::control::property::Handle> {
        self.handles
            .iter()
            .find_map(|(candidate, handle)| (candidate == name).then_some(*handle))
    }
}

pub trait LibdrmNativePropertyLookupDevice {
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet>;

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet>;

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet>;
}

impl<D> LibdrmNativePropertyLookupDevice for D
where
    D: drm::control::Device,
{
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        Ok(LibdrmNativePropertyHandleSet::from_property_info_map(
            self.get_properties(connector)?.as_hashmap(self)?,
        ))
    }

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        Ok(LibdrmNativePropertyHandleSet::from_property_info_map(
            self.get_properties(crtc)?.as_hashmap(self)?,
        ))
    }

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        Ok(LibdrmNativePropertyHandleSet::from_property_info_map(
            self.get_properties(plane)?.as_hashmap(self)?,
        ))
    }
}

#[derive(Debug)]
pub struct LibdrmNativePrimaryPlanePropertyDiscoveryResult {
    pub status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyHandles>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlanePropertyDiscoveryStatus {
    Discovered,
    ReadFailed,
    MissingConnectorProperty,
    MissingCrtcProperty,
    MissingPlaneProperty,
}

pub fn discover_native_primary_plane_property_handles<D>(
    device: &D,
    connector: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
    plane: drm::control::plane::Handle,
) -> LibdrmNativePrimaryPlanePropertyDiscoveryResult
where
    D: LibdrmNativePropertyLookupDevice,
{
    let Ok(connector_properties) = device.connector_property_handles(connector) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed,
            properties: None,
        };
    };
    let Some(connector_crtc_id) = connector_properties.get("CRTC_ID") else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingConnectorProperty,
            properties: None,
        };
    };

    let Ok(crtc_properties) = device.crtc_property_handles(crtc) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed,
            properties: None,
        };
    };
    let (Some(crtc_mode_id), Some(crtc_active)) = (
        crtc_properties.get("MODE_ID"),
        crtc_properties.get("ACTIVE"),
    ) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingCrtcProperty,
            properties: None,
        };
    };

    let Ok(plane_properties) = device.plane_property_handles(plane) else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed,
            properties: None,
        };
    };
    let (
        Some(plane_fb_id),
        Some(plane_crtc_id),
        Some(plane_src_x),
        Some(plane_src_y),
        Some(plane_src_w),
        Some(plane_src_h),
        Some(plane_crtc_x),
        Some(plane_crtc_y),
        Some(plane_crtc_w),
        Some(plane_crtc_h),
    ) = (
        plane_properties.get("FB_ID"),
        plane_properties.get("CRTC_ID"),
        plane_properties.get("SRC_X"),
        plane_properties.get("SRC_Y"),
        plane_properties.get("SRC_W"),
        plane_properties.get("SRC_H"),
        plane_properties.get("CRTC_X"),
        plane_properties.get("CRTC_Y"),
        plane_properties.get("CRTC_W"),
        plane_properties.get("CRTC_H"),
    )
    else {
        return LibdrmNativePrimaryPlanePropertyDiscoveryResult {
            status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingPlaneProperty,
            properties: None,
        };
    };

    LibdrmNativePrimaryPlanePropertyDiscoveryResult {
        status: LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered,
        properties: Some(LibdrmNativePrimaryPlanePropertyHandles::new(
            connector_crtc_id,
            crtc_mode_id,
            crtc_active,
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
        )),
    }
}

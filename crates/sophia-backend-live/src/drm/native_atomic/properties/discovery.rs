use super::{LibdrmNativePrimaryPlanePropertyHandles, LibdrmNativePropertyLookupDevice};

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
        properties: Some(
            LibdrmNativePrimaryPlanePropertyHandles::new(
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
            )
            .with_crtc_vrr_enabled(crtc_properties.get("VRR_ENABLED"))
            .with_plane_in_formats(plane_properties.get("IN_FORMATS")),
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeVrrPropertyDiscoveryResult {
    pub status: LibdrmNativeVrrPropertyDiscoveryStatus,
    pub capable: bool,
    pub enable_property: Option<drm::control::property::Handle>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeVrrPropertyDiscoveryStatus {
    Discovered,
    Unsupported,
    MissingEnableProperty,
    ReadFailed,
}

pub fn discover_native_vrr_properties<D>(
    device: &D,
    connector: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
) -> LibdrmNativeVrrPropertyDiscoveryResult
where
    D: LibdrmNativePropertyLookupDevice,
{
    let (Ok(connector_properties), Ok(crtc_properties)) = (
        device.connector_property_handles(connector),
        device.crtc_property_handles(crtc),
    ) else {
        return LibdrmNativeVrrPropertyDiscoveryResult {
            status: LibdrmNativeVrrPropertyDiscoveryStatus::ReadFailed,
            capable: false,
            enable_property: None,
        };
    };
    let Some(capable_property) = connector_properties.get("VRR_CAPABLE") else {
        return LibdrmNativeVrrPropertyDiscoveryResult {
            status: LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported,
            capable: false,
            enable_property: None,
        };
    };
    let Ok(Some(capable_value)) = device.connector_property_value(connector, capable_property)
    else {
        return LibdrmNativeVrrPropertyDiscoveryResult {
            status: LibdrmNativeVrrPropertyDiscoveryStatus::ReadFailed,
            capable: false,
            enable_property: None,
        };
    };
    if capable_value == 0 {
        return LibdrmNativeVrrPropertyDiscoveryResult {
            status: LibdrmNativeVrrPropertyDiscoveryStatus::Unsupported,
            capable: false,
            enable_property: crtc_properties.get("VRR_ENABLED"),
        };
    }
    let Some(enable_property) = crtc_properties.get("VRR_ENABLED") else {
        return LibdrmNativeVrrPropertyDiscoveryResult {
            status: LibdrmNativeVrrPropertyDiscoveryStatus::MissingEnableProperty,
            capable: true,
            enable_property: None,
        };
    };
    LibdrmNativeVrrPropertyDiscoveryResult {
        status: LibdrmNativeVrrPropertyDiscoveryStatus::Discovered,
        capable: true,
        enable_property: Some(enable_property),
    }
}

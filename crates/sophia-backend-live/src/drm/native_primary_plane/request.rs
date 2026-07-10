use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativeAtomicRequestBuildResult {
    pub status: LibdrmNativeAtomicRequestBuildStatus,
    pub request: Option<LibdrmNativeAtomicCommitRequest>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicRequestBuildStatus {
    Built,
    InvalidSize,
    MissingModeBlob,
}

#[cfg(feature = "libdrm-events")]
const LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT: u32 = 16;

#[cfg(feature = "libdrm-events")]
const LIBDRM_NATIVE_PRIMARY_PLANE_MAX_SOURCE_DIMENSION: i32 =
    (u32::MAX >> LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT) as i32;

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_atomic_request(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        LibdrmNativeAtomicCommitRequestScope::Modeset,
    )
}

#[cfg(feature = "libdrm-events")]
pub fn build_native_primary_plane_page_flip_atomic_request(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
) -> LibdrmNativeAtomicRequestBuildResult {
    build_native_primary_plane_atomic_request_with_scope(
        objects,
        properties,
        LibdrmNativeAtomicCommitRequestScope::PageFlip,
    )
}

#[cfg(feature = "libdrm-events")]
fn build_native_primary_plane_atomic_request_with_scope(
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    scope: LibdrmNativeAtomicCommitRequestScope,
) -> LibdrmNativeAtomicRequestBuildResult {
    if !is_valid_native_primary_plane_scanout_size(objects.size) {
        return LibdrmNativeAtomicRequestBuildResult {
            status: LibdrmNativeAtomicRequestBuildStatus::InvalidSize,
            request: None,
        };
    }

    let width = objects.size.width as u64;
    let height = objects.size.height as u64;
    let mut request = drm::control::atomic::AtomicModeReq::new();
    if scope == LibdrmNativeAtomicCommitRequestScope::Modeset {
        let Some(mode_blob) = objects.mode_blob else {
            return LibdrmNativeAtomicRequestBuildResult {
                status: LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob,
                request: None,
            };
        };
        request.add_property(
            objects.connector,
            properties.connector_crtc_id,
            drm::control::property::Value::CRTC(Some(objects.crtc)),
        );
        request.add_property(
            objects.crtc,
            properties.crtc_mode_id,
            drm::control::property::Value::Blob(mode_blob),
        );
        request.add_property(
            objects.crtc,
            properties.crtc_active,
            drm::control::property::Value::Boolean(true),
        );
    }
    add_primary_plane_properties(&mut request, objects, properties, width, height);

    LibdrmNativeAtomicRequestBuildResult {
        status: LibdrmNativeAtomicRequestBuildStatus::Built,
        request: Some(match scope {
            LibdrmNativeAtomicCommitRequestScope::PageFlip => {
                LibdrmNativeAtomicCommitRequest::new(request)
            }
            LibdrmNativeAtomicCommitRequestScope::Modeset => {
                LibdrmNativeAtomicCommitRequest::modeset(request)
            }
        }),
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) const fn is_valid_native_primary_plane_scanout_size(size: Size) -> bool {
    size.width > 0
        && size.height > 0
        && size.width <= LIBDRM_NATIVE_PRIMARY_PLANE_MAX_SOURCE_DIMENSION
        && size.height <= LIBDRM_NATIVE_PRIMARY_PLANE_MAX_SOURCE_DIMENSION
}

#[cfg(feature = "libdrm-events")]
fn add_primary_plane_properties(
    request: &mut drm::control::atomic::AtomicModeReq,
    objects: LibdrmNativePrimaryPlaneObjects,
    properties: LibdrmNativePrimaryPlanePropertyHandles,
    width: u64,
    height: u64,
) {
    request.add_property(
        objects.plane,
        properties.plane_fb_id,
        drm::control::property::Value::Framebuffer(Some(objects.framebuffer)),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_id,
        drm::control::property::Value::CRTC(Some(objects.crtc)),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_x,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_y,
        drm::control::property::Value::UnsignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_w,
        drm::control::property::Value::UnsignedRange(
            width << LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT,
        ),
    );
    request.add_property(
        objects.plane,
        properties.plane_src_h,
        drm::control::property::Value::UnsignedRange(
            height << LIBDRM_NATIVE_PRIMARY_PLANE_SOURCE_FIXED_POINT_SHIFT,
        ),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_x,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_y,
        drm::control::property::Value::SignedRange(0),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_w,
        drm::control::property::Value::UnsignedRange(width),
    );
    request.add_property(
        objects.plane,
        properties.plane_crtc_h,
        drm::control::property::Value::UnsignedRange(height),
    );
}

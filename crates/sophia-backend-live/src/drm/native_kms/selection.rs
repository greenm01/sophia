use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneSelectionResult {
    pub status: LibdrmNativePrimaryPlaneSelectionStatus,
    pub selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneSelectionStatus {
    Selected,
    ReadFailed,
    NoConnectedConnector,
    NoUsableMode,
    NoUsableEncoder,
    NoCompatibleCrtc,
    NoCompatiblePrimaryPlane,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneSelection {
    pub(crate) connector: drm::control::connector::Handle,
    pub(crate) crtc: drm::control::crtc::Handle,
    pub(crate) plane: drm::control::plane::Handle,
    pub(crate) size: Size,
    pub(crate) mode: Option<drm::control::Mode>,
}

impl LibdrmNativePrimaryPlaneSelection {
    pub const fn size(self) -> Size {
        self.size
    }

    pub const fn crtc_route(self, slot: LibdrmNativeOutputSlot) -> LibdrmNativeCrtcRoute {
        LibdrmNativeCrtcRoute::new(self.crtc, slot)
    }

    pub const fn into_objects(
        self,
        framebuffer: drm::control::framebuffer::Handle,
        mode_blob: Option<u64>,
    ) -> LibdrmNativePrimaryPlaneObjects {
        LibdrmNativePrimaryPlaneObjects::new_with_optional_mode_blob(
            self.connector,
            self.crtc,
            self.plane,
            framebuffer,
            mode_blob,
            self.size,
        )
    }
}

pub fn select_native_primary_plane_target<D>(device: &D) -> LibdrmNativePrimaryPlaneSelectionResult
where
    D: LibdrmNativeKmsSelectionDevice,
{
    let (Ok(connectors), Ok(crtcs), Ok(planes)) = (
        device.connector_handles(),
        device.crtc_handles(),
        device.plane_handles(),
    ) else {
        return LibdrmNativePrimaryPlaneSelectionResult {
            status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
            selection: None,
        };
    };

    let mut saw_connected = false;
    let mut saw_mode = false;
    let mut saw_encoder = false;
    let mut saw_crtc = false;

    for connector in connectors {
        let Ok(connector_snapshot) = device.connector_snapshot(connector) else {
            return LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
                selection: None,
            };
        };
        if !connector_snapshot.connected {
            continue;
        }
        saw_connected = true;
        let Some(size) = connector_snapshot.mode_size else {
            continue;
        };
        if size.width <= 0 || size.height <= 0 {
            continue;
        }
        saw_mode = true;

        for encoder in connector_snapshot.ordered_encoders() {
            saw_encoder = true;
            let Ok(encoder_snapshot) = device.encoder_snapshot(encoder) else {
                return LibdrmNativePrimaryPlaneSelectionResult {
                    status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
                    selection: None,
                };
            };
            for crtc in encoder_snapshot.ordered_crtcs() {
                if !crtcs.contains(&crtc) {
                    continue;
                }
                saw_crtc = true;
                let plane = match select_primary_plane_for_crtc(device, &planes, crtc) {
                    Ok(Some(plane)) => plane,
                    Ok(None) => continue,
                    Err(()) => {
                        return LibdrmNativePrimaryPlaneSelectionResult {
                            status: LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed,
                            selection: None,
                        };
                    }
                };
                return LibdrmNativePrimaryPlaneSelectionResult {
                    status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                    selection: Some(LibdrmNativePrimaryPlaneSelection {
                        connector,
                        crtc,
                        plane,
                        size,
                        mode: connector_snapshot.native_mode,
                    }),
                };
            }
        }
    }

    LibdrmNativePrimaryPlaneSelectionResult {
        status: if !saw_connected {
            LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
        } else if !saw_mode {
            LibdrmNativePrimaryPlaneSelectionStatus::NoUsableMode
        } else if !saw_encoder {
            LibdrmNativePrimaryPlaneSelectionStatus::NoUsableEncoder
        } else if !saw_crtc {
            LibdrmNativePrimaryPlaneSelectionStatus::NoCompatibleCrtc
        } else {
            LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
        },
        selection: None,
    }
}

fn select_primary_plane_for_crtc<D>(
    device: &D,
    planes: &[drm::control::plane::Handle],
    crtc: drm::control::crtc::Handle,
) -> Result<Option<drm::control::plane::Handle>, ()>
where
    D: LibdrmNativeKmsSelectionDevice,
{
    for plane in planes.iter().copied() {
        let Ok(snapshot) = device.plane_snapshot(plane) else {
            return Err(());
        };
        if !snapshot.supports_crtc(crtc) {
            continue;
        }
        let Ok(plane_type) = device.plane_type(plane) else {
            return Err(());
        };
        if plane_type == Some(drm::control::PlaneType::Primary) {
            return Ok(Some(plane));
        }
    }
    Ok(None)
}

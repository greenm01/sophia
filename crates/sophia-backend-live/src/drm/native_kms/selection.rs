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

    pub fn connector_id(self) -> u32 {
        self.connector.into()
    }

    pub fn crtc_id(self) -> u32 {
        self.crtc.into()
    }

    pub fn plane_id(self) -> u32 {
        self.plane.into()
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneSelectionSetResult {
    pub status: LibdrmNativePrimaryPlaneSelectionSetStatus,
    pub connected_connectors: usize,
    pub selections: Vec<LibdrmNativePrimaryPlaneSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneSelectionSetStatus {
    SelectedAll,
    Partial,
    ReadFailed,
    NoConnectedConnector,
    CapacityExceeded,
    NoDisjointAssignment,
}

pub fn select_native_primary_plane_targets<D>(
    device: &D,
) -> LibdrmNativePrimaryPlaneSelectionSetResult
where
    D: LibdrmNativeKmsSelectionDevice,
{
    let (Ok(mut connectors), Ok(mut crtcs), Ok(mut planes)) = (
        device.connector_handles(),
        device.crtc_handles(),
        device.plane_handles(),
    ) else {
        return selection_set_failure(LibdrmNativePrimaryPlaneSelectionSetStatus::ReadFailed, 0);
    };
    connectors.sort_by_key(|handle| u32::from(*handle));
    crtcs.sort_by_key(|handle| u32::from(*handle));
    planes.sort_by_key(|handle| u32::from(*handle));

    let mut connected_connectors = 0usize;
    let mut selections = Vec::new();
    let mut used_crtcs = Vec::new();
    let mut used_planes = Vec::new();
    for connector in connectors {
        let Ok(snapshot) = device.connector_snapshot(connector) else {
            return selection_set_failure(
                LibdrmNativePrimaryPlaneSelectionSetStatus::ReadFailed,
                connected_connectors,
            );
        };
        if !snapshot.connected {
            continue;
        }
        connected_connectors = connected_connectors.saturating_add(1);
        if connected_connectors > crate::runtime::LIVE_RENDERED_OUTPUT_CAPACITY {
            return LibdrmNativePrimaryPlaneSelectionSetResult {
                status: LibdrmNativePrimaryPlaneSelectionSetStatus::CapacityExceeded,
                connected_connectors,
                selections,
            };
        }
        let Some(size) = snapshot
            .mode_size
            .filter(|size| size.width > 0 && size.height > 0)
        else {
            continue;
        };

        let mut selected = None;
        for encoder in snapshot.ordered_encoders() {
            let Ok(encoder) = device.encoder_snapshot(encoder) else {
                return selection_set_failure(
                    LibdrmNativePrimaryPlaneSelectionSetStatus::ReadFailed,
                    connected_connectors,
                );
            };
            for crtc in encoder.ordered_crtcs() {
                if !crtcs.contains(&crtc) || used_crtcs.contains(&crtc) {
                    continue;
                }
                for plane in planes.iter().copied() {
                    if used_planes.contains(&plane) {
                        continue;
                    }
                    let Ok(plane_snapshot) = device.plane_snapshot(plane) else {
                        return selection_set_failure(
                            LibdrmNativePrimaryPlaneSelectionSetStatus::ReadFailed,
                            connected_connectors,
                        );
                    };
                    if !plane_snapshot.supports_crtc(crtc) {
                        continue;
                    }
                    let Ok(plane_type) = device.plane_type(plane) else {
                        return selection_set_failure(
                            LibdrmNativePrimaryPlaneSelectionSetStatus::ReadFailed,
                            connected_connectors,
                        );
                    };
                    if plane_type == Some(drm::control::PlaneType::Primary) {
                        selected = Some(LibdrmNativePrimaryPlaneSelection {
                            connector,
                            crtc,
                            plane,
                            size,
                            mode: snapshot.native_mode,
                        });
                        break;
                    }
                }
                if selected.is_some() {
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
        if let Some(selection) = selected {
            used_crtcs.push(selection.crtc);
            used_planes.push(selection.plane);
            selections.push(selection);
        }
    }

    let status = if connected_connectors == 0 {
        LibdrmNativePrimaryPlaneSelectionSetStatus::NoConnectedConnector
    } else if selections.len() == connected_connectors {
        LibdrmNativePrimaryPlaneSelectionSetStatus::SelectedAll
    } else if selections.is_empty() {
        LibdrmNativePrimaryPlaneSelectionSetStatus::NoDisjointAssignment
    } else {
        LibdrmNativePrimaryPlaneSelectionSetStatus::Partial
    };
    LibdrmNativePrimaryPlaneSelectionSetResult {
        status,
        connected_connectors,
        selections,
    }
}

fn selection_set_failure(
    status: LibdrmNativePrimaryPlaneSelectionSetStatus,
    connected_connectors: usize,
) -> LibdrmNativePrimaryPlaneSelectionSetResult {
    LibdrmNativePrimaryPlaneSelectionSetResult {
        status,
        connected_connectors,
        selections: Vec::new(),
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

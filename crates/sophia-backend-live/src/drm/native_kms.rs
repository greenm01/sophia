use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeConnectorSnapshot {
    connected: bool,
    current_encoder: Option<drm::control::encoder::Handle>,
    encoders: Vec<drm::control::encoder::Handle>,
    mode_size: Option<Size>,
    native_mode: Option<drm::control::Mode>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeConnectorSnapshot {
    pub fn new(
        connected: bool,
        current_encoder: Option<drm::control::encoder::Handle>,
        encoders: impl IntoIterator<Item = drm::control::encoder::Handle>,
        mode_size: Option<Size>,
    ) -> Self {
        Self {
            connected,
            current_encoder,
            encoders: encoders.into_iter().collect(),
            mode_size,
            native_mode: None,
        }
    }

    pub fn new_with_native_mode(
        connected: bool,
        current_encoder: Option<drm::control::encoder::Handle>,
        encoders: impl IntoIterator<Item = drm::control::encoder::Handle>,
        mode: Option<drm::control::Mode>,
    ) -> Self {
        let mode_size = mode.map(|mode| {
            let (width, height) = mode.size();
            Size {
                width: i32::from(width),
                height: i32::from(height),
            }
        });
        Self {
            connected,
            current_encoder,
            encoders: encoders.into_iter().collect(),
            mode_size,
            native_mode: mode,
        }
    }

    fn ordered_encoders(&self) -> Vec<drm::control::encoder::Handle> {
        let mut handles = Vec::new();
        if let Some(current) = self.current_encoder {
            handles.push(current);
        }
        for encoder in self.encoders.iter().copied() {
            if !handles.contains(&encoder) {
                handles.push(encoder);
            }
        }
        handles
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativeEncoderSnapshot {
    current_crtc: Option<drm::control::crtc::Handle>,
    compatible_crtcs: Vec<drm::control::crtc::Handle>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeEncoderSnapshot {
    pub fn new(
        current_crtc: Option<drm::control::crtc::Handle>,
        compatible_crtcs: impl IntoIterator<Item = drm::control::crtc::Handle>,
    ) -> Self {
        Self {
            current_crtc,
            compatible_crtcs: compatible_crtcs.into_iter().collect(),
        }
    }

    fn ordered_crtcs(&self) -> Vec<drm::control::crtc::Handle> {
        let mut handles = Vec::new();
        if let Some(current) = self.current_crtc {
            handles.push(current);
        }
        for crtc in self.compatible_crtcs.iter().copied() {
            if !handles.contains(&crtc) {
                handles.push(crtc);
            }
        }
        handles
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePlaneSnapshot {
    compatible_crtcs: Vec<drm::control::crtc::Handle>,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePlaneSnapshot {
    pub fn new(compatible_crtcs: impl IntoIterator<Item = drm::control::crtc::Handle>) -> Self {
        Self {
            compatible_crtcs: compatible_crtcs.into_iter().collect(),
        }
    }

    fn supports_crtc(&self, crtc: drm::control::crtc::Handle) -> bool {
        self.compatible_crtcs.contains(&crtc)
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativeKmsSelectionDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>>;

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>>;

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot>;

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot>;

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>>;

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot>;

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativeKmsSelectionDevice for D
where
    D: drm::control::Device,
{
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        Ok(self.resource_handles()?.connectors().to_vec())
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        Ok(self.resource_handles()?.crtcs().to_vec())
    }

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        let info = self.get_connector(connector, false)?;
        let selected_mode = info
            .modes()
            .iter()
            .find(|mode| {
                mode.mode_type()
                    .contains(drm::control::ModeTypeFlags::PREFERRED)
            })
            .or_else(|| info.modes().first())
            .copied();
        Ok(LibdrmNativeConnectorSnapshot::new_with_native_mode(
            info.state() == drm::control::connector::State::Connected,
            info.current_encoder(),
            info.encoders().iter().copied(),
            selected_mode,
        ))
    }

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        let resources = self.resource_handles()?;
        let info = self.get_encoder(encoder)?;
        Ok(LibdrmNativeEncoderSnapshot::new(
            info.crtc(),
            resources.filter_crtcs(info.possible_crtcs()),
        ))
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        drm::control::Device::plane_handles(self)
    }

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        let resources = self.resource_handles()?;
        let info = self.get_plane(plane)?;
        Ok(LibdrmNativePlaneSnapshot::new(
            resources.filter_crtcs(info.possible_crtcs()),
        ))
    }

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        for (property, value) in self.get_properties(plane)?.iter() {
            let info = self.get_property(*property)?;
            if info
                .name()
                .to_str()
                .map(|name| name == "type")
                .unwrap_or(false)
            {
                return Ok(match *value as u32 {
                    x if x == drm::control::PlaneType::Primary as u32 => {
                        Some(drm::control::PlaneType::Primary)
                    }
                    x if x == drm::control::PlaneType::Overlay as u32 => {
                        Some(drm::control::PlaneType::Overlay)
                    }
                    x if x == drm::control::PlaneType::Cursor as u32 => {
                        Some(drm::control::PlaneType::Cursor)
                    }
                    _ => None,
                });
            }
        }
        Ok(None)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneSelectionResult {
    pub status: LibdrmNativePrimaryPlaneSelectionStatus,
    pub selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

#[cfg(feature = "libdrm-events")]
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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneSelection {
    pub(crate) connector: drm::control::connector::Handle,
    pub(crate) crtc: drm::control::crtc::Handle,
    pub(crate) plane: drm::control::plane::Handle,
    pub(crate) size: Size,
    pub(crate) mode: Option<drm::control::Mode>,
}

#[cfg(feature = "libdrm-events")]
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

#[cfg(feature = "libdrm-events")]
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

#[cfg(feature = "libdrm-events")]
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

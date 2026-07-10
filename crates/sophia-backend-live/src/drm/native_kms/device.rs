use crate::prelude::*;

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

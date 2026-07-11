use crate::prelude::*;

use super::LibdrmNativePropertyHandleSet;

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

    fn connector_property_value(
        &self,
        _connector: drm::control::connector::Handle,
        _property: drm::control::property::Handle,
    ) -> io::Result<Option<u64>> {
        Ok(None)
    }
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

    fn connector_property_value(
        &self,
        connector: drm::control::connector::Handle,
        property: drm::control::property::Handle,
    ) -> io::Result<Option<u64>> {
        Ok(self
            .get_properties(connector)?
            .iter()
            .find_map(|(candidate, value)| (*candidate == property).then_some(*value)))
    }
}

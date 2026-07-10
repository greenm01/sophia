use crate::prelude::*;

pub trait RenderDeviceDiscoveryBackend {
    type Device: AsFd;

    fn open_render_device(&self) -> io::Result<Self::Device>;
}

impl<T> RenderDeviceDiscoveryBackend for &T
where
    T: RenderDeviceDiscoveryBackend + ?Sized,
{
    type Device = T::Device;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        (*self).open_render_device()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRenderDeviceDiscoveryReport {
    pub status: LiveRenderDeviceDiscoveryStatus,
}

impl LiveRenderDeviceDiscoveryReport {
    pub(in crate::startup) fn from_open_result<T>(device: &io::Result<T>) -> Self {
        Self {
            status: if device.is_ok() {
                LiveRenderDeviceDiscoveryStatus::Opened
            } else {
                LiveRenderDeviceDiscoveryStatus::Unavailable
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderDeviceDiscoveryStatus {
    NotRequested,
    Opened,
    Unavailable,
}

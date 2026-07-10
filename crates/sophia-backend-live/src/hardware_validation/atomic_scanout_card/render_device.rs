use super::RealAtomicScanoutCard;
use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutRenderDeviceDiscovery {
    device: std::fs::File,
}

impl RealAtomicScanoutRenderDeviceDiscovery {
    pub fn from_card(card: &RealAtomicScanoutCard) -> io::Result<Self> {
        Ok(Self {
            device: card.try_clone_file()?,
        })
    }
}

impl RenderDeviceDiscoveryBackend for RealAtomicScanoutRenderDeviceDiscovery {
    type Device = std::fs::File;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        self.device.try_clone()
    }
}

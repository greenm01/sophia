use crate::api::*;

#[cfg(feature = "libdrm-events")]
use sophia_renderer_live::{
    LiveRendererScanoutBufferDescriptor, LiveRendererScanoutBufferExportStatus,
};

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedScanoutBufferExport<Owner> {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub descriptor: Option<LiveRendererScanoutBufferDescriptor>,
    pub owner: Option<Owner>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedScanoutBufferExport<Owner> {
    pub fn new(
        status: LiveRendererScanoutBufferExportStatus,
        descriptor: Option<LiveRendererScanoutBufferDescriptor>,
        owner: Option<Owner>,
    ) -> Self {
        match (status, descriptor.is_some() && owner.is_some()) {
            (LiveRendererScanoutBufferExportStatus::Exported, true) => Self {
                status,
                descriptor,
                owner,
            },
            (LiveRendererScanoutBufferExportStatus::Exported, false) => Self {
                status: LiveRendererScanoutBufferExportStatus::Degraded,
                descriptor: None,
                owner: None,
            },
            (status, _) => Self {
                status,
                descriptor: None,
                owner: None,
            },
        }
    }

    pub fn normalized(self) -> Self {
        Self::new(self.status, self.descriptor, self.owner)
    }
}

#[cfg(feature = "libdrm-events")]
pub trait LiveRenderedScanoutBufferExporter {
    type Owner;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner>;
}

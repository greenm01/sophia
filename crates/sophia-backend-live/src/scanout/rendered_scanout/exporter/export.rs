use crate::api::*;

#[cfg(feature = "libdrm-events")]
use std::os::fd::OwnedFd;

#[cfg(feature = "libdrm-events")]
use sophia_renderer_live::{
    LiveRendererScanoutBufferDescriptor, LiveRendererScanoutBufferExportDetail,
    LiveRendererScanoutBufferExportStatus,
};

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedScanoutBufferExport<Owner> {
    pub status: LiveRendererScanoutBufferExportStatus,
    pub detail: LiveRendererScanoutBufferExportDetail,
    pub descriptor: Option<LiveRendererScanoutBufferDescriptor>,
    pub owner: Option<Owner>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedScanoutBufferExport<Owner> {
    pub fn new(
        status: LiveRendererScanoutBufferExportStatus,
        detail: LiveRendererScanoutBufferExportDetail,
        descriptor: Option<LiveRendererScanoutBufferDescriptor>,
        owner: Option<Owner>,
    ) -> Self {
        match (status, descriptor.is_some() && owner.is_some()) {
            (LiveRendererScanoutBufferExportStatus::Exported, true) => Self {
                status,
                detail,
                descriptor,
                owner,
            },
            (LiveRendererScanoutBufferExportStatus::Exported, false) => Self {
                status: LiveRendererScanoutBufferExportStatus::Degraded,
                detail: LiveRendererScanoutBufferExportDetail::RetainedBufferMissing,
                descriptor: None,
                owner: None,
            },
            (status, _) => Self {
                status,
                detail,
                descriptor: None,
                owner: None,
            },
        }
    }

    pub fn normalized(self) -> Self {
        Self::new(self.status, self.detail, self.descriptor, self.owner)
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

#[cfg(feature = "libdrm-events")]
pub trait LiveRenderedScanoutBufferPrimeSource {
    fn export_scanout_dma_buf_fds(&self) -> std::io::Result<Option<LiveRenderedScanoutDmaBufFds>>;
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl LiveRenderedScanoutBufferPrimeSource for sophia_renderer_live::NativeGbmOwnedScanoutBuffer {
    fn export_scanout_dma_buf_fds(&self) -> std::io::Result<Option<LiveRenderedScanoutDmaBufFds>> {
        self.export_scanout_dma_buf_fds()
            .map(LiveRenderedScanoutDmaBufFds::from_native_gbm)
            .map(Some)
    }
}

#[cfg(feature = "libdrm-events")]
pub struct LiveRenderedScanoutDmaBufFds {
    plane_count: u8,
    plane_fds: [Option<OwnedFd>; 4],
}

#[cfg(feature = "libdrm-events")]
impl LiveRenderedScanoutDmaBufFds {
    #[cfg(feature = "gbm-probe")]
    fn from_native_gbm(fds: sophia_renderer_live::NativeGbmScanoutBufferPlaneFds) -> Self {
        Self {
            plane_count: fds.plane_count(),
            plane_fds: fds.into_plane_fds(),
        }
    }

    pub fn new_for_test(plane_fds: [Option<OwnedFd>; 4], plane_count: u8) -> Self {
        Self {
            plane_count,
            plane_fds,
        }
    }

    pub const fn plane_count(&self) -> u8 {
        self.plane_count
    }

    pub fn into_plane_fds(self) -> [Option<OwnedFd>; 4] {
        self.plane_fds
    }
}

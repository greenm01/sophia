#[cfg(feature = "libdrm-events")]
use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::any::Any;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutSubmission<Owner> {
    pub(crate) scanout_buffer: Owner,
    pub(crate) primary_plane: LibdrmNativePrimaryPlaneScanoutSubmission,
    pub(crate) submitted_after_page_flip_serial: Option<u64>,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutSubmission<Owner> {
    pub fn into_scanout_buffer(self) -> Owner {
        self.scanout_buffer
    }

    pub fn map_scanout_buffer<Next>(
        self,
        map: impl FnOnce(Owner) -> Next,
    ) -> LiveRenderedPrimaryPlaneScanoutSubmission<Next> {
        LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: map(self.scanout_buffer),
            primary_plane: self.primary_plane,
            submitted_after_page_flip_serial: self.submitted_after_page_flip_serial,
        }
    }

    pub(crate) fn with_submitted_after_page_flip_serial(
        mut self,
        submitted_after_page_flip_serial: Option<u64>,
    ) -> Self {
        self.submitted_after_page_flip_serial = submitted_after_page_flip_serial;
        self
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) type BoxedRenderedPrimaryPlaneScanoutSubmission =
    LiveRenderedPrimaryPlaneScanoutSubmission<Box<dyn Any>>;

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LiveRenderedPrimaryPlaneScanoutCleanup<Owner> {
    pub(crate) scanout_buffer: Owner,
    pub(crate) primary_plane: LibdrmNativePrimaryPlaneResourceCleanup,
}

#[cfg(feature = "libdrm-events")]
impl<Owner> LiveRenderedPrimaryPlaneScanoutCleanup<Owner> {
    pub fn into_scanout_buffer(self) -> Owner {
        self.scanout_buffer
    }

    pub fn map_scanout_buffer<Next>(
        self,
        map: impl FnOnce(Owner) -> Next,
    ) -> LiveRenderedPrimaryPlaneScanoutCleanup<Next> {
        LiveRenderedPrimaryPlaneScanoutCleanup {
            scanout_buffer: map(self.scanout_buffer),
            primary_plane: self.primary_plane,
        }
    }
}

#[cfg(feature = "libdrm-events")]
pub(crate) type BoxedRenderedPrimaryPlaneScanoutCleanup =
    LiveRenderedPrimaryPlaneScanoutCleanup<Box<dyn Any>>;

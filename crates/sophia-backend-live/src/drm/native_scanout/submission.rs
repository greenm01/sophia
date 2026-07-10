use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmission {
    pub(crate) resources: LibdrmNativePrimaryPlaneResourceBundle,
}

impl LibdrmNativePrimaryPlaneScanoutSubmission {
    pub fn retire<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resources(device, self.resources)
    }
}

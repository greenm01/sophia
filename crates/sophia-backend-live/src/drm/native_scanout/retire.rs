use crate::prelude::*;

pub fn retire_native_primary_plane_scanout_after_page_flip<D>(
    device: &D,
    submission: LibdrmNativePrimaryPlaneScanoutSubmission,
    callback: &LivePageFlipCallbackReport,
) -> LibdrmNativePrimaryPlaneScanoutRetireResult
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    if callback.decision != LivePageFlipCallbackDecision::Accepted
        || callback.event.status != LivePageFlipEventStatus::Presented
    {
        return LibdrmNativePrimaryPlaneScanoutRetireResult {
            status: LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
            destroy: None,
            submission: Some(submission),
            cleanup: None,
        };
    }

    let destroy = submission.retire(device);
    if destroy.status != LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed {
        return LibdrmNativePrimaryPlaneScanoutRetireResult {
            status: LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed,
            destroy: Some(destroy.status),
            submission: None,
            cleanup: destroy.cleanup,
        };
    }

    LibdrmNativePrimaryPlaneScanoutRetireResult {
        status: LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
        destroy: Some(destroy.status),
        submission: None,
        cleanup: None,
    }
}

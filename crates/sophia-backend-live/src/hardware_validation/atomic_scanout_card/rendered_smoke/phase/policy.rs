use crate::prelude::*;

pub(super) fn submit_policy_for_smoke_phase(
    phase: LibdrmNativeAtomicScanoutSmokePhase,
) -> LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    match phase {
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset => {
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset()
        }
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip => {
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip()
        }
    }
}

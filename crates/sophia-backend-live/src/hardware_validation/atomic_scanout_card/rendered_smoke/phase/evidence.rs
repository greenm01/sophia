use crate::prelude::*;

pub(super) fn reduced_smoke_evidence_for_phase(
    phase: LibdrmNativeAtomicScanoutSmokePhase,
    scanout_target: LiveKmsScanoutTargetStatus,
    rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
    gbm_export: LiveRendererScanoutBufferExportStatus,
    gbm_export_detail: LiveRendererScanoutBufferExportDetail,
    submit: Option<&LibdrmNativePrimaryPlaneScanoutSubmitResult>,
    poll: Option<&LibdrmPageFlipEventPollReport>,
    callback: Option<&LivePageFlipCallbackReport>,
    retire: Option<&LibdrmNativePrimaryPlaneScanoutRetireResult>,
) -> LibdrmNativeAtomicScanoutSmokeEvidence {
    match phase {
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset => {
            LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports_with_gbm_export_detail(
                scanout_target,
                rendered_context,
                gbm_export,
                gbm_export_detail,
                submit,
                poll,
                callback,
                retire,
            )
        }
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip => {
            LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports_with_gbm_export_detail(
                scanout_target,
                rendered_context,
                gbm_export,
                gbm_export_detail,
                submit,
                poll,
                callback,
                retire,
            )
        }
    }
}

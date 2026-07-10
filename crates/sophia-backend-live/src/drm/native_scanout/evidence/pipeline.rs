use crate::prelude::*;

impl LibdrmNativeAtomicScanoutSmokeEvidence {
    pub fn from_pipeline_reports(
        scanout_target: LiveKmsScanoutTargetStatus,
        rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
        gbm_export: LiveRendererScanoutBufferExportStatus,
        submit: Option<&LibdrmNativePrimaryPlaneScanoutSubmitResult>,
        poll: Option<&LibdrmPageFlipEventPollReport>,
        callback: Option<&LivePageFlipCallbackReport>,
        retire: Option<&LibdrmNativePrimaryPlaneScanoutRetireResult>,
    ) -> Self {
        Self::from_pipeline_reports_for_phase(
            LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            scanout_target,
            rendered_context,
            gbm_export,
            submit,
            poll,
            callback,
            retire,
        )
    }

    pub fn from_page_flip_pipeline_reports(
        scanout_target: LiveKmsScanoutTargetStatus,
        rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
        gbm_export: LiveRendererScanoutBufferExportStatus,
        submit: Option<&LibdrmNativePrimaryPlaneScanoutSubmitResult>,
        poll: Option<&LibdrmPageFlipEventPollReport>,
        callback: Option<&LivePageFlipCallbackReport>,
        retire: Option<&LibdrmNativePrimaryPlaneScanoutRetireResult>,
    ) -> Self {
        Self::from_pipeline_reports_for_phase(
            LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip,
            scanout_target,
            rendered_context,
            gbm_export,
            submit,
            poll,
            callback,
            retire,
        )
    }

    fn from_pipeline_reports_for_phase(
        phase: LibdrmNativeAtomicScanoutSmokePhase,
        scanout_target: LiveKmsScanoutTargetStatus,
        rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
        gbm_export: LiveRendererScanoutBufferExportStatus,
        submit: Option<&LibdrmNativePrimaryPlaneScanoutSubmitResult>,
        poll: Option<&LibdrmPageFlipEventPollReport>,
        callback: Option<&LivePageFlipCallbackReport>,
        retire: Option<&LibdrmNativePrimaryPlaneScanoutRetireResult>,
    ) -> Self {
        let submit_status = submit.map(|report| report.status);
        let scanout_buffer = submit.map(|report| report.scanout_buffer);
        let properties = submit.and_then(|report| report.properties);
        let resources = submit.and_then(|report| report.resources);
        let request = submit.and_then(|report| report.request);
        let request_scope = submit.and_then(|report| report.request_scope);
        let commit_flags = submit.and_then(|report| report.commit_flags);
        let page_flip_poll = poll.map(|report| report.status);
        let page_flip = callback.map(|report| report.event.status);
        let accepted_page_flip =
            callback.map(|report| report.decision) == Some(LivePageFlipCallbackDecision::Accepted);
        let retire_status = retire.map(|report| report.status);
        let retire_destroy = retire.and_then(|report| report.destroy);
        let retire_cleanup_pending = retire.is_some_and(|report| report.cleanup.is_some());
        let page_flip_wait = LibdrmNativeAtomicScanoutPageFlipWaitStatus::from_reduced_reports(
            page_flip_poll,
            callback,
            retire_status,
            retire_destroy,
            retire_cleanup_pending,
        );

        let status = if scanout_target != LiveKmsScanoutTargetStatus::Ready {
            LibdrmNativeAtomicScanoutSmokeStatus::KmsTargetUnavailable
        } else if rendered_context != Some(LibdrmNativeRenderedScanoutContextStatus::Ready) {
            LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
        } else if gbm_export != LiveRendererScanoutBufferExportStatus::Exported {
            LibdrmNativeAtomicScanoutSmokeStatus::GbmExportFailed
        } else if scanout_buffer != Some(LiveRendererScanoutBufferStatus::Ready) {
            LibdrmNativeAtomicScanoutSmokeStatus::ScanoutBufferUnavailable
        } else if properties != Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered) {
            LibdrmNativeAtomicScanoutSmokeStatus::PropertyDiscoveryFailed
        } else if resources != Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created) {
            LibdrmNativeAtomicScanoutSmokeStatus::ResourceCreationFailed
        } else if request != Some(LibdrmNativeAtomicRequestBuildStatus::Built) {
            LibdrmNativeAtomicScanoutSmokeStatus::RequestBuildFailed
        } else if submit_status
            != Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::AtomicSubmitFailed
        } else if request_scope != Some(phase.required_request_scope())
            || commit_flags != Some(phase.required_commit_flags())
        {
            LibdrmNativeAtomicScanoutSmokeStatus::RequestShapeMismatch
        } else if !accepted_page_flip
            || page_flip_poll != Some(LibdrmPageFlipEventPollStatus::Emitted)
            || page_flip != Some(LivePageFlipEventStatus::Presented)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::PageFlipMissing
        } else if retire_status
            != Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip)
            || retire_destroy != Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
            || retire_cleanup_pending
        {
            LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
        } else {
            LibdrmNativeAtomicScanoutSmokeStatus::Passed
        };

        Self {
            phase,
            status,
            scanout_target: Some(scanout_target),
            rendered_context,
            gbm_export: Some(gbm_export),
            scanout_buffer,
            properties,
            resources,
            request,
            submit: submit_status,
            request_scope,
            commit_flags,
            page_flip_wait: Some(page_flip_wait),
            page_flip_poll,
            page_flip,
            retire: retire_status,
            retire_destroy,
            retire_cleanup_pending,
        }
    }
}

use crate::prelude::*;

use super::submit::{
    LibdrmNativePrimaryPlaneScanoutRetireResult, LibdrmNativePrimaryPlaneScanoutRetireStatus,
    LibdrmNativePrimaryPlaneScanoutSubmitResult, LibdrmNativePrimaryPlaneScanoutSubmitStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicScanoutSmokeEvidence {
    pub phase: LibdrmNativeAtomicScanoutSmokePhase,
    pub status: LibdrmNativeAtomicScanoutSmokeStatus,
    pub scanout_target: Option<LiveKmsScanoutTargetStatus>,
    pub rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
    pub gbm_export: Option<LiveRendererScanoutBufferExportStatus>,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub page_flip_poll: Option<LibdrmPageFlipEventPollStatus>,
    pub page_flip: Option<LivePageFlipEventStatus>,
    pub retire: Option<LibdrmNativePrimaryPlaneScanoutRetireStatus>,
    pub retire_destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub retire_cleanup_pending: bool,
}

impl LibdrmNativeAtomicScanoutSmokeEvidence {
    pub fn reduced_log_line(&self) -> String {
        fn status<T: std::fmt::Debug>(status: Option<T>) -> String {
            status
                .map(|status| format!("{status:?}"))
                .unwrap_or_else(|| "none".to_owned())
        }

        let (commit_page_flip_event, commit_nonblocking, commit_allow_modeset, commit_test_only) =
            self.commit_flags
                .map(|flags| {
                    (
                        flags.page_flip_event.to_string(),
                        flags.nonblocking.to_string(),
                        flags.allow_modeset.to_string(),
                        flags.test_only.to_string(),
                    )
                })
                .unwrap_or_else(|| {
                    (
                        "none".to_owned(),
                        "none".to_owned(),
                        "none".to_owned(),
                        "none".to_owned(),
                    )
                });

        format!(
            "sophia_atomic_scanout_evidence schema=4 phase={:?} status={:?} scanout_target={} rendered_context={} gbm_export={} properties={} resources={} request={} submit={} request_scope={} commit_page_flip_event={} commit_nonblocking={} commit_allow_modeset={} commit_test_only={} page_flip_poll={} page_flip={} retire={} retire_destroy={} retire_cleanup_pending={}",
            self.phase,
            self.status,
            status(self.scanout_target),
            status(self.rendered_context),
            status(self.gbm_export),
            status(self.properties),
            status(self.resources),
            status(self.request),
            status(self.submit),
            status(self.request_scope),
            commit_page_flip_event,
            commit_nonblocking,
            commit_allow_modeset,
            commit_test_only,
            status(self.page_flip_poll),
            status(self.page_flip),
            status(self.retire),
            status(self.retire_destroy),
            self.retire_cleanup_pending,
        )
    }

    pub const fn no_primary_card() -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            properties: None,
            resources: None,
            request: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            page_flip_poll: None,
            page_flip: None,
            retire: None,
            retire_destroy: None,
            retire_cleanup_pending: false,
        }
    }

    pub const fn kms_selection_failed() -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            properties: None,
            resources: None,
            request: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            page_flip_poll: None,
            page_flip: None,
            retire: None,
            retire_destroy: None,
            retire_cleanup_pending: false,
        }
    }

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

        let status = if scanout_target != LiveKmsScanoutTargetStatus::Ready {
            LibdrmNativeAtomicScanoutSmokeStatus::KmsTargetUnavailable
        } else if matches!(
            rendered_context,
            Some(
                LibdrmNativeRenderedScanoutContextStatus::Unavailable
                    | LibdrmNativeRenderedScanoutContextStatus::Degraded
            )
        ) {
            LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
        } else if gbm_export != LiveRendererScanoutBufferExportStatus::Exported {
            LibdrmNativeAtomicScanoutSmokeStatus::GbmExportFailed
        } else if properties != Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
            || resources != Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
            || request != Some(LibdrmNativeAtomicRequestBuildStatus::Built)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::SubmitFailed
        } else if submit_status
            != Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
            || request_scope != Some(phase.required_request_scope())
        {
            LibdrmNativeAtomicScanoutSmokeStatus::SubmitFailed
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
            properties,
            resources,
            request,
            submit: submit_status,
            request_scope,
            commit_flags,
            page_flip_poll,
            page_flip,
            retire: retire_status,
            retire_destroy,
            retire_cleanup_pending,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokePhase {
    InitialModeset,
    SteadyPageFlip,
}

impl LibdrmNativeAtomicScanoutSmokePhase {
    pub const fn required_request_scope(self) -> LibdrmNativeAtomicCommitRequestScope {
        match self {
            Self::InitialModeset => LibdrmNativeAtomicCommitRequestScope::Modeset,
            Self::SteadyPageFlip => LibdrmNativeAtomicCommitRequestScope::PageFlip,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokeStatus {
    Passed,
    NoPrimaryCard,
    KmsSelectionFailed,
    KmsTargetUnavailable,
    RenderedContextUnavailable,
    GbmExportFailed,
    SubmitFailed,
    PageFlipMissing,
    RetireFailed,
}

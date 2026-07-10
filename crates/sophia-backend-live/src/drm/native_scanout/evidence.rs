use crate::prelude::*;

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
    pub scanout_buffer: Option<LiveRendererScanoutBufferStatus>,
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
            "sophia_atomic_scanout_evidence schema=5 phase={:?} status={:?} scanout_target={} rendered_context={} gbm_export={} scanout_buffer={} properties={} resources={} request={} submit={} request_scope={} commit_page_flip_event={} commit_nonblocking={} commit_allow_modeset={} commit_test_only={} page_flip_poll={} page_flip={} retire={} retire_destroy={} retire_cleanup_pending={}",
            self.phase,
            self.status,
            status(self.scanout_target),
            status(self.rendered_context),
            status(self.gbm_export),
            status(self.scanout_buffer),
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
            scanout_buffer: None,
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

    pub const fn smoke_child_timeout() -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::SmokeChildTimeout,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            scanout_buffer: None,
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

    pub const fn primary_card_open_failed() -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::PrimaryCardOpenFailed,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            scanout_buffer: None,
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

    pub const fn client_capability_failed() -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::ClientCapabilityFailed,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            scanout_buffer: None,
            properties: None,
            resources: None,
            request: None,
            request_scope: None,
            submit: None,
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
            scanout_buffer: None,
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

    pub const fn property_discovery_failed() -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status: LibdrmNativeAtomicScanoutSmokeStatus::PropertyDiscoveryFailed,
            scanout_target: Some(LiveKmsScanoutTargetStatus::Ready),
            rendered_context: None,
            gbm_export: None,
            scanout_buffer: None,
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

    pub const fn required_commit_flags(self) -> LibdrmNativeAtomicCommitFlagsReport {
        match self {
            Self::InitialModeset => LibdrmNativeAtomicCommitFlagsReport {
                page_flip_event: true,
                nonblocking: true,
                allow_modeset: true,
                test_only: false,
            },
            Self::SteadyPageFlip => LibdrmNativeAtomicCommitFlagsReport {
                page_flip_event: true,
                nonblocking: true,
                allow_modeset: false,
                test_only: false,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokeStatus {
    Passed,
    SmokeChildTimeout,
    NoPrimaryCard,
    PrimaryCardOpenFailed,
    ClientCapabilityFailed,
    KmsSelectionFailed,
    KmsTargetUnavailable,
    RenderedContextUnavailable,
    GbmExportFailed,
    ScanoutBufferUnavailable,
    RetainedResourceMissing,
    PropertyDiscoveryFailed,
    ResourceCreationFailed,
    RequestBuildFailed,
    AtomicSubmitFailed,
    RequestShapeMismatch,
    PageFlipReaderUnavailable,
    PageFlipMissing,
    RetireFailed,
}

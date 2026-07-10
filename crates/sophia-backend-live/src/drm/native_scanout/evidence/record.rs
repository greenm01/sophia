use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicScanoutSmokeEvidence {
    pub phase: LibdrmNativeAtomicScanoutSmokePhase,
    pub status: LibdrmNativeAtomicScanoutSmokeStatus,
    pub scanout_target: Option<LiveKmsScanoutTargetStatus>,
    pub rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
    pub gbm_export: Option<LiveRendererScanoutBufferExportStatus>,
    pub gbm_export_detail: Option<LiveRendererScanoutBufferExportDetail>,
    pub scanout_buffer: Option<LiveRendererScanoutBufferStatus>,
    pub buffer_format: Option<LibdrmNativeScanoutBufferFormatDetail>,
    pub buffer_modifier: Option<LibdrmNativeScanoutBufferModifierDetail>,
    pub buffer_planes: Option<LibdrmNativeScanoutBufferPlaneDetail>,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub framebuffer: Option<LibdrmNativePrimaryPlaneFramebufferCreateDetail>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub page_flip_wait: Option<LibdrmNativeAtomicScanoutPageFlipWaitStatus>,
    pub page_flip_poll: Option<LibdrmPageFlipEventPollStatus>,
    pub page_flip: Option<LivePageFlipEventStatus>,
    pub retire: Option<LibdrmNativePrimaryPlaneScanoutRetireStatus>,
    pub retire_destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub retire_cleanup_pending: bool,
}

impl LibdrmNativeAtomicScanoutSmokeEvidence {
    const fn initial_failure(
        status: LibdrmNativeAtomicScanoutSmokeStatus,
        scanout_target: Option<LiveKmsScanoutTargetStatus>,
    ) -> Self {
        Self {
            phase: LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            status,
            scanout_target,
            rendered_context: None,
            gbm_export: None,
            gbm_export_detail: None,
            scanout_buffer: None,
            buffer_format: None,
            buffer_modifier: None,
            buffer_planes: None,
            properties: None,
            resources: None,
            framebuffer: None,
            request: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            page_flip_wait: None,
            page_flip_poll: None,
            page_flip: None,
            retire: None,
            retire_destroy: None,
            retire_cleanup_pending: false,
        }
    }

    pub const fn no_primary_card() -> Self {
        Self::initial_failure(LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard, None)
    }

    pub const fn smoke_child_timeout() -> Self {
        Self::initial_failure(
            LibdrmNativeAtomicScanoutSmokeStatus::SmokeChildTimeout,
            None,
        )
    }

    pub const fn primary_card_open_failed() -> Self {
        Self::initial_failure(
            LibdrmNativeAtomicScanoutSmokeStatus::PrimaryCardOpenFailed,
            None,
        )
    }

    pub const fn client_capability_failed() -> Self {
        Self::initial_failure(
            LibdrmNativeAtomicScanoutSmokeStatus::ClientCapabilityFailed,
            None,
        )
    }

    pub const fn kms_selection_failed() -> Self {
        Self::initial_failure(
            LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed,
            None,
        )
    }

    pub const fn property_discovery_failed() -> Self {
        Self::initial_failure(
            LibdrmNativeAtomicScanoutSmokeStatus::PropertyDiscoveryFailed,
            Some(LiveKmsScanoutTargetStatus::Ready),
        )
    }
}

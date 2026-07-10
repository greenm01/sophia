use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub trait LibdrmNativeAtomicCommitDevice {
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()>;
}

#[cfg(feature = "libdrm-events")]
impl<D> LibdrmNativeAtomicCommitDevice for D
where
    D: drm::control::Device,
{
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.atomic_commit(flags, request)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct NativeLibdrmAtomicScanoutCommitter<D> {
    device: D,
    submitted: usize,
    rejected: usize,
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmAtomicScanoutCommitter<D> {
    pub const fn new(device: D) -> Self {
        Self {
            device,
            submitted: 0,
            rejected: 0,
        }
    }

    pub const fn submitted_count(&self) -> usize {
        self.submitted
    }

    pub const fn rejected_count(&self) -> usize {
        self.rejected
    }
}

#[cfg(feature = "libdrm-events")]
impl<D> NativeLibdrmAtomicScanoutCommitter<D>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    pub fn submit_native_atomic_commit(
        &mut self,
        request: LibdrmNativeAtomicCommitRequest,
    ) -> LibdrmNativeAtomicCommitSubmitReport {
        let (flags, request) = request.into_native();
        match self.device.submit_atomic_commit(flags, request) {
            Ok(()) => {
                self.submitted = self.submitted.saturating_add(1);
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::WouldBlock,
                }
            }
            Err(_) => {
                self.rejected = self.rejected.saturating_add(1);
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::Rejected,
                }
            }
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmission {
    resources: LibdrmNativePrimaryPlaneResourceBundle,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneScanoutSubmission {
    pub fn retire<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resources(device, self.resources)
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitResult {
    pub status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
    pub selection: LibdrmNativePrimaryPlaneSelectionStatus,
    pub scanout_buffer: LiveRendererScanoutBufferStatus,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub submit: Option<LibdrmNativeAtomicCommitSubmitStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    KmsTargetUnavailable,
    ScanoutBufferUnavailable,
    PropertyDiscoveryUnavailable,
    ResourceCreationUnavailable,
    AtomicRequestBuildFailed,
    AtomicSubmitFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutRetireResult {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutRetireStatus {
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeAtomicScanoutSmokeEvidence {
    pub status: LibdrmNativeAtomicScanoutSmokeStatus,
    pub scanout_target: Option<LiveKmsScanoutTargetStatus>,
    pub rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
    pub gbm_export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub page_flip_poll: Option<LibdrmPageFlipEventPollStatus>,
    pub page_flip: Option<LivePageFlipEventStatus>,
    pub retire: Option<LibdrmNativePrimaryPlaneScanoutRetireStatus>,
    pub retire_destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub retire_cleanup_pending: bool,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicScanoutSmokeEvidence {
    pub const fn no_primary_card() -> Self {
        Self {
            status: LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            submit: None,
            page_flip_poll: None,
            page_flip: None,
            retire: None,
            retire_destroy: None,
            retire_cleanup_pending: false,
        }
    }

    pub const fn kms_selection_failed() -> Self {
        Self {
            status: LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed,
            scanout_target: None,
            rendered_context: None,
            gbm_export: None,
            submit: None,
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
        let submit_status = submit.map(|report| report.status);
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
        } else if submit_status
            != Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::SubmitFailed
        } else if !accepted_page_flip
            || page_flip_poll != Some(LibdrmPageFlipEventPollStatus::Emitted)
            || page_flip != Some(LivePageFlipEventStatus::Presented)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::PageFlipMissing
        } else if retire_status
            != Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip)
        {
            LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
        } else {
            LibdrmNativeAtomicScanoutSmokeStatus::Passed
        };

        Self {
            status,
            scanout_target: Some(scanout_target),
            rendered_context,
            gbm_export: Some(gbm_export),
            submit: submit_status,
            page_flip_poll,
            page_flip,
            retire: retire_status,
            retire_destroy,
            retire_cleanup_pending,
        }
    }
}

#[cfg(feature = "libdrm-events")]
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

#[cfg(feature = "libdrm-events")]
pub fn submit_native_primary_plane_scanout_from_renderer_descriptor<D>(
    device: &D,
    descriptor: LiveRendererScanoutBufferDescriptor,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    let selection = select_native_primary_plane_target(device);
    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: None,
            resources: None,
            request: None,
            submit: None,
            submission: None,
        };
    };

    let Some(buffer) = LibdrmRendererScanoutBuffer::from_descriptor(descriptor) else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: None,
            resources: None,
            request: None,
            submit: None,
            submission: None,
        };
    };

    let properties = discover_native_primary_plane_property_handles(
        device,
        selected.connector,
        selected.crtc,
        selected.plane,
    );
    let Some(property_handles) = properties.properties else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::PropertyDiscoveryUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: None,
            request: None,
            submit: None,
            submission: None,
        };
    };

    let resources = create_native_primary_plane_resources(device, selected, &buffer);
    let Some(resource_bundle) = resources.resources else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: None,
            submit: None,
            submission: None,
        };
    };

    let request = build_native_primary_plane_atomic_request(
        resource_bundle.into_objects(selected),
        property_handles,
    );
    let Some(request) = request.request else {
        let _ = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(request.status),
            submit: None,
            submission: None,
        };
    };

    let request = request.allow_modeset();
    let (flags, request) = request.into_native();
    let submit = match device.submit_atomic_commit(flags, request) {
        Ok(()) => LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            LibdrmNativeAtomicCommitSubmitStatus::WouldBlock
        }
        Err(_) => LibdrmNativeAtomicCommitSubmitStatus::Rejected,
    };

    if submit != LibdrmNativeAtomicCommitSubmitStatus::Submitted {
        let _ = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
            submit: Some(submit),
            submission: None,
        };
    }

    LibdrmNativePrimaryPlaneScanoutSubmitResult {
        status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        selection: selection.status,
        scanout_buffer: descriptor.status,
        properties: Some(properties.status),
        resources: Some(resources.status),
        request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
        submit: Some(submit),
        submission: Some(LibdrmNativePrimaryPlaneScanoutSubmission {
            resources: resource_bundle,
        }),
    }
}

#[cfg(feature = "libdrm-events")]
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

#[cfg(feature = "libdrm-events")]
impl<D> LiveAtomicScanoutCommitter for NativeLibdrmAtomicScanoutCommitter<D> {
    fn commit_atomic_scanout(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome)
    }

    fn commit_atomic_scanout_after_page_flip(
        &mut self,
        callback: &LivePageFlipCallbackReport,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        LiveAtomicScanoutCommitReport::from_page_flip_callback_and_outcome(callback, outcome)
    }
}

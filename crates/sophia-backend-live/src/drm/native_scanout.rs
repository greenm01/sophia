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
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
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
pub struct LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub allow_modeset: bool,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub const fn page_flip() -> Self {
        Self {
            allow_modeset: false,
        }
    }

    pub const fn modeset() -> Self {
        Self {
            allow_modeset: true,
        }
    }
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
    pub phase: LibdrmNativeAtomicScanoutSmokePhase,
    pub status: LibdrmNativeAtomicScanoutSmokeStatus,
    pub scanout_target: Option<LiveKmsScanoutTargetStatus>,
    pub rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
    pub gbm_export: Option<LiveRendererScanoutBufferExportStatus>,
    pub submit: Option<LibdrmNativePrimaryPlaneScanoutSubmitStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub page_flip_poll: Option<LibdrmPageFlipEventPollStatus>,
    pub page_flip: Option<LivePageFlipEventStatus>,
    pub retire: Option<LibdrmNativePrimaryPlaneScanoutRetireStatus>,
    pub retire_destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub retire_cleanup_pending: bool,
}

#[cfg(feature = "libdrm-events")]
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
            "sophia_atomic_scanout_evidence schema=3 phase={:?} status={:?} scanout_target={} rendered_context={} gbm_export={} submit={} request_scope={} commit_page_flip_event={} commit_nonblocking={} commit_allow_modeset={} commit_test_only={} page_flip_poll={} page_flip={} retire={} retire_destroy={} retire_cleanup_pending={}",
            self.phase,
            self.status,
            status(self.scanout_target),
            status(self.rendered_context),
            status(self.gbm_export),
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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokePhase {
    InitialModeset,
    SteadyPageFlip,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeAtomicScanoutSmokePhase {
    pub const fn required_request_scope(self) -> LibdrmNativeAtomicCommitRequestScope {
        match self {
            Self::InitialModeset => LibdrmNativeAtomicCommitRequestScope::Modeset,
            Self::SteadyPageFlip => LibdrmNativeAtomicCommitRequestScope::PageFlip,
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
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        device, selection, descriptor,
    )
}

#[cfg(feature = "libdrm-events")]
pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
        device,
        selection,
        descriptor,
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset(),
    )
}

#[cfg(feature = "libdrm-events")]
pub fn submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy<D>(
    device: &D,
    selection: LibdrmNativePrimaryPlaneSelectionResult,
    descriptor: LiveRendererScanoutBufferDescriptor,
    policy: LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
) -> LibdrmNativePrimaryPlaneScanoutSubmitResult
where
    D: LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
{
    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: None,
            resources: None,
            request: None,
            request_scope: None,
            commit_flags: None,
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
            request_scope: None,
            commit_flags: None,
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
            request_scope: None,
            commit_flags: None,
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
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
        };
    };

    let objects = resource_bundle.into_objects(selected);
    let request = if policy.allow_modeset {
        build_native_primary_plane_atomic_request(objects, property_handles)
    } else {
        build_native_primary_plane_page_flip_atomic_request(objects, property_handles)
    };
    let Some(request) = request.request else {
        let _ = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection: selection.status,
            scanout_buffer: descriptor.status,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(request.status),
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
        };
    };

    let request = if policy.allow_modeset {
        request.allow_modeset()
    } else {
        request
    };
    let request_scope = request.reduced_scope();
    let commit_flags = request.reduced_flags();
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
            request_scope: Some(request_scope),
            commit_flags: Some(commit_flags),
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
        request_scope: Some(request_scope),
        commit_flags: Some(commit_flags),
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

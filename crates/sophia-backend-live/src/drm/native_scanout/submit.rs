use crate::prelude::*;

use super::commit::LibdrmNativeAtomicCommitDevice;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmission {
    resources: LibdrmNativePrimaryPlaneResourceBundle,
}

impl LibdrmNativePrimaryPlaneScanoutSubmission {
    pub fn retire<D>(self, device: &D) -> LibdrmNativePrimaryPlaneResourceDestroyReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        destroy_native_primary_plane_resources(device, self.resources)
    }
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub allow_modeset: bool,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutRetireResult {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutRetireStatus {
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

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

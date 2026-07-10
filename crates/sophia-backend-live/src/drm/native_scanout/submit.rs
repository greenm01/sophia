use crate::prelude::*;

use super::commit::LibdrmNativeAtomicCommitDevice;

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
    let scanout_buffer = if descriptor.is_valid_scanout_buffer() {
        LiveRendererScanoutBufferStatus::Ready
    } else {
        LiveRendererScanoutBufferStatus::Invalid
    };

    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection: selection.status,
            scanout_buffer,
            properties: None,
            resources: None,
            request: None,
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: None,
        };
    };

    let Some(buffer) = LibdrmRendererScanoutBuffer::from_descriptor(descriptor) else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable,
            selection: selection.status,
            scanout_buffer,
            properties: None,
            resources: None,
            request: None,
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: None,
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
            scanout_buffer,
            properties: Some(properties.status),
            resources: None,
            request: None,
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: None,
        };
    };

    let resources = if policy.allow_modeset {
        create_native_primary_plane_resources(device, selected, &buffer)
    } else {
        create_native_primary_plane_page_flip_resources(device, selected, &buffer)
    };
    let Some(resource_bundle) = resources.resources else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable,
            selection: selection.status,
            scanout_buffer,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: None,
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: resources.cleanup,
        };
    };

    let objects = resource_bundle.into_objects(selected);
    let request = if policy.allow_modeset {
        build_native_primary_plane_atomic_request(objects, property_handles)
    } else {
        build_native_primary_plane_page_flip_atomic_request(objects, property_handles)
    };
    let Some(request) = request.request else {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection: selection.status,
            scanout_buffer,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(request.status),
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: destroy.cleanup,
        };
    };

    let request = if policy.allow_modeset {
        request.allow_modeset()
    } else {
        request
    };
    let request_scope = request.reduced_scope();
    if request_scope != policy.expected_request_scope() {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection: selection.status,
            scanout_buffer,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
            request_scope: Some(request_scope),
            commit_flags: Some(request.reduced_flags()),
            submit: None,
            submission: None,
            cleanup: destroy.cleanup,
        };
    }
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
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        return LibdrmNativePrimaryPlaneScanoutSubmitResult {
            status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed,
            selection: selection.status,
            scanout_buffer,
            properties: Some(properties.status),
            resources: Some(resources.status),
            request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
            request_scope: Some(request_scope),
            commit_flags: Some(commit_flags),
            submit: Some(submit),
            submission: None,
            cleanup: destroy.cleanup,
        };
    }

    LibdrmNativePrimaryPlaneScanoutSubmitResult {
        status: LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        selection: selection.status,
        scanout_buffer,
        properties: Some(properties.status),
        resources: Some(resources.status),
        request: Some(LibdrmNativeAtomicRequestBuildStatus::Built),
        request_scope: Some(request_scope),
        commit_flags: Some(commit_flags),
        submit: Some(submit),
        submission: Some(LibdrmNativePrimaryPlaneScanoutSubmission {
            resources: resource_bundle,
        }),
        cleanup: None,
    }
}

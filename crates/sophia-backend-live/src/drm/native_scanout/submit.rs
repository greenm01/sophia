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

    if selection.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    }

    let Some(selected) = selection.selection else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    };

    let Some(buffer) = LibdrmRendererScanoutBuffer::from_descriptor(descriptor) else {
        return LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
    };

    let properties = discover_native_primary_plane_property_handles(
        device,
        selected.connector,
        selected.crtc,
        selected.plane,
    );
    let Some(property_handles) = properties.properties else {
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::PropertyDiscoveryUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        return result;
    };

    let resources = if policy.allow_modeset {
        create_native_primary_plane_resources(device, selected, &buffer)
    } else {
        create_native_primary_plane_page_flip_resources(device, selected, &buffer)
    };
    let Some(resource_bundle) = resources.resources else {
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.cleanup = resources.cleanup;
        return result;
    };

    let objects = resource_bundle.into_objects(selected);
    let request = if policy.allow_modeset {
        build_native_primary_plane_atomic_request(objects, property_handles)
    } else {
        build_native_primary_plane_page_flip_atomic_request(objects, property_handles)
    };
    let Some(request) = request.request else {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(request.status);
        result.cleanup = destroy.cleanup;
        return result;
    };

    let request = if policy.allow_modeset {
        request.allow_modeset()
    } else {
        request
    };
    let request_scope = request.reduced_scope();
    if request_scope != policy.expected_request_scope() {
        let destroy = destroy_native_primary_plane_resources(device, resource_bundle);
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicRequestBuildFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
        result.request_scope = Some(request_scope);
        result.commit_flags = Some(request.reduced_flags());
        result.cleanup = destroy.cleanup;
        return result;
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
        let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
            LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed,
            selection.status,
            scanout_buffer,
            descriptor,
        );
        result.properties = Some(properties.status);
        result.resources = Some(resources.status);
        result.framebuffer = resources.framebuffer;
        result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
        result.request_scope = Some(request_scope);
        result.commit_flags = Some(commit_flags);
        result.submit = Some(submit);
        result.cleanup = destroy.cleanup;
        return result;
    }

    let mut result = LibdrmNativePrimaryPlaneScanoutSubmitResult::from_descriptor(
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        selection.status,
        scanout_buffer,
        descriptor,
    );
    result.properties = Some(properties.status);
    result.resources = Some(resources.status);
    result.framebuffer = resources.framebuffer;
    result.request = Some(LibdrmNativeAtomicRequestBuildStatus::Built);
    result.request_scope = Some(request_scope);
    result.commit_flags = Some(commit_flags);
    result.submit = Some(submit);
    result.submission = Some(LibdrmNativePrimaryPlaneScanoutSubmission {
        resources: resource_bundle,
    });
    result
}

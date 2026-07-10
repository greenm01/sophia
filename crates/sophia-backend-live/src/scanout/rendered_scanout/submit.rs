#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub(crate) fn submit_rendered_primary_plane_scanout_from_scanout_target_with<D, E>(
    scanout_target: LiveKmsScanoutTargetStatus,
    target: Option<LiveGbmEglFrameTargetRecord>,
    device: &D,
    exporter: &mut E,
) -> LiveRenderedPrimaryPlaneScanoutSubmitResult<E::Owner>
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
    E: LiveRenderedScanoutBufferExporter,
{
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            target.map(|target| target.status),
            None,
        );
    }

    let Some(target) = target else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable,
            scanout_target,
            None,
            None,
        );
    };

    let selection = select_native_primary_plane_target(device);
    let scanout_target =
        reduced_scanout_target_status_from_native_selection(scanout_target, target, &selection);
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            Some(target.status),
            None,
        );
    }

    let export = exporter.export_rendered_scanout_buffer(target).normalized();
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            Some(target.status),
            Some(export.status),
        );
    }

    let (Some(descriptor), Some(owner)) = (export.descriptor, export.owner) else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult::stopped_before_native_submit(
            LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            Some(target.status),
            Some(export.status),
        );
    };

    let mut submit =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            device,
            selection,
            descriptor,
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );
    if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            scanout_buffer: Some(submit.scanout_buffer),
            properties: submit.properties,
            resources: submit.resources,
            request: submit.request,
            submit: Some(submit.status),
            request_scope: submit.request_scope,
            commit_flags: submit.commit_flags,
            commit_submit: submit.submit,
            submission: None,
            cleanup: submit
                .cleanup
                .map(|primary_plane| LiveRenderedPrimaryPlaneScanoutCleanup {
                    scanout_buffer: owner,
                    primary_plane,
                }),
        };
    }

    let Some(primary_plane) = submit.submission.take() else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            scanout_buffer: Some(submit.scanout_buffer),
            properties: submit.properties,
            resources: submit.resources,
            request: submit.request,
            submit: Some(submit.status),
            request_scope: submit.request_scope,
            commit_flags: submit.commit_flags,
            commit_submit: submit.submit,
            submission: None,
            cleanup: None,
        };
    };

    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        scanout_target,
        target: Some(target.status),
        export: Some(export.status),
        scanout_buffer: Some(submit.scanout_buffer),
        properties: submit.properties,
        resources: submit.resources,
        request: submit.request,
        submit: Some(submit.status),
        request_scope: submit.request_scope,
        commit_flags: submit.commit_flags,
        commit_submit: submit.submit,
        submission: Some(LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: owner,
            primary_plane,
            submitted_after_page_flip_serial: None,
        }),
        cleanup: None,
    }
}

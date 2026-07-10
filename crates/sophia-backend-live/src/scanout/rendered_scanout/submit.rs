#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::{any::Any, collections::VecDeque};

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
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            submission: None,
            cleanup: None,
        };
    }

    let Some(target) = target else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::FrameTargetUnavailable,
            scanout_target,
            target: None,
            export: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            submission: None,
            cleanup: None,
        };
    };

    let selection = select_native_primary_plane_target(device);
    let scanout_target =
        reduced_scanout_target_status_from_native_selection(scanout_target, target, &selection);
    if scanout_target != LiveKmsScanoutTargetStatus::Ready {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady,
            scanout_target,
            target: Some(target.status),
            export: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            submission: None,
            cleanup: None,
        };
    }

    let export = exporter.export_rendered_scanout_buffer(target);
    if export.status != LiveRendererScanoutBufferExportStatus::Exported {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            submit: None,
            request_scope: None,
            commit_flags: None,
            submission: None,
            cleanup: None,
        };
    }

    let (Some(descriptor), Some(owner)) = (export.descriptor, export.owner) else {
        return LiveRenderedPrimaryPlaneScanoutSubmitResult {
            status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed,
            scanout_target,
            target: Some(target.status),
            export: Some(export.status),
            submit: None,
            request_scope: None,
            commit_flags: None,
            submission: None,
            cleanup: None,
        };
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
            submit: Some(submit.status),
            request_scope: submit.request_scope,
            commit_flags: submit.commit_flags,
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
            submit: Some(submit.status),
            request_scope: submit.request_scope,
            commit_flags: submit.commit_flags,
            submission: None,
            cleanup: None,
        };
    };

    LiveRenderedPrimaryPlaneScanoutSubmitResult {
        status: LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip,
        scanout_target,
        target: Some(target.status),
        export: Some(export.status),
        submit: Some(submit.status),
        request_scope: submit.request_scope,
        commit_flags: submit.commit_flags,
        submission: Some(LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer: owner,
            primary_plane,
            submitted_after_page_flip_serial: None,
        }),
        cleanup: None,
    }
}

#[cfg(feature = "libdrm-events")]
fn reduced_scanout_target_status_from_native_selection(
    current_status: LiveKmsScanoutTargetStatus,
    target: LiveGbmEglFrameTargetRecord,
    selection: &LibdrmNativePrimaryPlaneSelectionResult,
) -> LiveKmsScanoutTargetStatus {
    if current_status != LiveKmsScanoutTargetStatus::Ready {
        return current_status;
    }

    if target.status != LiveGbmEglFrameTargetStatus::Ready {
        return LiveKmsScanoutTargetStatus::InvalidFrameTarget;
    }

    let Some(selected) = selection.selection else {
        return LiveKmsScanoutTargetStatus::OutputUnavailable;
    };

    if selected.size() != target.size {
        return LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch;
    }

    LiveKmsScanoutTargetStatus::Ready
}

#[cfg(feature = "libdrm-events")]
pub(crate) fn track_rendered_primary_plane_scanout_submit_from_target_with<D, E>(
    scanout_target: LiveKmsScanoutTargetStatus,
    target: Option<LiveGbmEglFrameTargetRecord>,
    rendered_primary_plane_scanout_submission: &mut Option<
        BoxedRenderedPrimaryPlaneScanoutSubmission,
    >,
    rendered_primary_plane_scanout_cleanup: &mut Option<BoxedRenderedPrimaryPlaneScanoutCleanup>,
    rendered_primary_plane_runtime_scanout_state: &mut Option<RuntimeScanoutState>,
    rendered_primary_plane_scanout_in_flight_ticks: &mut u64,
    submitted_after_page_flip_serial: Option<u64>,
    pending_runtime_scanout_states: Option<&mut VecDeque<RuntimeScanoutState>>,
    device: &D,
    exporter: &mut E,
) -> LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport
where
    D: LibdrmNativeKmsSelectionDevice
        + LibdrmNativePropertyLookupDevice
        + LibdrmNativePrimaryPlaneResourceDevice
        + LibdrmNativeAtomicCommitDevice,
    E: LiveRenderedScanoutBufferExporter,
    E::Owner: 'static,
{
    if rendered_primary_plane_scanout_submission.is_some() {
        *rendered_primary_plane_runtime_scanout_state = Some(RuntimeScanoutState::Deferred);
        return LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight,
            scanout_target,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            runtime_scanout_state: Some(RuntimeScanoutState::Deferred),
            in_flight: true,
            in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
        };
    }

    if rendered_primary_plane_scanout_cleanup.is_some() {
        *rendered_primary_plane_runtime_scanout_state = Some(RuntimeScanoutState::Deferred);
        return LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending,
            scanout_target,
            target: target.map(|target| target.status),
            export: None,
            submit: None,
            request_scope: None,
            commit_flags: None,
            runtime_scanout_state: Some(RuntimeScanoutState::Deferred),
            in_flight: false,
            in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
        };
    }

    let mut result = submit_rendered_primary_plane_scanout_from_scanout_target_with(
        scanout_target,
        target,
        device,
        exporter,
    );
    let runtime_scanout_state = Some(result.runtime_scanout_state());

    if let Some(submission) = result.submission.take() {
        *rendered_primary_plane_scanout_submission = Some(
            submission
                .with_submitted_after_page_flip_serial(submitted_after_page_flip_serial)
                .map_scanout_buffer(|owner| Box::new(owner) as Box<dyn Any>),
        );
    }
    if let Some(cleanup) = result.cleanup.take() {
        *rendered_primary_plane_scanout_cleanup =
            Some(cleanup.map_scanout_buffer(|owner| Box::new(owner) as Box<dyn Any>));
    }
    *rendered_primary_plane_scanout_in_flight_ticks = 0;
    *rendered_primary_plane_runtime_scanout_state = runtime_scanout_state;
    if runtime_scanout_state == Some(RuntimeScanoutState::Rejected) {
        if let Some(pending_runtime_scanout_states) = pending_runtime_scanout_states {
            pending_runtime_scanout_states.push_back(RuntimeScanoutState::Rejected);
        }
    }

    LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport {
        status: result.status.into(),
        scanout_target: result.scanout_target,
        target: result.target,
        export: result.export,
        submit: result.submit,
        request_scope: result.request_scope,
        commit_flags: result.commit_flags,
        runtime_scanout_state,
        in_flight: rendered_primary_plane_scanout_submission.is_some(),
        in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
    }
}

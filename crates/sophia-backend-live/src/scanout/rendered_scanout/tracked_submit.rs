#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::{any::Any, collections::VecDeque};

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
            scanout_buffer: None,
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
            scanout_buffer: None,
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
        scanout_buffer: result.scanout_buffer,
        submit: result.submit,
        request_scope: result.request_scope,
        commit_flags: result.commit_flags,
        runtime_scanout_state,
        in_flight: rendered_primary_plane_scanout_submission.is_some(),
        in_flight_ticks: *rendered_primary_plane_scanout_in_flight_ticks,
    }
}

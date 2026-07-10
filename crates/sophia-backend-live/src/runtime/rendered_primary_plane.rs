#[cfg(feature = "libdrm-events")]
use super::*;

#[cfg(feature = "libdrm-events")]
impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn rendered_primary_plane_scanout_in_flight(&self) -> bool {
        self.rendered_primary_plane_scanout_submission.is_some()
    }

    pub fn rendered_primary_plane_scanout_cleanup_pending(&self) -> bool {
        self.rendered_primary_plane_scanout_cleanup.is_some()
    }

    pub const fn rendered_primary_plane_scanout_in_flight_ticks(&self) -> u64 {
        self.rendered_primary_plane_scanout_in_flight_ticks
    }

    pub fn rendered_primary_plane_scanout_backpressure_report(
        &self,
        threshold_ticks: u64,
    ) -> LiveRenderedPrimaryPlaneScanoutBackpressureReport {
        LiveRenderedPrimaryPlaneScanoutBackpressureReport::from_in_flight_state(
            self.rendered_primary_plane_scanout_in_flight(),
            self.rendered_primary_plane_scanout_in_flight_ticks,
            threshold_ticks,
        )
    }

    pub fn rendered_primary_plane_runtime_scanout_state(&self) -> Option<RuntimeScanoutState> {
        self.rendered_primary_plane_runtime_scanout_state
    }

    pub fn pending_runtime_scanout_state_count(&self) -> usize {
        self.pending_runtime_scanout_states.len()
    }

    pub fn submit_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
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
        submit_rendered_primary_plane_scanout_from_scanout_target_with(
            self.kms_scanout_target.status,
            self.gbm_egl_frame_target,
            device,
            exporter,
        )
    }

    pub fn submit_and_track_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
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
        track_rendered_primary_plane_scanout_submit_from_target_with(
            self.kms_scanout_target.status,
            self.gbm_egl_frame_target,
            &mut self.rendered_primary_plane_scanout_submission,
            &mut self.rendered_primary_plane_scanout_cleanup,
            &mut self.rendered_primary_plane_runtime_scanout_state,
            &mut self.rendered_primary_plane_scanout_in_flight_ticks,
            self.page_flip_callback_intake.last_frame_serial(),
            Some(&mut self.pending_runtime_scanout_states),
            device,
            exporter,
        )
    }

    pub fn retire_tracked_rendered_primary_plane_scanout_after_page_flip<D>(
        &mut self,
        device: &D,
        callback: &LivePageFlipCallbackReport,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutRetireReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        let Some(submission) = self.rendered_primary_plane_scanout_submission.take() else {
            return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
                status: LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission,
                destroy: None,
                runtime_scanout_state: None,
                in_flight: false,
                in_flight_ticks: 0,
                cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
            };
        };

        let retired =
            retire_rendered_primary_plane_scanout_after_page_flip(device, submission, callback);
        let runtime_scanout_state = retired.runtime_scanout_state();
        if let Some(submission) = retired.submission {
            self.rendered_primary_plane_scanout_submission = Some(submission);
        } else {
            self.rendered_primary_plane_scanout_in_flight_ticks = 0;
        }
        if let Some(cleanup) = retired.cleanup {
            self.rendered_primary_plane_scanout_cleanup = Some(cleanup);
        }
        if let Some(runtime_scanout_state) = runtime_scanout_state {
            self.rendered_primary_plane_runtime_scanout_state = Some(runtime_scanout_state);
            self.pending_runtime_scanout_states
                .push_back(runtime_scanout_state);
        }

        LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
            status: retired.status.into(),
            destroy: retired.destroy,
            runtime_scanout_state,
            in_flight: self.rendered_primary_plane_scanout_in_flight(),
            in_flight_ticks: self.rendered_primary_plane_scanout_in_flight_ticks,
            cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
        }
    }

    pub fn retry_tracked_rendered_primary_plane_scanout_cleanup<D>(
        &mut self,
        device: &D,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        let Some(cleanup) = self.rendered_primary_plane_scanout_cleanup.take() else {
            return LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
                status: LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending,
                destroy: None,
                cleanup_pending: false,
            };
        };

        let retried = retry_rendered_primary_plane_scanout_cleanup(device, cleanup);
        let status = if retried.cleanup.is_some() {
            LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanupFailed
        } else {
            LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
        };
        if let Some(cleanup) = retried.cleanup {
            self.rendered_primary_plane_scanout_cleanup = Some(cleanup);
        }

        LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
            status,
            destroy: Some(retried.destroy),
            cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
        }
    }

    pub(crate) fn advance_rendered_primary_plane_scanout_age_if_in_flight(&mut self) {
        if self.rendered_primary_plane_scanout_submission.is_some() {
            self.rendered_primary_plane_scanout_in_flight_ticks = self
                .rendered_primary_plane_scanout_in_flight_ticks
                .saturating_add(1);
        }
    }
}

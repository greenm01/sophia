#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use std::any::Any;

#[cfg(feature = "libdrm-events")]
impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn rendered_primary_plane_scanout_in_flight(&self) -> bool {
        self.primary_output_state().in_flight()
    }

    pub fn rendered_primary_plane_scanout_in_flight_for(&self, output: OutputId) -> bool {
        self.outputs
            .get(output)
            .is_some_and(LiveRenderedOutputState::in_flight)
    }

    pub fn rendered_primary_plane_scanout_cleanup_pending(&self) -> bool {
        self.primary_output_state().cleanup_pending()
    }

    pub fn rendered_primary_plane_scanout_cleanup_pending_for(&self, output: OutputId) -> bool {
        self.outputs
            .get(output)
            .is_some_and(LiveRenderedOutputState::cleanup_pending)
    }

    pub fn rendered_primary_plane_scanout_displayed(&self) -> bool {
        self.primary_output_state()
            .rendered_primary_plane_displayed_submission
            .is_some()
    }

    /// Releases the final displayed submission during bounded session
    /// teardown. Persistent scanout intentionally retains that submission
    /// between frames, so callers must retire it through the DRM device before
    /// dropping the renderer-owned buffer.
    pub fn retire_displayed_rendered_primary_plane_scanout<D>(
        &mut self,
        device: &D,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        let state = self.primary_output_state_mut();
        state.retain_rendered_primary_plane_displayed_submission = false;
        let Some(displayed) = state.rendered_primary_plane_displayed_submission.take() else {
            return LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
                status: LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending,
                destroy: None,
                cleanup_pending: state.cleanup_pending(),
            };
        };
        let LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer,
            primary_plane,
            ..
        } = displayed;
        let retired = primary_plane.retire(device);
        let destroy = retired.status;
        if let Some(primary_plane) = retired.cleanup {
            state.rendered_primary_plane_scanout_cleanup =
                Some(LiveRenderedPrimaryPlaneScanoutCleanup {
                    scanout_buffer,
                    primary_plane,
                });
        }
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
            status: if state.cleanup_pending() {
                LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanupFailed
            } else {
                LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
            },
            destroy: Some(destroy),
            cleanup_pending: state.cleanup_pending(),
        }
    }

    pub fn with_persistent_rendered_primary_plane_scanout(mut self) -> Self {
        self.primary_output_state_mut()
            .retain_rendered_primary_plane_displayed_submission = true;
        self
    }

    pub(crate) fn adopt_presented_rendered_primary_plane_scanout<Owner>(
        &mut self,
        submission: LiveRenderedPrimaryPlaneScanoutSubmission<Owner>,
    ) -> bool
    where
        Owner: 'static,
    {
        let state = self.primary_output_state_mut();
        if !state.retain_rendered_primary_plane_displayed_submission
            || state.rendered_primary_plane_displayed_submission.is_some()
            || state.rendered_primary_plane_scanout_submission.is_some()
            || state.rendered_primary_plane_scanout_cleanup.is_some()
        {
            return false;
        }
        state.rendered_primary_plane_displayed_submission =
            Some(submission.map_scanout_buffer(|owner| Box::new(owner) as Box<dyn Any>));
        true
    }

    pub fn rendered_primary_plane_scanout_in_flight_ticks(&self) -> u64 {
        self.primary_output_state()
            .rendered_primary_plane_scanout_in_flight_ticks
    }

    pub fn rendered_primary_plane_scanout_backpressure_report(
        &self,
        threshold_ticks: u64,
    ) -> LiveRenderedPrimaryPlaneScanoutBackpressureReport {
        let state = self.primary_output_state();
        LiveRenderedPrimaryPlaneScanoutBackpressureReport::from_in_flight_state(
            state.in_flight(),
            state.rendered_primary_plane_scanout_in_flight_ticks,
            threshold_ticks,
        )
    }

    pub fn rendered_primary_plane_runtime_scanout_state(&self) -> Option<RuntimeScanoutState> {
        self.primary_output_state()
            .rendered_primary_plane_runtime_scanout_state
    }

    pub fn pending_runtime_scanout_state_count(&self) -> usize {
        self.primary_output_state()
            .pending_runtime_scanout_states
            .len()
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
        E::Owner: LiveRenderedScanoutBufferPrimeSource,
    {
        let state = self.primary_output_state();
        let selection = state.native_selection.map_or_else(
            || select_native_primary_plane_target(device),
            |selection| LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                selection: Some(selection),
            },
        );
        submit_rendered_primary_plane_scanout_from_scanout_target_and_selection_with(
            state.kms_scanout_target.status,
            state.gbm_egl_frame_target,
            selection,
            state.vrr_property_request,
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
        E::Owner: LiveRenderedScanoutBufferPrimeSource + 'static,
    {
        let state = self.primary_output_state_mut();
        let selection = state.native_selection.map_or_else(
            || select_native_primary_plane_target(device),
            |selection| LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                selection: Some(selection),
            },
        );
        track_rendered_primary_plane_scanout_submit_from_target_and_selection_with(
            state.kms_scanout_target.status,
            state.output_size,
            state.gbm_egl_frame_target,
            &mut state.rendered_primary_plane_scanout_submission,
            &mut state.rendered_primary_plane_scanout_cleanup,
            &mut state.rendered_primary_plane_runtime_scanout_state,
            &mut state.rendered_primary_plane_scanout_in_flight_ticks,
            state.page_flip_callback_intake.last_frame_serial(),
            Some(&mut state.pending_runtime_scanout_states),
            selection,
            state.vrr_property_request,
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
        retire_tracked_output_after_page_flip(self.primary_output_state_mut(), device, callback)
    }

    pub fn retry_tracked_rendered_primary_plane_scanout_cleanup<D>(
        &mut self,
        device: &D,
    ) -> LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        retry_tracked_output_cleanup(self.primary_output_state_mut(), device)
    }

    pub fn drain_rendered_primary_plane_page_flip_callbacks_with<D>(
        &mut self,
        device: &D,
    ) -> LiveRenderedPrimaryPlanePageFlipDrainReport
    where
        D: LibdrmNativePrimaryPlaneResourceDevice,
    {
        let page_flip_callbacks = self.drain_page_flip_callback_queue();
        let rendered_primary_plane_scanout_retire =
            page_flip_callbacks.last_accepted.map(|callback| {
                self.retire_tracked_rendered_primary_plane_scanout_after_page_flip(
                    device, &callback,
                )
            });
        LiveRenderedPrimaryPlanePageFlipDrainReport {
            page_flip_callbacks,
            rendered_primary_plane_scanout_retire,
        }
    }

    pub(crate) fn advance_rendered_primary_plane_scanout_age_if_in_flight(&mut self) {
        for state in self.outputs.outputs.values_mut() {
            if state.in_flight() {
                state.rendered_primary_plane_scanout_in_flight_ticks = state
                    .rendered_primary_plane_scanout_in_flight_ticks
                    .saturating_add(1);
            }
        }
    }
}

#[cfg(feature = "libdrm-events")]
fn retire_tracked_output_after_page_flip<D>(
    state: &mut LiveRenderedOutputState,
    device: &D,
    callback: &LivePageFlipCallbackReport,
) -> LiveTrackedRenderedPrimaryPlaneScanoutRetireReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    let Some(submission) = state.rendered_primary_plane_scanout_submission.take() else {
        return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission,
            destroy: None,
            runtime_scanout_state: None,
            in_flight: false,
            in_flight_ticks: 0,
            cleanup_pending: state.cleanup_pending(),
        };
    };

    if !state.retain_rendered_primary_plane_displayed_submission {
        let retired =
            retire_rendered_primary_plane_scanout_after_page_flip(device, submission, callback);
        let runtime_scanout_state = retired.runtime_scanout_state();
        if let Some(submission) = retired.submission {
            state.rendered_primary_plane_scanout_submission = Some(submission);
        } else {
            state.rendered_primary_plane_scanout_in_flight_ticks = 0;
        }
        if let Some(cleanup) = retired.cleanup {
            state.rendered_primary_plane_scanout_cleanup = Some(cleanup);
        }
        if let Some(runtime_scanout_state) = runtime_scanout_state {
            state.rendered_primary_plane_runtime_scanout_state = Some(runtime_scanout_state);
            state
                .pending_runtime_scanout_states
                .push_back(runtime_scanout_state);
        }
        return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
            status: retired.status.into(),
            destroy: retired.destroy,
            runtime_scanout_state,
            in_flight: state.in_flight(),
            in_flight_ticks: state.rendered_primary_plane_scanout_in_flight_ticks,
            cleanup_pending: state.cleanup_pending(),
        };
    }

    let waiting_for_newer_page_flip = callback.decision != LivePageFlipCallbackDecision::Accepted
        || callback.event.status != LivePageFlipEventStatus::Presented
        || submission
            .submitted_after_page_flip_serial
            .is_some_and(|baseline| match callback.event.frame_serial {
                Some(serial) => serial <= baseline,
                None => true,
            });
    if waiting_for_newer_page_flip {
        state.rendered_primary_plane_scanout_submission = Some(submission);
        return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
            status: LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
            destroy: None,
            runtime_scanout_state: None,
            in_flight: true,
            in_flight_ticks: state.rendered_primary_plane_scanout_in_flight_ticks,
            cleanup_pending: state.cleanup_pending(),
        };
    }

    state.rendered_primary_plane_scanout_in_flight_ticks = 0;
    let previous = state
        .rendered_primary_plane_displayed_submission
        .replace(submission);
    let (status, destroy) = match previous {
        None => (
            LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
            None,
        ),
        Some(previous) => {
            let LiveRenderedPrimaryPlaneScanoutSubmission {
                scanout_buffer,
                primary_plane,
                ..
            } = previous;
            let retired = primary_plane.retire(device);
            if retired.status == LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed {
                (
                    LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip,
                    Some(retired.status),
                )
            } else {
                if let Some(primary_plane) = retired.cleanup {
                    state.rendered_primary_plane_scanout_cleanup =
                        Some(LiveRenderedPrimaryPlaneScanoutCleanup {
                            scanout_buffer,
                            primary_plane,
                        });
                }
                (
                    LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed,
                    Some(retired.status),
                )
            }
        }
    };
    let runtime_scanout_state = Some(match status {
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip => {
            RuntimeScanoutState::Retired
        }
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed => {
            RuntimeScanoutState::Rejected
        }
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission
        | LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip => {
            unreachable!("terminal retire statuses are constructed above")
        }
    });
    if let Some(runtime_scanout_state) = runtime_scanout_state {
        state.rendered_primary_plane_runtime_scanout_state = Some(runtime_scanout_state);
        state
            .pending_runtime_scanout_states
            .push_back(runtime_scanout_state);
    }

    LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
        status,
        destroy,
        runtime_scanout_state,
        in_flight: state.in_flight(),
        in_flight_ticks: state.rendered_primary_plane_scanout_in_flight_ticks,
        cleanup_pending: state.cleanup_pending(),
    }
}

#[cfg(feature = "libdrm-events")]
fn retry_tracked_output_cleanup<D>(
    state: &mut LiveRenderedOutputState,
    device: &D,
) -> LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport
where
    D: LibdrmNativePrimaryPlaneResourceDevice,
{
    let Some(cleanup) = state.rendered_primary_plane_scanout_cleanup.take() else {
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
        state.rendered_primary_plane_scanout_cleanup = Some(cleanup);
    }
    LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport {
        status,
        destroy: Some(retried.destroy),
        cleanup_pending: state.cleanup_pending(),
    }
}

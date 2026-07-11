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
        self.rendered_primary_plane_scanout_submission.is_some()
    }

    pub fn rendered_primary_plane_scanout_cleanup_pending(&self) -> bool {
        self.rendered_primary_plane_scanout_cleanup.is_some()
    }

    pub fn rendered_primary_plane_scanout_displayed(&self) -> bool {
        self.rendered_primary_plane_displayed_submission.is_some()
    }

    pub fn with_persistent_rendered_primary_plane_scanout(mut self) -> Self {
        self.retain_rendered_primary_plane_displayed_submission = true;
        self
    }

    pub(crate) fn adopt_presented_rendered_primary_plane_scanout<Owner>(
        &mut self,
        submission: LiveRenderedPrimaryPlaneScanoutSubmission<Owner>,
    ) -> bool
    where
        Owner: 'static,
    {
        if !self.retain_rendered_primary_plane_displayed_submission
            || self.rendered_primary_plane_displayed_submission.is_some()
            || self.rendered_primary_plane_scanout_submission.is_some()
            || self.rendered_primary_plane_scanout_cleanup.is_some()
        {
            return false;
        }
        self.rendered_primary_plane_displayed_submission =
            Some(submission.map_scanout_buffer(|owner| Box::new(owner) as Box<dyn Any>));
        true
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
        E::Owner: LiveRenderedScanoutBufferPrimeSource,
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
        E::Owner: LiveRenderedScanoutBufferPrimeSource + 'static,
    {
        track_rendered_primary_plane_scanout_submit_from_target_with(
            self.kms_scanout_target.status,
            self.output_size,
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

        if !self.retain_rendered_primary_plane_displayed_submission {
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

            return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
                status: retired.status.into(),
                destroy: retired.destroy,
                runtime_scanout_state,
                in_flight: self.rendered_primary_plane_scanout_in_flight(),
                in_flight_ticks: self.rendered_primary_plane_scanout_in_flight_ticks,
                cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
            };
        }

        let waiting_for_newer_page_flip = callback.decision
            != LivePageFlipCallbackDecision::Accepted
            || callback.event.status != LivePageFlipEventStatus::Presented
            || submission
                .submitted_after_page_flip_serial
                .is_some_and(|baseline| match callback.event.frame_serial {
                    Some(serial) => serial <= baseline,
                    None => true,
                });
        if waiting_for_newer_page_flip {
            self.rendered_primary_plane_scanout_submission = Some(submission);
            return LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
                status:
                    LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
                destroy: None,
                runtime_scanout_state: None,
                in_flight: true,
                in_flight_ticks: self.rendered_primary_plane_scanout_in_flight_ticks,
                cleanup_pending: self.rendered_primary_plane_scanout_cleanup_pending(),
            };
        }

        self.rendered_primary_plane_scanout_in_flight_ticks = 0;
        let previous = self
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
                        self.rendered_primary_plane_scanout_cleanup =
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
            self.rendered_primary_plane_runtime_scanout_state = Some(runtime_scanout_state);
            self.pending_runtime_scanout_states
                .push_back(runtime_scanout_state);
        }

        LiveTrackedRenderedPrimaryPlaneScanoutRetireReport {
            status,
            destroy,
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
        if self.rendered_primary_plane_scanout_submission.is_some() {
            self.rendered_primary_plane_scanout_in_flight_ticks = self
                .rendered_primary_plane_scanout_in_flight_ticks
                .saturating_add(1);
        }
    }
}

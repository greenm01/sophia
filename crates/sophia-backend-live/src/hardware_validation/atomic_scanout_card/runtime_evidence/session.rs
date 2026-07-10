use super::super::{RealAtomicScanoutPageFlipSession, RealAtomicScanoutPageFlipWaitPolicy};
use super::observation::real_atomic_runtime_rendered_scanout_renderer_observation;
use crate::prelude::*;

impl RealAtomicScanoutPageFlipSession {
    pub fn run_runtime_rendered_scanout_evidence_lines<R>(
        &mut self,
        output_id: OutputId,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<R>,
        wait_policy: RealAtomicScanoutPageFlipWaitPolicy,
    ) -> Vec<String>
    where
        R: RenderDeviceDiscoveryBackend,
    {
        let output = HeadlessOutput {
            id: output_id,
            size: self.selection().size(),
            scale: 1,
        };
        let frame_target = LiveGbmEglFrameTargetRecord::new(output.size);
        let (sender, receiver) = std::sync::mpsc::sync_channel(4);
        let mut runtime = LiveBackendRuntimeAssembly {
            assembly: HeadlessCompositorBackendAssembly::new(output),
            renderer_observation: real_atomic_runtime_rendered_scanout_renderer_observation(),
            output_size: Some(output.size),
            scanout_readiness: LiveScanoutReadinessReport {
                status: LiveScanoutReadinessStatus::Ready,
            },
            kms_scanout_target: LiveKmsScanoutTargetReport {
                status: LiveKmsScanoutTargetStatus::Ready,
                size: Some(output.size),
            },
            gbm_egl_frame_target: Some(frame_target),
            gbm_egl_frame_target_lifecycle: Some(LiveGbmEglFrameTargetLifecycleReport::created(
                frame_target,
            )),
            gbm_egl_frame_target_allocation: None,
            page_flip_event: LivePageFlipEvent::from_kms_scanout_target_status(
                LiveKmsScanoutTargetStatus::Ready,
            ),
            page_flip_callback_intake: LivePageFlipCallbackIntake::new(output.id),
            page_flip_callback_queue: Some(LivePageFlipCallbackQueue::new(receiver, 4)),
            libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics::not_configured(),
            rendered_primary_plane_scanout_submission: None,
            rendered_primary_plane_scanout_cleanup: None,
            rendered_primary_plane_runtime_scanout_state: None,
            rendered_primary_plane_scanout_in_flight_ticks: 0,
            pending_runtime_scanout_states: VecDeque::new(),
        };

        let first = match runtime
            .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
                CompositorBackendTickInput::default(),
                &self.card,
                exporter,
                &mut self.reader,
                &mut self.poller,
                &sender,
                wait_policy.max_read,
                wait_policy.max_emit,
            ) {
            Ok(report) => report.tick,
            Err(_) => {
                return vec![LiveRuntimeRenderedScanoutEvidenceFailureReport::new(
                    LiveRuntimeRenderedScanoutEvidenceFailureStatus::InitialTickFailed,
                    false,
                    false,
                )
                .reduced_log_line()];
            }
        };

        let Some(submit) = first.rendered_primary_plane_scanout_submit else {
            return vec![
                LiveRuntimeRenderedScanoutEvidenceFailureReport::new(
                    LiveRuntimeRenderedScanoutEvidenceFailureStatus::SubmitReportMissing,
                    false,
                    false,
                )
                .reduced_log_line(),
            ];
        };
        let mut lines = vec![submit.reduced_log_line()];
        if submit.status
            != LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
        {
            return lines;
        }

        let deadline = std::time::Instant::now() + wait_policy.timeout;
        while std::time::Instant::now() < deadline {
            let tick = match runtime
                .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
                    CompositorBackendTickInput::default(),
                    &self.card,
                    exporter,
                    &mut self.reader,
                    &mut self.poller,
                    &sender,
                    wait_policy.max_read,
                    wait_policy.max_emit,
                ) {
                Ok(report) => report.tick,
                Err(_) => {
                    lines.push(
                        LiveRuntimeRenderedScanoutEvidenceFailureReport::new(
                            LiveRuntimeRenderedScanoutEvidenceFailureStatus::RetireTickFailed,
                            true,
                            false,
                        )
                        .reduced_log_line(),
                    );
                    return lines;
                }
            };

            if let Some(retire) = tick.rendered_primary_plane_scanout_retire {
                lines.push(retire.reduced_log_line());
                if let Some(cleanup) = tick.rendered_primary_plane_scanout_cleanup_retry {
                    lines.push(cleanup.reduced_log_line());
                }
                return lines;
            }

            std::thread::sleep(wait_policy.sleep);
        }

        lines.push(
            LiveRuntimeRenderedScanoutEvidenceFailureReport::new(
                LiveRuntimeRenderedScanoutEvidenceFailureStatus::RetireTimedOut,
                true,
                false,
            )
            .reduced_log_line(),
        );
        lines
    }
}

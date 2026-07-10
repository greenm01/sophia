use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::RealAtomicScanoutPageFlipSession;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealAtomicScanoutPageFlipWaitPolicy {
    pub max_read: usize,
    pub max_emit: usize,
    pub timeout: Duration,
    pub sleep: Duration,
}

impl RealAtomicScanoutPageFlipWaitPolicy {
    pub fn hardware_smoke() -> Self {
        Self {
            max_read: 4,
            max_emit: 1,
            timeout: Duration::from_secs(2),
            sleep: Duration::from_millis(5),
        }
    }
}

impl Default for RealAtomicScanoutPageFlipWaitPolicy {
    fn default() -> Self {
        Self::hardware_smoke()
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipWaitReport {
    pub poll: LibdrmPageFlipEventPollReport,
    pub callback_report: Option<LivePageFlipCallbackReport>,
    pub retired: Option<LibdrmNativePrimaryPlaneScanoutRetireResult>,
}

impl RealAtomicScanoutPageFlipSession {
    pub fn wait_for_rendered_submitted_page_flip_retirement<Owner>(
        &mut self,
        intake: &mut LivePageFlipCallbackIntake,
        submission: LiveRenderedPrimaryPlaneScanoutSubmission<Owner>,
        policy: RealAtomicScanoutPageFlipWaitPolicy,
    ) -> RealAtomicScanoutPageFlipWaitReport {
        let LiveRenderedPrimaryPlaneScanoutSubmission {
            scanout_buffer,
            primary_plane,
            submitted_after_page_flip_serial: _,
        } = submission;
        let report = self.wait_for_submitted_page_flip_retirement(intake, primary_plane, policy);
        drop(scanout_buffer);
        report
    }

    pub fn wait_for_submitted_page_flip_retirement(
        &mut self,
        intake: &mut LivePageFlipCallbackIntake,
        submission: LibdrmNativePrimaryPlaneScanoutSubmission,
        policy: RealAtomicScanoutPageFlipWaitPolicy,
    ) -> RealAtomicScanoutPageFlipWaitReport {
        let (sender, receiver) = mpsc::sync_channel(1);
        let deadline = Instant::now() + policy.timeout;
        let mut submission = Some(submission);

        loop {
            let report = self.poller.read_and_poll_page_flip_events(
                &mut self.reader,
                &sender,
                policy.max_read,
                policy.max_emit,
            );
            let last_poll = report.poll;

            if let Ok(callback) = receiver.try_recv() {
                let callback_report = intake.observe(callback);
                let retired = submission.take().map(|submission| {
                    retire_native_primary_plane_scanout_after_page_flip(
                        &self.card,
                        submission,
                        &callback_report,
                    )
                });
                return RealAtomicScanoutPageFlipWaitReport {
                    poll: last_poll,
                    callback_report: Some(callback_report),
                    retired,
                };
            }

            if matches!(
                last_poll.status,
                LibdrmPageFlipEventPollStatus::Disconnected
                    | LibdrmPageFlipEventPollStatus::Backpressure
            ) || Instant::now() >= deadline
            {
                return RealAtomicScanoutPageFlipWaitReport {
                    poll: last_poll,
                    callback_report: None,
                    retired: submission.map(|submission| {
                        LibdrmNativePrimaryPlaneScanoutRetireResult {
                            status:
                                LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
                            destroy: None,
                            submission: Some(submission),
                            cleanup: None,
                        }
                    }),
                };
            }

            std::thread::sleep(policy.sleep);
        }
    }
}

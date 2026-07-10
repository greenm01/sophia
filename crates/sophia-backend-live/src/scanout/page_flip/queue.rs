use super::*;
use crate::prelude::*;
use std::sync::mpsc::{Receiver, TryRecvError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LivePageFlipCallbackQueueReport {
    pub drained: usize,
    pub accepted: usize,
    pub rejected_unexpected_output: usize,
    pub rejected_stale_frame_serial: usize,
    pub last_accepted: Option<LivePageFlipCallbackReport>,
    pub disconnected: bool,
    pub max_reached: bool,
}

impl LivePageFlipCallbackQueueReport {
    fn record_decision(&mut self, decision: LivePageFlipCallbackDecision) {
        match decision {
            LivePageFlipCallbackDecision::Accepted => {
                self.accepted = self.accepted.saturating_add(1);
            }
            LivePageFlipCallbackDecision::RejectedUnexpectedOutput => {
                self.rejected_unexpected_output = self.rejected_unexpected_output.saturating_add(1);
            }
            LivePageFlipCallbackDecision::RejectedStaleFrameSerial => {
                self.rejected_stale_frame_serial =
                    self.rejected_stale_frame_serial.saturating_add(1);
            }
        }
    }
}

pub struct LivePageFlipCallbackQueue {
    receiver: Receiver<LivePageFlipCallback>,
    max_drain_per_tick: usize,
}

impl LivePageFlipCallbackQueue {
    pub fn new(receiver: Receiver<LivePageFlipCallback>, max_drain_per_tick: usize) -> Self {
        Self {
            receiver,
            max_drain_per_tick,
        }
    }

    pub(crate) fn drain_ready(
        &self,
        intake: &mut LivePageFlipCallbackIntake,
        page_flip_event: &mut LivePageFlipEvent,
    ) -> LivePageFlipCallbackQueueReport {
        let mut report = LivePageFlipCallbackQueueReport::default();
        let mut last_rejected_event = None;

        for _ in 0..self.max_drain_per_tick {
            match self.receiver.try_recv() {
                Ok(callback) => {
                    let callback_report = intake.observe(callback);
                    report.drained = report.drained.saturating_add(1);
                    report.record_decision(callback_report.decision);
                    if callback_report.decision == LivePageFlipCallbackDecision::Accepted {
                        report.last_accepted = Some(callback_report);
                    } else {
                        last_rejected_event = Some(callback_report.event);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    report.disconnected = true;
                    break;
                }
            }
        }

        report.max_reached = report.drained == self.max_drain_per_tick;
        if let Some(accepted) = report.last_accepted {
            *page_flip_event = accepted.event;
        } else if let Some(rejected) = last_rejected_event {
            *page_flip_event = rejected;
        }
        report
    }
}

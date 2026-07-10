use crate::prelude::*;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallback {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallbackReport {
    pub decision: LivePageFlipCallbackDecision,
    pub event: LivePageFlipEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePageFlipCallbackDecision {
    Accepted,
    RejectedUnexpectedOutput,
    RejectedStaleFrameSerial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipCallbackIntake {
    expected_output: OutputId,
    last_frame_serial: Option<u64>,
}

impl LivePageFlipCallbackIntake {
    pub const fn new(expected_output: OutputId) -> Self {
        Self {
            expected_output,
            last_frame_serial: None,
        }
    }

    pub const fn last_frame_serial(&self) -> Option<u64> {
        self.last_frame_serial
    }

    pub fn observe(&mut self, callback: LivePageFlipCallback) -> LivePageFlipCallbackReport {
        if callback.output != self.expected_output {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedUnexpectedOutput,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::WaitingForOutput,
                    frame_serial: None,
                },
            };
        }

        if self
            .last_frame_serial
            .is_some_and(|last_frame_serial| callback.frame_serial <= last_frame_serial)
        {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Rejected,
                    frame_serial: Some(callback.frame_serial),
                },
            };
        }

        self.last_frame_serial = Some(callback.frame_serial);
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(callback.frame_serial),
            },
        }
    }
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLibdrmPollerDiagnostics {
    pub status: LiveLibdrmPollerDiagnosticsStatus,
    pub route_count: usize,
    pub pending_callbacks: usize,
    pub decoded_callbacks: usize,
    pub rejected_callbacks: usize,
}

impl LiveLibdrmPollerDiagnostics {
    pub const fn not_configured() -> Self {
        Self {
            status: LiveLibdrmPollerDiagnosticsStatus::NotConfigured,
            route_count: 0,
            pending_callbacks: 0,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }
}

impl Default for LiveLibdrmPollerDiagnostics {
    fn default() -> Self {
        Self::not_configured()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveLibdrmPollerDiagnosticsStatus {
    NotConfigured,
    Idle,
    WouldBlock,
    CallbackDecoded,
    CallbackRejected,
    ReadFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLibdrmPollerStartupReport {
    pub status: LiveLibdrmPollerStartupStatus,
    pub route_count: usize,
}

impl LiveLibdrmPollerStartupReport {
    pub const fn not_configured() -> Self {
        Self {
            status: LiveLibdrmPollerStartupStatus::NotConfigured,
            route_count: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveLibdrmPollerStartupStatus {
    NotConfigured,
    Ready,
    NoOutputs,
    BackendNotReady,
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

        for _ in 0..self.max_drain_per_tick {
            match self.receiver.try_recv() {
                Ok(callback) => {
                    let callback_report = intake.observe(callback);
                    *page_flip_event = callback_report.event;
                    report.drained = report.drained.saturating_add(1);
                    report.record_decision(callback_report.decision);
                    if callback_report.decision == LivePageFlipCallbackDecision::Accepted {
                        report.last_accepted = Some(callback_report);
                    }
                }
                Err(TryRecvError::Empty) => return report,
                Err(TryRecvError::Disconnected) => {
                    report.disconnected = true;
                    return report;
                }
            }
        }

        report.max_reached = true;
        report
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LivePageFlipCallbackSourceReport {
    pub emitted: usize,
    pub queued_remaining: usize,
    pub backpressure: bool,
    pub disconnected: bool,
    pub max_reached: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FakePageFlipCallbackSource {
    queued: VecDeque<LivePageFlipCallback>,
}

impl FakePageFlipCallbackSource {
    pub fn new(callbacks: impl IntoIterator<Item = LivePageFlipCallback>) -> Self {
        Self {
            queued: callbacks.into_iter().collect(),
        }
    }

    pub fn push(&mut self, callback: LivePageFlipCallback) {
        self.queued.push_back(callback);
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn emit_ready(
        &mut self,
        sender: &SyncSender<LivePageFlipCallback>,
        max_emit: usize,
    ) -> LivePageFlipCallbackSourceReport {
        let mut report = LivePageFlipCallbackSourceReport::default();

        for _ in 0..max_emit {
            let Some(callback) = self.queued.pop_front() else {
                report.queued_remaining = self.queued.len();
                return report;
            };

            match sender.try_send(callback) {
                Ok(()) => {
                    report.emitted = report.emitted.saturating_add(1);
                }
                Err(TrySendError::Full(callback)) => {
                    self.queued.push_front(callback);
                    report.backpressure = true;
                    report.queued_remaining = self.queued.len();
                    return report;
                }
                Err(TrySendError::Disconnected(callback)) => {
                    self.queued.push_front(callback);
                    report.disconnected = true;
                    report.queued_remaining = self.queued.len();
                    return report;
                }
            }
        }

        report.queued_remaining = self.queued.len();
        report.max_reached = !self.queued.is_empty();
        report
    }
}

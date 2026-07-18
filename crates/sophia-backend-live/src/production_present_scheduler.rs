use crate::{
    LiveCpuComposedFrame, LivePresentationResourceSession, LivePresentationSubmission,
    LiveProductionAuthorityBatch,
};
use sophia_engine::PreparedSurfaceCommit;
use sophia_protocol::{Rect, SurfaceTransaction, TransactionId};
use std::collections::VecDeque;
use std::error::Error;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct LiveProductionQueuedPresent {
    pub submission: LivePresentationSubmission,
    pub transactions: Vec<SurfaceTransaction>,
    pub cpu_background: Option<LiveCpuComposedFrame>,
    pub target: Rect,
    deadline: Instant,
    not_before: Instant,
}

#[derive(Clone, Debug)]
pub struct LiveProductionSubmittedPresent {
    pub transaction: TransactionId,
    pub prepared: PreparedSurfaceCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionPresentGate {
    Idle,
    SubmittedInFlight,
    WaitingAcquire,
    Reject(TransactionId),
    Ready(TransactionId),
}

#[derive(Debug, Default)]
pub struct LiveProductionPresentScheduler {
    queued: VecDeque<LiveProductionQueuedPresent>,
    submitted: Option<LiveProductionSubmittedPresent>,
    first_acquire_delay: Option<Duration>,
    first_acquire_delay_applied: bool,
    reject_first_present: bool,
    acquire_waits: usize,
    controlled_rejections: usize,
    diagnose_first_mixed_export: bool,
}

impl LiveProductionPresentScheduler {
    pub fn with_controls(
        mut self,
        first_acquire_delay: Option<Duration>,
        reject_first_present: bool,
        diagnose_first_mixed_export: bool,
    ) -> Self {
        self.first_acquire_delay = first_acquire_delay;
        self.reject_first_present = reject_first_present;
        self.diagnose_first_mixed_export = diagnose_first_mixed_export;
        self
    }

    pub fn enqueue_batch(
        &mut self,
        batch: &LiveProductionAuthorityBatch,
        cpu_background: Option<LiveCpuComposedFrame>,
        resources: &mut LivePresentationResourceSession,
        now: Instant,
    ) -> Result<(), Box<dyn Error>> {
        for submission in &batch.present_submissions {
            let transaction = batch
                .transactions
                .iter()
                .find(|transaction| transaction.surface == submission.surface)
                .ok_or("Present submission has no matching Engine transaction")?;
            let submission = LivePresentationSubmission {
                transaction: submission.transaction,
                buffer: submission.buffer,
                acquire_fence: submission.acquire_fence,
                idle_fence: submission.idle_fence,
            };
            resources.begin(submission)?;
            let acquire_delay =
                if !self.first_acquire_delay_applied && self.first_acquire_delay.is_some() {
                    self.first_acquire_delay_applied = true;
                    self.first_acquire_delay.unwrap_or(Duration::ZERO)
                } else {
                    Duration::ZERO
                };
            let not_before = now + acquire_delay;
            self.queued.push_back(LiveProductionQueuedPresent {
                submission,
                transactions: batch.transactions.clone(),
                cpu_background: cpu_background.clone(),
                target: transaction.target_geometry,
                deadline: not_before
                    + Duration::from_millis(u64::from(transaction.timeout_msec.clamp(100, 2_000))),
                not_before,
            });
        }
        Ok(())
    }

    pub fn poll_gate(
        &mut self,
        resources: &mut LivePresentationResourceSession,
        now: Instant,
    ) -> Result<LiveProductionPresentGate, Box<dyn Error>> {
        if self.submitted.is_some() {
            return Ok(LiveProductionPresentGate::SubmittedInFlight);
        }
        let Some(queued) = self.queued.front() else {
            return Ok(LiveProductionPresentGate::Idle);
        };
        let transaction = queued.submission.transaction;
        if now < queued.not_before {
            self.acquire_waits = self.acquire_waits.saturating_add(1);
            return Ok(LiveProductionPresentGate::WaitingAcquire);
        }
        if !resources.poll_acquire_fence(transaction)? {
            self.acquire_waits = self.acquire_waits.saturating_add(1);
            if now >= queued.deadline {
                self.queued.pop_front();
                return Ok(LiveProductionPresentGate::Reject(transaction));
            }
            return Ok(LiveProductionPresentGate::WaitingAcquire);
        }
        if self.reject_first_present {
            self.reject_first_present = false;
            self.controlled_rejections = self.controlled_rejections.saturating_add(1);
            self.queued.pop_front();
            return Ok(LiveProductionPresentGate::Reject(transaction));
        }
        Ok(LiveProductionPresentGate::Ready(transaction))
    }

    pub fn front(&self) -> Option<&LiveProductionQueuedPresent> {
        self.queued.front()
    }

    pub fn pop_front(&mut self) -> Option<LiveProductionQueuedPresent> {
        self.queued.pop_front()
    }

    pub fn mark_submitted(&mut self, submitted: LiveProductionSubmittedPresent) {
        self.submitted = Some(submitted);
    }

    pub fn take_submitted(&mut self) -> Option<LiveProductionSubmittedPresent> {
        self.submitted.take()
    }

    pub fn has_queued(&self) -> bool {
        !self.queued.is_empty()
    }

    pub fn take_diagnose_first_mixed_export(&mut self) -> bool {
        std::mem::take(&mut self.diagnose_first_mixed_export)
    }

    pub fn drain_transactions(&mut self) -> Vec<TransactionId> {
        self.queued
            .drain(..)
            .map(|queued| queued.submission.transaction)
            .collect()
    }

    pub const fn acquire_waits(&self) -> usize {
        self.acquire_waits
    }

    pub const fn controlled_rejections(&self) -> usize {
        self.controlled_rejections
    }
}

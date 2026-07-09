use crate::prelude::*;
use crate::{HeadlessEngine, SurfaceTransactionCommitReadiness, SurfaceVisualStateTable};

#[derive(Clone, Copy, Debug)]
pub struct FramePlanRequest {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameClockTick {
    pub output: OutputId,
    pub frame_serial: u64,
    pub target_msec: u64,
}

pub trait FrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicFrameClock {
    next_serial: u64,
    frame_interval_msec: u64,
}

impl DeterministicFrameClock {
    pub const fn new(start_serial: u64, frame_interval_msec: u64) -> Self {
        Self {
            next_serial: start_serial,
            frame_interval_msec,
        }
    }

    pub const fn next_serial(&self) -> u64 {
        self.next_serial
    }

    pub const fn frame_interval_msec(&self) -> u64 {
        self.frame_interval_msec
    }
}

impl Default for DeterministicFrameClock {
    fn default() -> Self {
        Self::new(1, 16)
    }
}

impl FrameClock for DeterministicFrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick {
        let frame_serial = self.next_serial;
        self.next_serial = self.next_serial.saturating_add(1);

        FrameClockTick {
            output,
            frame_serial,
            target_msec: frame_serial.saturating_mul(self.frame_interval_msec),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutEpochState {
    pub epoch: u64,
    started_msec: u64,
    timeout_msec: u32,
    pending_surfaces: BTreeSet<SurfaceId>,
}

impl LayoutEpochState {
    pub fn new(epoch: u64, pending_surfaces: impl IntoIterator<Item = SurfaceId>) -> Self {
        Self::with_timing(epoch, pending_surfaces, 0, 300)
    }

    pub fn with_timing(
        epoch: u64,
        pending_surfaces: impl IntoIterator<Item = SurfaceId>,
        started_msec: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            epoch,
            started_msec,
            timeout_msec,
            pending_surfaces: pending_surfaces
                .into_iter()
                .filter(|surface| surface.is_valid())
                .collect(),
        }
    }

    pub fn observe_damage(&mut self, damage: &DamageFrame) {
        for surface in &damage.affected_surfaces {
            self.pending_surfaces.remove(surface);
        }
    }

    pub fn is_complete(&self) -> bool {
        self.pending_surfaces.is_empty()
    }

    pub fn pending_surfaces(&self) -> Vec<SurfaceId> {
        self.pending_surfaces.iter().copied().collect()
    }

    pub fn readiness_for_surface(&self, surface: SurfaceId) -> SurfaceTransactionReadiness {
        if self.pending_surfaces.contains(&surface) {
            SurfaceTransactionReadiness::Pending
        } else {
            SurfaceTransactionReadiness::Ready
        }
    }

    pub fn started_msec(&self) -> u64 {
        self.started_msec
    }

    pub fn timeout_msec(&self) -> u32 {
        self.timeout_msec
    }

    pub fn elapsed_msec(&self, now_msec: u64) -> u64 {
        now_msec.saturating_sub(self.started_msec)
    }

    pub fn is_timed_out(&self, now_msec: u64) -> bool {
        !self.is_complete() && self.elapsed_msec(now_msec) >= u64::from(self.timeout_msec)
    }

    pub fn expire_if_timed_out(&mut self, now_msec: u64) -> Option<LayoutEpochTimeout> {
        if !self.is_timed_out(now_msec) {
            return None;
        }

        let pending_surfaces = self.pending_surfaces();
        self.pending_surfaces.clear();

        Some(LayoutEpochTimeout {
            epoch: self.epoch,
            elapsed_msec: self.elapsed_msec(now_msec),
            timeout_msec: self.timeout_msec,
            pending_surfaces,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutEpochTimeout {
    pub epoch: u64,
    pub elapsed_msec: u64,
    pub timeout_msec: u32,
    pub pending_surfaces: Vec<SurfaceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResizeBehaviorSample {
    pub epoch: u64,
    pub elapsed_msec: u64,
    pub timeout_msec: u32,
    pub completed: bool,
    pub timed_out: bool,
    pub pending_surfaces: Vec<SurfaceId>,
}

pub fn measure_resize_behavior(epoch: &LayoutEpochState, now_msec: u64) -> ResizeBehaviorSample {
    ResizeBehaviorSample {
        epoch: epoch.epoch,
        elapsed_msec: epoch.elapsed_msec(now_msec),
        timeout_msec: epoch.timeout_msec(),
        completed: epoch.is_complete(),
        timed_out: epoch.is_timed_out(now_msec),
        pending_surfaces: epoch.pending_surfaces(),
    }
}

pub fn explicit_sync_surfaces(layers: &[LayerSnapshot]) -> Vec<SurfaceId> {
    layers
        .iter()
        .filter(|layer| layer.resize_sync == ResizeSyncCapability::ExplicitSync)
        .map(|layer| layer.surface)
        .filter(|surface| surface.is_valid())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn layout_epoch_for_explicit_sync(
    epoch: u64,
    started_msec: u64,
    timeout_msec: u32,
    layers: &[LayerSnapshot],
) -> Option<LayoutEpochState> {
    let surfaces = explicit_sync_surfaces(layers);
    if surfaces.is_empty() {
        return None;
    }

    Some(LayoutEpochState::with_timing(
        epoch,
        surfaces,
        started_msec,
        timeout_msec,
    ))
}

pub fn surface_transaction_readiness_for_epoch(
    surface: SurfaceId,
    layout_epoch: Option<&LayoutEpochState>,
) -> SurfaceTransactionReadiness {
    layout_epoch
        .map(|epoch| epoch.readiness_for_surface(surface))
        .unwrap_or(SurfaceTransactionReadiness::Ready)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameScheduleDecision {
    WaitForDamage,
    WaitForLayoutEpoch {
        epoch: u64,
        pending_surfaces: Vec<SurfaceId>,
    },
    Render {
        output: OutputId,
        frame_serial: u64,
        damage: DamageFrame,
        completed_epoch: Option<u64>,
    },
}

pub fn schedule_frame_from_damage(
    tick: FrameClockTick,
    damage: Option<DamageFrame>,
    layout_epoch: Option<&mut LayoutEpochState>,
) -> FrameScheduleDecision {
    let Some(damage) = damage else {
        return match layout_epoch {
            Some(epoch) if !epoch.is_complete() => FrameScheduleDecision::WaitForLayoutEpoch {
                epoch: epoch.epoch,
                pending_surfaces: epoch.pending_surfaces(),
            },
            _ => FrameScheduleDecision::WaitForDamage,
        };
    };

    if damage.output != tick.output {
        return FrameScheduleDecision::WaitForDamage;
    }

    let mut completed_epoch = None;
    if let Some(epoch) = layout_epoch {
        epoch.observe_damage(&damage);
        if epoch.is_complete() {
            completed_epoch = Some(epoch.epoch);
        } else {
            return FrameScheduleDecision::WaitForLayoutEpoch {
                epoch: epoch.epoch,
                pending_surfaces: epoch.pending_surfaces(),
            };
        }
    }

    if damage.damage.is_empty() && damage.affected_surfaces.is_empty() && completed_epoch.is_none()
    {
        return FrameScheduleDecision::WaitForDamage;
    }

    FrameScheduleDecision::Render {
        output: tick.output,
        frame_serial: tick.frame_serial,
        damage,
        completed_epoch,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageFlipTransactionBatch {
    pub output: OutputId,
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageFlipCommitOutcome {
    Idle,
    WaitingForOutput {
        expected: OutputId,
        actual: OutputId,
        transaction: TransactionId,
    },
    WaitingForTransactionReadiness {
        transaction: TransactionId,
        pending_surfaces: Vec<SurfaceId>,
    },
    Committed {
        frame_serial: u64,
        commit: TransactionCommit,
    },
    Rejected {
        frame_serial: u64,
        commit: TransactionCommit,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PageFlipCommitGate {
    staged: Option<PageFlipTransactionBatch>,
}

impl PageFlipCommitGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stage(
        &mut self,
        output: OutputId,
        transaction: TransactionId,
        transactions: Vec<SurfaceTransaction>,
    ) {
        self.staged = Some(PageFlipTransactionBatch {
            output,
            transaction,
            transactions,
        });
    }

    pub fn staged(&self) -> Option<&PageFlipTransactionBatch> {
        self.staged.as_ref()
    }

    pub fn clear(&mut self) -> Option<PageFlipTransactionBatch> {
        self.staged.take()
    }

    pub fn commit_on_page_flip(
        &mut self,
        engine: &HeadlessEngine,
        tick: FrameClockTick,
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> PageFlipCommitOutcome {
        let Some(batch) = self.staged.as_ref() else {
            return PageFlipCommitOutcome::Idle;
        };

        if batch.output != tick.output {
            return PageFlipCommitOutcome::WaitingForOutput {
                expected: batch.output,
                actual: tick.output,
                transaction: batch.transaction,
            };
        }

        let visual_state = SurfaceVisualStateTable::from_committed_states(committed.clone());
        let pending_surfaces = batch
            .transactions
            .iter()
            .filter_map(|transaction| {
                match visual_state.transaction_commit_readiness(transaction) {
                    SurfaceTransactionCommitReadiness::NotReady(
                        SurfaceTransactionReadiness::Pending
                        | SurfaceTransactionReadiness::TimedOut,
                    ) => Some(transaction.surface),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();

        if !pending_surfaces.is_empty() {
            return PageFlipCommitOutcome::WaitingForTransactionReadiness {
                transaction: batch.transaction,
                pending_surfaces,
            };
        }

        let batch = self
            .staged
            .take()
            .expect("staged page-flip batch should exist after readiness check");
        let commit =
            engine.commit_surface_transactions(batch.transaction, &batch.transactions, committed);

        if commit.outcome == TransactionOutcome::Committed {
            PageFlipCommitOutcome::Committed {
                frame_serial: tick.frame_serial,
                commit,
            }
        } else {
            PageFlipCommitOutcome::Rejected {
                frame_serial: tick.frame_serial,
                commit,
            }
        }
    }
}

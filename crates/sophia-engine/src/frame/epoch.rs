use crate::prelude::*;

use super::clock::FrameClockTick;

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

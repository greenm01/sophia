use crate::prelude::*;
use crate::{
    DeterministicFrameClock, DrmKmsOutputRegistry, FrameClock, FrameClockTick, PerOutputFrameClock,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputPresentationState {
    pub output: OutputId,
    pub refresh_millihz: u32,
    pub damage_pending: bool,
    pub in_flight_frame: Option<u64>,
    pub last_retired_frame: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPresentationSchedule {
    Scheduled(FrameClockTick),
    WaitingForDamage,
    WaitingForRetirement { frame_serial: u64 },
    UnknownOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPresentationRetire {
    Retired { frame_serial: u64 },
    NoSubmission,
    UnexpectedFrame { expected: u64, actual: u64 },
    UnknownOutput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputPresentationRegistry {
    states: BTreeMap<OutputId, OutputPresentationState>,
    clocks: PerOutputFrameClock,
}

impl OutputPresentationRegistry {
    pub fn from_outputs(outputs: &DrmKmsOutputRegistry) -> Self {
        let states = outputs
            .outputs()
            .map(|output| {
                (
                    output.output,
                    OutputPresentationState {
                        output: output.output,
                        refresh_millihz: output.mode.refresh_millihz,
                        damage_pending: false,
                        in_flight_frame: None,
                        last_retired_frame: None,
                    },
                )
            })
            .collect();
        Self {
            states,
            clocks: PerOutputFrameClock::from_outputs(outputs, DeterministicFrameClock::default()),
        }
    }

    pub fn outputs(&self) -> impl Iterator<Item = &OutputPresentationState> {
        self.states.values()
    }

    pub fn get(&self, output: OutputId) -> Option<&OutputPresentationState> {
        self.states.get(&output)
    }

    pub fn mark_damage(&mut self, output: OutputId) -> bool {
        let Some(state) = self.states.get_mut(&output) else {
            return false;
        };
        state.damage_pending = true;
        true
    }

    pub fn schedule(&mut self, output: OutputId) -> OutputPresentationSchedule {
        let Some(state) = self.states.get_mut(&output) else {
            return OutputPresentationSchedule::UnknownOutput;
        };
        if let Some(frame_serial) = state.in_flight_frame {
            return OutputPresentationSchedule::WaitingForRetirement { frame_serial };
        }
        if !state.damage_pending {
            return OutputPresentationSchedule::WaitingForDamage;
        }
        let tick = self.clocks.next_frame(output);
        state.damage_pending = false;
        state.in_flight_frame = Some(tick.frame_serial);
        OutputPresentationSchedule::Scheduled(tick)
    }

    pub fn retire(&mut self, output: OutputId, frame_serial: u64) -> OutputPresentationRetire {
        let Some(state) = self.states.get_mut(&output) else {
            return OutputPresentationRetire::UnknownOutput;
        };
        let Some(expected) = state.in_flight_frame else {
            return OutputPresentationRetire::NoSubmission;
        };
        if expected != frame_serial {
            return OutputPresentationRetire::UnexpectedFrame {
                expected,
                actual: frame_serial,
            };
        }
        state.in_flight_frame = None;
        state.last_retired_frame = Some(frame_serial);
        OutputPresentationRetire::Retired { frame_serial }
    }
}

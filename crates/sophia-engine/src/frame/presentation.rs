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
    pub last_presented_sequence: Option<u64>,
    pub last_presented_msec: Option<u64>,
    pub next_target_msec: Option<u64>,
    pub submit_deferrals: u64,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPresentationFeedback {
    Accepted {
        sequence: u64,
        presentation_msec: u64,
    },
    NonMonotonicSequence {
        previous: u64,
        actual: u64,
    },
    NonMonotonicTimestamp {
        previous_msec: u64,
        actual_msec: u64,
    },
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
                        last_presented_sequence: None,
                        last_presented_msec: None,
                        next_target_msec: None,
                        submit_deferrals: 0,
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
            state.submit_deferrals = state.submit_deferrals.saturating_add(1);
            return OutputPresentationSchedule::WaitingForRetirement { frame_serial };
        }
        if !state.damage_pending {
            return OutputPresentationSchedule::WaitingForDamage;
        }
        let mut tick = self.clocks.next_frame(output);
        if let Some(target_msec) = state.next_target_msec {
            tick.target_msec = target_msec;
        }
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

    pub fn observe_page_flip(
        &mut self,
        output: OutputId,
        sequence: u64,
        presentation_msec: u64,
    ) -> OutputPresentationFeedback {
        let Some(state) = self.states.get_mut(&output) else {
            return OutputPresentationFeedback::UnknownOutput;
        };
        if let Some(previous) = state.last_presented_sequence
            && sequence <= previous
        {
            return OutputPresentationFeedback::NonMonotonicSequence {
                previous,
                actual: sequence,
            };
        }
        if let Some(previous_msec) = state.last_presented_msec
            && presentation_msec < previous_msec
        {
            return OutputPresentationFeedback::NonMonotonicTimestamp {
                previous_msec,
                actual_msec: presentation_msec,
            };
        }
        let interval_msec = if state.refresh_millihz == 0 {
            16
        } else {
            (1_000_000u64 / u64::from(state.refresh_millihz)).max(1)
        };
        state.last_presented_sequence = Some(sequence);
        state.last_presented_msec = Some(presentation_msec);
        state.next_target_msec = Some(presentation_msec.saturating_add(interval_msec));
        OutputPresentationFeedback::Accepted {
            sequence,
            presentation_msec,
        }
    }
}

use crate::DrmKmsOutputRegistry;
use crate::prelude::*;

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
pub struct PerOutputFrameClock {
    clocks: BTreeMap<OutputId, DeterministicFrameClock>,
    fallback: DeterministicFrameClock,
}

impl PerOutputFrameClock {
    pub fn from_outputs(outputs: &DrmKmsOutputRegistry, fallback: DeterministicFrameClock) -> Self {
        let clocks = outputs
            .outputs()
            .map(|output| {
                let interval_msec = if output.mode.refresh_millihz == 0 {
                    fallback.frame_interval_msec()
                } else {
                    (1_000_000u64 / u64::from(output.mode.refresh_millihz)).max(1)
                };
                (
                    output.output,
                    DeterministicFrameClock::new(fallback.next_serial(), interval_msec),
                )
            })
            .collect();
        Self { clocks, fallback }
    }

    pub fn get(&self, output: OutputId) -> Option<&DeterministicFrameClock> {
        self.clocks.get(&output)
    }

    pub fn outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.clocks.keys().copied()
    }
}

impl FrameClock for PerOutputFrameClock {
    fn next_frame(&mut self, output: OutputId) -> FrameClockTick {
        self.clocks
            .get_mut(&output)
            .unwrap_or(&mut self.fallback)
            .next_frame(output)
    }
}

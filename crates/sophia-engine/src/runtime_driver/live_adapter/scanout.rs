use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveScanoutRuntimeAdapter {
    submit_state: RuntimeScanoutState,
}

impl LiveScanoutRuntimeAdapter {
    pub fn from_submit_state(submit_state: RuntimeScanoutState) -> Self {
        Self { submit_state }
    }

    pub fn submitted() -> Self {
        Self::from_submit_state(RuntimeScanoutState::Submitted)
    }

    pub fn rejected() -> Self {
        Self::from_submit_state(RuntimeScanoutState::Rejected)
    }

    pub fn submit_observation(&self, frame_serial: u64) -> SessionRuntimeObservation {
        SessionRuntimeObservation::ScanoutStateChanged {
            state: self.submit_state,
            frame_serial: Some(frame_serial),
        }
    }
}

impl Default for LiveScanoutRuntimeAdapter {
    fn default() -> Self {
        Self::submitted()
    }
}

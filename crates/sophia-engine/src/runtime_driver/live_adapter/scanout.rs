use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveScanoutRuntimeAdapter {
    submit_state: RuntimeScanoutState,
    lifecycle_states: Vec<RuntimeScanoutState>,
}

impl LiveScanoutRuntimeAdapter {
    pub fn from_submit_state(submit_state: RuntimeScanoutState) -> Self {
        Self {
            submit_state,
            lifecycle_states: Vec::new(),
        }
    }

    pub fn from_states(
        submit_state: RuntimeScanoutState,
        lifecycle_states: Vec<RuntimeScanoutState>,
    ) -> Self {
        Self {
            submit_state,
            lifecycle_states,
        }
    }

    pub fn submitted() -> Self {
        Self::from_submit_state(RuntimeScanoutState::Submitted)
    }

    pub fn deferred() -> Self {
        Self::from_submit_state(RuntimeScanoutState::Deferred)
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

    pub fn lifecycle_observations(&self) -> Vec<SessionRuntimeObservation> {
        self.lifecycle_states
            .iter()
            .copied()
            .map(|state| SessionRuntimeObservation::ScanoutStateChanged {
                state,
                frame_serial: None,
            })
            .collect()
    }
}

impl Default for LiveScanoutRuntimeAdapter {
    fn default() -> Self {
        Self::submitted()
    }
}

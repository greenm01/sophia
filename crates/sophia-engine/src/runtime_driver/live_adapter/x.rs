use crate::prelude::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveXRuntimeAdapter {
    pub pending_event_count: u32,
}

impl LiveXRuntimeAdapter {
    pub fn from_polled_event_count(count: u32) -> Self {
        Self {
            pending_event_count: count,
        }
    }

    pub fn poll_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::XEventsPolled {
            count: self.pending_event_count,
        }
    }
}

use crate::prelude::*;
use crate::runtime_observation_from_authority_transaction_commit;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveXRuntimeAdapter {
    pub pending_event_count: u32,
    pub authority_commits: Vec<TransactionCommit>,
}

impl LiveXRuntimeAdapter {
    pub fn from_polled_event_count(count: u32) -> Self {
        Self {
            pending_event_count: count,
            authority_commits: Vec::new(),
        }
    }

    pub fn poll_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::XEventsPolled {
            count: self.pending_event_count,
        }
    }

    pub fn poll_observations(&self) -> Vec<SessionRuntimeObservation> {
        let mut observations = Vec::with_capacity(self.authority_commits.len().saturating_add(1));
        observations.push(self.poll_observation());
        observations.extend(
            self.authority_commits
                .iter()
                .map(runtime_observation_from_authority_transaction_commit),
        );
        observations
    }
}

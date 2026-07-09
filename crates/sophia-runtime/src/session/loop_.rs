use super::observation::{
    SessionRuntimeEventBatch, SessionRuntimeObservation, SessionRuntimeObservationError,
};
use super::reducer::update_session_runtime;
use super::types::{SessionRuntimeCommand, SessionRuntimeEvent, SessionRuntimeState};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionRuntimeStepReport {
    pub events_processed: usize,
    pub commands: Vec<SessionRuntimeCommand>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionRuntimeLoop {
    state: SessionRuntimeState,
}

impl SessionRuntimeLoop {
    pub fn new(state: SessionRuntimeState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &SessionRuntimeState {
        &self.state
    }

    pub fn into_state(self) -> SessionRuntimeState {
        self.state
    }

    pub fn step(
        &mut self,
        events: impl IntoIterator<Item = SessionRuntimeEvent>,
    ) -> SessionRuntimeStepReport {
        let mut report = SessionRuntimeStepReport::default();

        for event in events {
            let (state, command) = update_session_runtime(std::mem::take(&mut self.state), event);
            self.state = state;
            report.events_processed += 1;

            if command != SessionRuntimeCommand::None {
                report.commands.push(command);
            }
        }

        report
    }

    pub fn step_observations(
        &mut self,
        observations: impl IntoIterator<Item = SessionRuntimeObservation>,
    ) -> Result<SessionRuntimeStepReport, SessionRuntimeObservationError> {
        let batch = SessionRuntimeEventBatch::from_observations(observations)?;
        Ok(self.step(batch.into_events()))
    }
}

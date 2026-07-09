use super::*;
use crate::SessionRuntimeObservation;

#[derive(Debug)]
pub struct RuntimeAuthoritySupervisor {
    x_authority: ProcessSupervisor,
    generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAuthoritySupervisorReport {
    pub start: Option<SupervisorEvent>,
    pub poll: Option<SupervisorEvent>,
    pub observations: Vec<SessionRuntimeObservation>,
}

impl RuntimeAuthoritySupervisor {
    pub fn new_x_authority(spec: ProcessLaunchSpec) -> Self {
        Self {
            x_authority: ProcessSupervisor::new(SupervisedProcessKind::SophiaXAuthority, spec),
            generation: 0,
        }
    }

    pub fn process(&self) -> SupervisedProcessKind {
        self.x_authority.process()
    }

    pub fn child_id(&self) -> Option<u32> {
        self.x_authority.child_id()
    }

    pub fn start(&mut self) -> Result<RuntimeAuthoritySupervisorReport, ProcessSupervisorError> {
        let start = self.x_authority.apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::SophiaXAuthority,
            delay: Duration::ZERO,
        })?;
        let start_observation = start.and_then(|event| self.observation_from_event(event));
        let poll = self.x_authority.poll()?;
        let poll_observation = poll.and_then(|event| self.observation_from_event(event));

        Ok(RuntimeAuthoritySupervisorReport {
            start,
            poll,
            observations: [start_observation, poll_observation]
                .into_iter()
                .flatten()
                .collect(),
        })
    }

    pub fn poll(
        &mut self,
    ) -> Result<(Option<SupervisorEvent>, Vec<SessionRuntimeObservation>), ProcessSupervisorError>
    {
        let event = self.x_authority.poll()?;
        let observations = self
            .observation_from_optional_event(event)
            .into_iter()
            .collect();
        Ok((event, observations))
    }

    pub fn terminate(&mut self) -> Result<SessionRuntimeObservation, ProcessSupervisorError> {
        self.x_authority.terminate()?;
        Ok(self.health_observation(BrokerHealthState::Stopped))
    }

    fn observation_from_optional_event(
        &mut self,
        event: Option<SupervisorEvent>,
    ) -> Option<SessionRuntimeObservation> {
        event.and_then(|event| self.observation_from_event(event))
    }

    fn observation_from_event(
        &mut self,
        event: SupervisorEvent,
    ) -> Option<SessionRuntimeObservation> {
        match event {
            SupervisorEvent::ProcessStarted | SupervisorEvent::ProcessHealthy => {
                Some(self.health_observation(BrokerHealthState::Ready))
            }
            SupervisorEvent::ProcessExited => {
                Some(self.health_observation(BrokerHealthState::Stopped))
            }
            SupervisorEvent::StartRequested | SupervisorEvent::RestartRequested => None,
        }
    }

    fn health_observation(&mut self, state: BrokerHealthState) -> SessionRuntimeObservation {
        self.generation = self.generation.saturating_add(1);
        SessionRuntimeObservation::AuthorityProcessHealthChanged {
            process: SupervisedProcessKind::SophiaXAuthority,
            state,
            generation: self.generation,
            status_message_len: 0,
        }
    }
}

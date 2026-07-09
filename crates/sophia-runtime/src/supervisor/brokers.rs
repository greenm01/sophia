use super::*;

#[derive(Debug)]
pub struct RuntimeBrokerSupervisors {
    pub portal: ProcessSupervisor,
    pub metadata: ProcessSupervisor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBrokerSupervisorReport {
    pub portal_start: Option<SupervisorEvent>,
    pub metadata_start: Option<SupervisorEvent>,
    pub portal_poll: Option<SupervisorEvent>,
    pub metadata_poll: Option<SupervisorEvent>,
}

impl RuntimeBrokerSupervisors {
    pub fn new(portal_spec: ProcessLaunchSpec, metadata_spec: ProcessLaunchSpec) -> Self {
        Self {
            portal: ProcessSupervisor::new(SupervisedProcessKind::PortalBroker, portal_spec),
            metadata: ProcessSupervisor::new(SupervisedProcessKind::MetadataBroker, metadata_spec),
        }
    }

    pub fn start_placeholders(
        &mut self,
    ) -> Result<RuntimeBrokerSupervisorReport, ProcessSupervisorError> {
        let portal_start = self.portal.apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::PortalBroker,
            delay: Duration::ZERO,
        })?;
        let metadata_start = self.metadata.apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::MetadataBroker,
            delay: Duration::ZERO,
        })?;

        Ok(RuntimeBrokerSupervisorReport {
            portal_start,
            metadata_start,
            portal_poll: self.portal.poll()?,
            metadata_poll: self.metadata.poll()?,
        })
    }

    pub fn poll_all(
        &mut self,
    ) -> Result<(Option<SupervisorEvent>, Option<SupervisorEvent>), ProcessSupervisorError> {
        Ok((self.portal.poll()?, self.metadata.poll()?))
    }

    pub fn terminate_all(&mut self) -> Result<(), ProcessSupervisorError> {
        self.portal.terminate()?;
        self.metadata.terminate()
    }
}

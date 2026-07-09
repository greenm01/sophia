use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartPolicy {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(0),
            max_backoff: Duration::from_secs(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisedProcessKind {
    WindowManager,
    PortalBroker,
    MetadataBroker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisorState {
    pub process: SupervisedProcessKind,
    pub running: bool,
    pub restart_attempts: u32,
}

impl SupervisorState {
    pub const fn new(process: SupervisedProcessKind) -> Self {
        Self {
            process,
            running: false,
            restart_attempts: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorEvent {
    StartRequested,
    RestartRequested,
    ProcessExited,
    ProcessStarted,
    ProcessHealthy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorCommand {
    None,
    StartProcess {
        process: SupervisedProcessKind,
        delay: Duration,
    },
    GiveUp {
        process: SupervisedProcessKind,
    },
}

pub fn update_supervisor(
    mut state: SupervisorState,
    event: SupervisorEvent,
    policy: RestartPolicy,
) -> (SupervisorState, SupervisorCommand) {
    match event {
        SupervisorEvent::StartRequested => {
            state.running = false;
            state.restart_attempts = 0;
            let command = SupervisorCommand::StartProcess {
                process: state.process,
                delay: Duration::ZERO,
            };
            (state, command)
        }
        SupervisorEvent::RestartRequested | SupervisorEvent::ProcessExited => {
            state.running = false;
            match next_restart_delay(state.restart_attempts, policy) {
                Some(delay) => {
                    state.restart_attempts = state.restart_attempts.saturating_add(1);
                    let command = SupervisorCommand::StartProcess {
                        process: state.process,
                        delay,
                    };
                    (state, command)
                }
                None => {
                    let process = state.process;
                    (state, SupervisorCommand::GiveUp { process })
                }
            }
        }
        SupervisorEvent::ProcessStarted => {
            state.running = true;
            (state, SupervisorCommand::None)
        }
        SupervisorEvent::ProcessHealthy => {
            state.running = true;
            state.restart_attempts = 0;
            (state, SupervisorCommand::None)
        }
    }
}

fn next_restart_delay(attempts: u32, policy: RestartPolicy) -> Option<Duration> {
    if attempts >= policy.max_attempts {
        return None;
    }

    if attempts == 0 || policy.initial_backoff.is_zero() {
        return Some(Duration::ZERO);
    }

    let multiplier = 1_u32
        .checked_shl(attempts.saturating_sub(1))
        .unwrap_or(u32::MAX);
    Some(
        policy
            .initial_backoff
            .saturating_mul(multiplier)
            .min(policy.max_backoff),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessLaunchSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl ProcessLaunchSpec {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessSupervisorError {
    WrongProcess {
        expected: SupervisedProcessKind,
        actual: SupervisedProcessKind,
    },
    AlreadyRunning {
        process: SupervisedProcessKind,
    },
    SpawnFailed {
        process: SupervisedProcessKind,
        message: String,
    },
    WaitFailed {
        process: SupervisedProcessKind,
        message: String,
    },
}

impl fmt::Display for ProcessSupervisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProcess { expected, actual } => write!(
                f,
                "supervisor command for {:?} cannot be applied to {:?}",
                actual, expected
            ),
            Self::AlreadyRunning { process } => {
                write!(f, "{process:?} process is already running")
            }
            Self::SpawnFailed { process, message } => {
                write!(f, "failed to spawn {process:?}: {message}")
            }
            Self::WaitFailed { process, message } => {
                write!(f, "failed to wait for {process:?}: {message}")
            }
        }
    }
}

impl std::error::Error for ProcessSupervisorError {}

impl SophiaErrorExt for ProcessSupervisorError {
    fn kind(&self) -> SophiaErrorKind {
        SophiaErrorKind::ExternalProcess
    }
}

#[derive(Debug)]
pub struct ProcessSupervisor {
    process: SupervisedProcessKind,
    spec: ProcessLaunchSpec,
    child: Option<Child>,
}

impl ProcessSupervisor {
    pub fn new(process: SupervisedProcessKind, spec: ProcessLaunchSpec) -> Self {
        Self {
            process,
            spec,
            child: None,
        }
    }

    pub const fn process(&self) -> SupervisedProcessKind {
        self.process
    }

    pub fn child_id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    pub fn apply(
        &mut self,
        command: SupervisorCommand,
    ) -> Result<Option<SupervisorEvent>, ProcessSupervisorError> {
        match command {
            SupervisorCommand::None => Ok(None),
            SupervisorCommand::GiveUp { process } => {
                self.ensure_process(process)?;
                Ok(None)
            }
            SupervisorCommand::StartProcess { process, delay } => {
                self.ensure_process(process)?;
                self.start_after(delay).map(Some)
            }
        }
    }

    pub fn poll(&mut self) -> Result<Option<SupervisorEvent>, ProcessSupervisorError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };

        match child.try_wait() {
            Ok(Some(_status)) => {
                self.child = None;
                Ok(Some(SupervisorEvent::ProcessExited))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(ProcessSupervisorError::WaitFailed {
                process: self.process,
                message: error.to_string(),
            }),
        }
    }

    pub fn terminate(&mut self) -> Result<(), ProcessSupervisorError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };

        if child
            .try_wait()
            .map_err(|error| ProcessSupervisorError::WaitFailed {
                process: self.process,
                message: error.to_string(),
            })?
            .is_none()
        {
            child
                .kill()
                .map_err(|error| ProcessSupervisorError::WaitFailed {
                    process: self.process,
                    message: error.to_string(),
                })?;
        }

        child
            .wait()
            .map_err(|error| ProcessSupervisorError::WaitFailed {
                process: self.process,
                message: error.to_string(),
            })?;
        Ok(())
    }

    fn start_after(&mut self, delay: Duration) -> Result<SupervisorEvent, ProcessSupervisorError> {
        if self.child.is_some() {
            return Err(ProcessSupervisorError::AlreadyRunning {
                process: self.process,
            });
        }

        if !delay.is_zero() {
            std::thread::sleep(delay);
        }

        let mut command = Command::new(&self.spec.program);
        command.args(&self.spec.args);
        let child = command
            .spawn()
            .map_err(|error| ProcessSupervisorError::SpawnFailed {
                process: self.process,
                message: error.to_string(),
            })?;
        self.child = Some(child);
        Ok(SupervisorEvent::ProcessStarted)
    }

    fn ensure_process(&self, process: SupervisedProcessKind) -> Result<(), ProcessSupervisorError> {
        if process == self.process {
            Ok(())
        } else {
            Err(ProcessSupervisorError::WrongProcess {
                expected: self.process,
                actual: process,
            })
        }
    }
}

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

impl Drop for ProcessSupervisor {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

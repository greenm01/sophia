use super::*;

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

impl Drop for ProcessSupervisor {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

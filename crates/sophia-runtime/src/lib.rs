//! Shared runtime conventions for Sophia processes.
//!
//! Libraries emit structured diagnostics through `tracing`; binaries decide
//! when and how to install a subscriber.

use core::fmt;
use std::time::Duration;

use tracing_subscriber::EnvFilter;

pub type SophiaResult<T, E = SophiaError> = Result<T, E>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SophiaErrorKind {
    InvalidOutput,
    InvalidSurface,
    InvalidFrame,
    ExternalProcess,
    RuntimeAlreadyInitialized,
}

pub trait SophiaErrorExt {
    fn kind(&self) -> SophiaErrorKind;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SophiaError {
    kind: SophiaErrorKind,
    message: String,
}

impl SophiaError {
    pub fn new(kind: SophiaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl SophiaErrorExt for SophiaError {
    fn kind(&self) -> SophiaErrorKind {
        self.kind
    }
}

impl fmt::Display for SophiaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SophiaError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceLevel {
    Info,
    Debug,
    Trace,
}

impl TraceLevel {
    const fn filter(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TracingInitError {
    AlreadyInitialized,
}

impl SophiaErrorExt for TracingInitError {
    fn kind(&self) -> SophiaErrorKind {
        SophiaErrorKind::RuntimeAlreadyInitialized
    }
}

impl fmt::Display for TracingInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInitialized => f.write_str("tracing subscriber is already initialized"),
        }
    }
}

impl std::error::Error for TracingInitError {}

pub fn init_tracing(level: TraceLevel) -> Result<(), TracingInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.filter()));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|_| TracingInitError::AlreadyInitialized)
}

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

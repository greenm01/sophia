//! Shared runtime conventions for Sophia processes.
//!
//! Libraries emit structured diagnostics through `tracing`; binaries decide
//! when and how to install a subscriber.

use core::fmt;
use std::ffi::OsString;
use std::process::{Child, Command};
use std::time::Duration;

use sophia_protocol::{
    BrokerHealthState, BrokerKind, SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN, TransactionOutcome,
};
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SessionRuntimePhase {
    #[default]
    Idle,
    PollingX,
    ApplyingWmPolicy,
    WaitingForFrame,
    Rendering,
    DrainingPortals,
    PresentingChrome,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionRuntimeState {
    pub phase: SessionRuntimePhase,
    pub x_events_polled: u64,
    pub frames_rendered: u64,
    pub portal_commands_drained: u64,
    pub chrome_commands_presented: u64,
    pub wm_restart_requests: u64,
    pub authority_transactions_committed: u64,
    pub authority_transactions_rejected: u64,
    pub authority_transactions_timed_out: u64,
    pub authority_surfaces_applied: u64,
    pub slow_client_timeouts: u64,
    pub slow_client_preserved: u64,
    pub slow_client_degraded: u64,
    pub last_frame_serial: Option<u64>,
    pub portal_broker_health: Option<RuntimeBrokerHealth>,
    pub metadata_broker_health: Option<RuntimeBrokerHealth>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBrokerHealth {
    pub state: BrokerHealthState,
    pub generation: u64,
    pub status_message_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionRuntimeEvent {
    TickStarted,
    XEventsPolled {
        count: u32,
    },
    WmLayoutReady,
    WmRestartRequested,
    FrameScheduled {
        frame_serial: u64,
    },
    FrameRendered {
        frame_serial: u64,
    },
    PortalCommandsReady {
        count: u32,
    },
    ChromeCommandsReady {
        count: u32,
    },
    BrokerHealthChanged {
        broker: BrokerKind,
        state: BrokerHealthState,
        generation: u64,
        status_message_len: usize,
    },
    AuthorityTransactionObserved {
        outcome: TransactionOutcome,
        applied_surface_count: u32,
    },
    SlowClientVisualDecisionsObserved {
        timeout_count: u32,
        preserved_count: u32,
        degraded_count: u32,
    },
    TickCompleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRuntimeCommand {
    None,
    PollXEvents,
    RequestWmLayout,
    ScheduleFrame,
    RenderFrame { frame_serial: u64 },
    DrainPortalCommands,
    PresentChrome,
    RestartWindowManager,
}

pub const MAX_SESSION_RUNTIME_OBSERVATION_BATCH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRuntimeObservation {
    TickStarted,
    XEventsPolled {
        count: u32,
    },
    WmLayoutReady,
    WmRestartRequested,
    FrameScheduled {
        frame_serial: u64,
    },
    FrameRendered {
        frame_serial: u64,
    },
    PortalCommandsReady {
        count: u32,
    },
    ChromeCommandsReady {
        count: u32,
    },
    BrokerHealthChanged {
        broker: BrokerKind,
        state: BrokerHealthState,
        generation: u64,
        status_message_len: usize,
    },
    AuthorityTransactionObserved {
        outcome: TransactionOutcome,
        applied_surface_count: u32,
    },
    SlowClientVisualDecisionsObserved {
        timeout_count: u32,
        preserved_count: u32,
        degraded_count: u32,
    },
    TickCompleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionRuntimeObservationError {
    TooManyObservations { max: usize },
    BrokerStatusMessageTooLong { len: usize, max: usize },
}

impl SophiaErrorExt for SessionRuntimeObservationError {
    fn kind(&self) -> SophiaErrorKind {
        SophiaErrorKind::InvalidFrame
    }
}

impl fmt::Display for SessionRuntimeObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyObservations { max } => {
                write!(f, "session runtime observation batch exceeds {max} events")
            }
            Self::BrokerStatusMessageTooLong { len, max } => write!(
                f,
                "broker status message length {len} exceeds runtime observation limit {max}"
            ),
        }
    }
}

impl std::error::Error for SessionRuntimeObservationError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionRuntimeEventBatch {
    events: Vec<SessionRuntimeEvent>,
}

impl SessionRuntimeEventBatch {
    pub fn from_observations(
        observations: impl IntoIterator<Item = SessionRuntimeObservation>,
    ) -> Result<Self, SessionRuntimeObservationError> {
        let mut events = Vec::new();

        for observation in observations {
            if events.len() >= MAX_SESSION_RUNTIME_OBSERVATION_BATCH {
                return Err(SessionRuntimeObservationError::TooManyObservations {
                    max: MAX_SESSION_RUNTIME_OBSERVATION_BATCH,
                });
            }

            events.push(session_runtime_event_from_observation(observation)?);
        }

        Ok(Self { events })
    }

    pub fn events(&self) -> &[SessionRuntimeEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<SessionRuntimeEvent> {
        self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

pub fn session_runtime_event_from_observation(
    observation: SessionRuntimeObservation,
) -> Result<SessionRuntimeEvent, SessionRuntimeObservationError> {
    match observation {
        SessionRuntimeObservation::TickStarted => Ok(SessionRuntimeEvent::TickStarted),
        SessionRuntimeObservation::XEventsPolled { count } => {
            Ok(SessionRuntimeEvent::XEventsPolled { count })
        }
        SessionRuntimeObservation::WmLayoutReady => Ok(SessionRuntimeEvent::WmLayoutReady),
        SessionRuntimeObservation::WmRestartRequested => {
            Ok(SessionRuntimeEvent::WmRestartRequested)
        }
        SessionRuntimeObservation::FrameScheduled { frame_serial } => {
            Ok(SessionRuntimeEvent::FrameScheduled { frame_serial })
        }
        SessionRuntimeObservation::FrameRendered { frame_serial } => {
            Ok(SessionRuntimeEvent::FrameRendered { frame_serial })
        }
        SessionRuntimeObservation::PortalCommandsReady { count } => {
            Ok(SessionRuntimeEvent::PortalCommandsReady { count })
        }
        SessionRuntimeObservation::ChromeCommandsReady { count } => {
            Ok(SessionRuntimeEvent::ChromeCommandsReady { count })
        }
        SessionRuntimeObservation::BrokerHealthChanged {
            broker,
            state,
            generation,
            status_message_len,
        } => {
            if status_message_len > SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN {
                return Err(SessionRuntimeObservationError::BrokerStatusMessageTooLong {
                    len: status_message_len,
                    max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
                });
            }

            Ok(SessionRuntimeEvent::BrokerHealthChanged {
                broker,
                state,
                generation,
                status_message_len,
            })
        }
        SessionRuntimeObservation::AuthorityTransactionObserved {
            outcome,
            applied_surface_count,
        } => Ok(SessionRuntimeEvent::AuthorityTransactionObserved {
            outcome,
            applied_surface_count,
        }),
        SessionRuntimeObservation::SlowClientVisualDecisionsObserved {
            timeout_count,
            preserved_count,
            degraded_count,
        } => Ok(SessionRuntimeEvent::SlowClientVisualDecisionsObserved {
            timeout_count,
            preserved_count,
            degraded_count,
        }),
        SessionRuntimeObservation::TickCompleted => Ok(SessionRuntimeEvent::TickCompleted),
    }
}

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

pub fn update_session_runtime(
    mut state: SessionRuntimeState,
    event: SessionRuntimeEvent,
) -> (SessionRuntimeState, SessionRuntimeCommand) {
    let command = match event {
        SessionRuntimeEvent::TickStarted => {
            state.phase = SessionRuntimePhase::PollingX;
            SessionRuntimeCommand::PollXEvents
        }
        SessionRuntimeEvent::XEventsPolled { count } => {
            state.x_events_polled = state.x_events_polled.saturating_add(u64::from(count));
            if count == 0 {
                state.phase = SessionRuntimePhase::WaitingForFrame;
                SessionRuntimeCommand::ScheduleFrame
            } else {
                state.phase = SessionRuntimePhase::ApplyingWmPolicy;
                SessionRuntimeCommand::RequestWmLayout
            }
        }
        SessionRuntimeEvent::WmLayoutReady => {
            state.phase = SessionRuntimePhase::WaitingForFrame;
            SessionRuntimeCommand::ScheduleFrame
        }
        SessionRuntimeEvent::WmRestartRequested => {
            state.wm_restart_requests = state.wm_restart_requests.saturating_add(1);
            state.phase = SessionRuntimePhase::ApplyingWmPolicy;
            SessionRuntimeCommand::RestartWindowManager
        }
        SessionRuntimeEvent::FrameScheduled { frame_serial } => {
            state.phase = SessionRuntimePhase::Rendering;
            SessionRuntimeCommand::RenderFrame { frame_serial }
        }
        SessionRuntimeEvent::FrameRendered { frame_serial } => {
            state.frames_rendered = state.frames_rendered.saturating_add(1);
            state.last_frame_serial = Some(frame_serial);
            state.phase = SessionRuntimePhase::DrainingPortals;
            SessionRuntimeCommand::DrainPortalCommands
        }
        SessionRuntimeEvent::PortalCommandsReady { count } => {
            state.portal_commands_drained = state
                .portal_commands_drained
                .saturating_add(u64::from(count));
            state.phase = SessionRuntimePhase::PresentingChrome;
            SessionRuntimeCommand::PresentChrome
        }
        SessionRuntimeEvent::ChromeCommandsReady { count } => {
            state.chrome_commands_presented = state
                .chrome_commands_presented
                .saturating_add(u64::from(count));
            state.phase = SessionRuntimePhase::Idle;
            SessionRuntimeCommand::None
        }
        SessionRuntimeEvent::BrokerHealthChanged {
            broker,
            state: broker_state,
            generation,
            status_message_len,
        } => {
            let health = RuntimeBrokerHealth {
                state: broker_state,
                generation,
                status_message_len,
            };
            match broker {
                BrokerKind::Portal => {
                    if accepts_broker_health(state.portal_broker_health, generation) {
                        state.portal_broker_health = Some(health);
                    }
                }
                BrokerKind::Metadata => {
                    if accepts_broker_health(state.metadata_broker_health, generation) {
                        state.metadata_broker_health = Some(health);
                    }
                }
            }
            SessionRuntimeCommand::None
        }
        SessionRuntimeEvent::AuthorityTransactionObserved {
            outcome,
            applied_surface_count,
        } => {
            state.authority_surfaces_applied = state
                .authority_surfaces_applied
                .saturating_add(u64::from(applied_surface_count));
            match outcome {
                TransactionOutcome::Committed => {
                    state.authority_transactions_committed =
                        state.authority_transactions_committed.saturating_add(1);
                }
                TransactionOutcome::TimedOut => {
                    state.authority_transactions_timed_out =
                        state.authority_transactions_timed_out.saturating_add(1);
                }
                TransactionOutcome::RejectedStaleSurface
                | TransactionOutcome::RejectedInvalidSurface => {
                    state.authority_transactions_rejected =
                        state.authority_transactions_rejected.saturating_add(1);
                }
            }
            SessionRuntimeCommand::None
        }
        SessionRuntimeEvent::SlowClientVisualDecisionsObserved {
            timeout_count,
            preserved_count,
            degraded_count,
        } => {
            state.slow_client_timeouts = state
                .slow_client_timeouts
                .saturating_add(u64::from(timeout_count));
            state.slow_client_preserved = state
                .slow_client_preserved
                .saturating_add(u64::from(preserved_count));
            state.slow_client_degraded = state
                .slow_client_degraded
                .saturating_add(u64::from(degraded_count));
            SessionRuntimeCommand::None
        }
        SessionRuntimeEvent::TickCompleted => {
            state.phase = SessionRuntimePhase::Idle;
            SessionRuntimeCommand::None
        }
    };

    (state, command)
}

fn accepts_broker_health(current: Option<RuntimeBrokerHealth>, generation: u64) -> bool {
    current.is_none_or(|health| generation >= health.generation)
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

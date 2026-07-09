use crate::prelude::*;

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

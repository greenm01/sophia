use crate::prelude::*;

use super::types::SessionRuntimeEvent;

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

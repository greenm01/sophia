use crate::prelude::*;

use super::types::{
    RuntimeAuthorityHealth, RuntimeBrokerHealth, SessionRuntimeCommand, SessionRuntimeEvent,
    SessionRuntimePhase, SessionRuntimeState,
};

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
        SessionRuntimeEvent::AuthorityProcessHealthChanged {
            process,
            state: authority_state,
            generation,
            status_message_len,
        } => {
            if process == SupervisedProcessKind::SophiaXAuthority
                && accepts_authority_health(state.x_authority_health, generation)
            {
                state.x_authority_health = Some(RuntimeAuthorityHealth {
                    process,
                    state: authority_state,
                    generation,
                    status_message_len,
                });
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

fn accepts_authority_health(current: Option<RuntimeAuthorityHealth>, generation: u64) -> bool {
    current.is_none_or(|health| generation >= health.generation)
}

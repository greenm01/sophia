use std::time::Duration;

use sophia_protocol::{BrokerHealthState, BrokerKind};
use sophia_runtime::{
    ProcessLaunchSpec, ProcessSupervisor, ProcessSupervisorError, RestartPolicy,
    RuntimeBrokerHealth, RuntimeBrokerSupervisors, SessionRuntimeCommand, SessionRuntimeEvent,
    SessionRuntimePhase, SessionRuntimeState, SupervisedProcessKind, SupervisorCommand,
    SupervisorEvent, SupervisorState, update_session_runtime, update_supervisor,
};

#[test]
fn session_runtime_reducer_runs_one_continuous_tick() {
    let state = SessionRuntimeState::default();

    let (state, command) = update_session_runtime(state, SessionRuntimeEvent::TickStarted);
    assert_eq!(state.phase, SessionRuntimePhase::PollingX);
    assert_eq!(command, SessionRuntimeCommand::PollXEvents);

    let (state, command) =
        update_session_runtime(state, SessionRuntimeEvent::XEventsPolled { count: 3 });
    assert_eq!(state.x_events_polled, 3);
    assert_eq!(state.phase, SessionRuntimePhase::ApplyingWmPolicy);
    assert_eq!(command, SessionRuntimeCommand::RequestWmLayout);

    let (state, command) = update_session_runtime(state, SessionRuntimeEvent::WmLayoutReady);
    assert_eq!(state.phase, SessionRuntimePhase::WaitingForFrame);
    assert_eq!(command, SessionRuntimeCommand::ScheduleFrame);

    let (state, command) = update_session_runtime(
        state,
        SessionRuntimeEvent::FrameScheduled { frame_serial: 9 },
    );
    assert_eq!(state.phase, SessionRuntimePhase::Rendering);
    assert_eq!(
        command,
        SessionRuntimeCommand::RenderFrame { frame_serial: 9 }
    );

    let (state, command) = update_session_runtime(
        state,
        SessionRuntimeEvent::FrameRendered { frame_serial: 9 },
    );
    assert_eq!(state.frames_rendered, 1);
    assert_eq!(state.last_frame_serial, Some(9));
    assert_eq!(state.phase, SessionRuntimePhase::DrainingPortals);
    assert_eq!(command, SessionRuntimeCommand::DrainPortalCommands);

    let (state, command) =
        update_session_runtime(state, SessionRuntimeEvent::PortalCommandsReady { count: 2 });
    assert_eq!(state.portal_commands_drained, 2);
    assert_eq!(state.phase, SessionRuntimePhase::PresentingChrome);
    assert_eq!(command, SessionRuntimeCommand::PresentChrome);

    let (state, command) =
        update_session_runtime(state, SessionRuntimeEvent::ChromeCommandsReady { count: 1 });
    assert_eq!(state.chrome_commands_presented, 1);
    assert_eq!(state.phase, SessionRuntimePhase::Idle);
    assert_eq!(command, SessionRuntimeCommand::None);
}

#[test]
fn session_runtime_reducer_skips_wm_policy_when_no_x_events_arrive() {
    let state = SessionRuntimeState::default();
    let (state, _command) = update_session_runtime(state, SessionRuntimeEvent::TickStarted);

    let (state, command) =
        update_session_runtime(state, SessionRuntimeEvent::XEventsPolled { count: 0 });

    assert_eq!(state.x_events_polled, 0);
    assert_eq!(state.phase, SessionRuntimePhase::WaitingForFrame);
    assert_eq!(command, SessionRuntimeCommand::ScheduleFrame);
}

#[test]
fn session_runtime_reducer_requests_wm_restart_without_rendering() {
    let state = SessionRuntimeState::default();

    let (state, command) = update_session_runtime(state, SessionRuntimeEvent::WmRestartRequested);

    assert_eq!(state.wm_restart_requests, 1);
    assert_eq!(state.frames_rendered, 0);
    assert_eq!(state.phase, SessionRuntimePhase::ApplyingWmPolicy);
    assert_eq!(command, SessionRuntimeCommand::RestartWindowManager);
}

#[test]
fn session_runtime_records_broker_health_without_status_payload() {
    let state = SessionRuntimeState::default();

    let (state, command) = update_session_runtime(
        state,
        SessionRuntimeEvent::BrokerHealthChanged {
            broker: BrokerKind::Portal,
            state: BrokerHealthState::Ready,
            generation: 7,
            status_message_len: 17,
        },
    );

    assert_eq!(command, SessionRuntimeCommand::None);
    assert_eq!(
        state.portal_broker_health,
        Some(RuntimeBrokerHealth {
            state: BrokerHealthState::Ready,
            generation: 7,
            status_message_len: 17,
        })
    );
    assert_eq!(state.metadata_broker_health, None);

    let (state, _command) = update_session_runtime(
        state,
        SessionRuntimeEvent::BrokerHealthChanged {
            broker: BrokerKind::Portal,
            state: BrokerHealthState::Stopped,
            generation: 6,
            status_message_len: 0,
        },
    );

    assert_eq!(
        state.portal_broker_health,
        Some(RuntimeBrokerHealth {
            state: BrokerHealthState::Ready,
            generation: 7,
            status_message_len: 17,
        })
    );

    let (state, _command) = update_session_runtime(
        state,
        SessionRuntimeEvent::BrokerHealthChanged {
            broker: BrokerKind::Metadata,
            state: BrokerHealthState::Degraded,
            generation: 2,
            status_message_len: 8,
        },
    );

    assert_eq!(
        state.metadata_broker_health,
        Some(RuntimeBrokerHealth {
            state: BrokerHealthState::Degraded,
            generation: 2,
            status_message_len: 8,
        })
    );
}

#[test]
fn supervisor_start_request_emits_immediate_start_without_consuming_restart_budget() {
    let state = SupervisorState::new(SupervisedProcessKind::WindowManager);

    let (state, command) = update_supervisor(
        state,
        SupervisorEvent::StartRequested,
        RestartPolicy::default(),
    );

    assert_eq!(state.restart_attempts, 0);
    assert!(!state.running);
    assert_eq!(
        command,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::ZERO
        }
    );
}

#[test]
fn supervisor_restart_request_consumes_budget_and_applies_backoff() {
    let policy = RestartPolicy {
        max_attempts: 4,
        initial_backoff: Duration::from_millis(25),
        max_backoff: Duration::from_millis(60),
    };
    let state = SupervisorState::new(SupervisedProcessKind::WindowManager);

    let (state, first) = update_supervisor(state, SupervisorEvent::RestartRequested, policy);
    let (state, second) = update_supervisor(state, SupervisorEvent::RestartRequested, policy);
    let (state, third) = update_supervisor(state, SupervisorEvent::RestartRequested, policy);
    let (state, fourth) = update_supervisor(state, SupervisorEvent::RestartRequested, policy);

    assert_eq!(state.restart_attempts, 4);
    assert_eq!(
        first,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::ZERO
        }
    );
    assert_eq!(
        second,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::from_millis(25)
        }
    );
    assert_eq!(
        third,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::from_millis(50)
        }
    );
    assert_eq!(
        fourth,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::from_millis(60)
        }
    );
}

#[test]
fn supervisor_gives_up_after_restart_budget_is_exhausted() {
    let policy = RestartPolicy {
        max_attempts: 1,
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(100),
    };
    let state = SupervisorState::new(SupervisedProcessKind::PortalBroker);

    let (state, first) = update_supervisor(state, SupervisorEvent::ProcessExited, policy);
    let (_state, second) = update_supervisor(state, SupervisorEvent::ProcessExited, policy);

    assert_eq!(
        first,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::PortalBroker,
            delay: Duration::ZERO
        }
    );
    assert_eq!(
        second,
        SupervisorCommand::GiveUp {
            process: SupervisedProcessKind::PortalBroker
        }
    );
}

#[test]
fn supervisor_healthy_event_resets_restart_budget() {
    let policy = RestartPolicy {
        max_attempts: 2,
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(100),
    };
    let state = SupervisorState::new(SupervisedProcessKind::MetadataBroker);
    let (state, _command) = update_supervisor(state, SupervisorEvent::RestartRequested, policy);

    let (state, command) = update_supervisor(state, SupervisorEvent::ProcessHealthy, policy);

    assert!(state.running);
    assert_eq!(state.restart_attempts, 0);
    assert_eq!(command, SupervisorCommand::None);
}

#[test]
fn process_supervisor_spawns_and_observes_process_exit() {
    let mut supervisor = ProcessSupervisor::new(
        SupervisedProcessKind::WindowManager,
        ProcessLaunchSpec::new("/usr/bin/true"),
    );

    let event = supervisor
        .apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::ZERO,
        })
        .unwrap();

    assert_eq!(event, Some(SupervisorEvent::ProcessStarted));
    assert!(supervisor.child_id().is_some());

    let mut observed = None;
    for _ in 0..100 {
        observed = supervisor.poll().unwrap();
        if observed.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(observed, Some(SupervisorEvent::ProcessExited));
    assert_eq!(supervisor.child_id(), None);
}

#[test]
fn runtime_broker_supervisors_start_and_observe_placeholder_exits() {
    let mut supervisors = RuntimeBrokerSupervisors::new(
        ProcessLaunchSpec::new("/usr/bin/true"),
        ProcessLaunchSpec::new("/usr/bin/true"),
    );

    let report = supervisors.start_placeholders().unwrap();

    assert_eq!(report.portal_start, Some(SupervisorEvent::ProcessStarted));
    assert_eq!(report.metadata_start, Some(SupervisorEvent::ProcessStarted));

    let mut portal_exit = report.portal_poll;
    let mut metadata_exit = report.metadata_poll;
    for _ in 0..100 {
        if portal_exit == Some(SupervisorEvent::ProcessExited)
            && metadata_exit == Some(SupervisorEvent::ProcessExited)
        {
            break;
        }
        let (portal, metadata) = supervisors.poll_all().unwrap();
        portal_exit = portal_exit.or(portal);
        metadata_exit = metadata_exit.or(metadata);
        std::thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(portal_exit, Some(SupervisorEvent::ProcessExited));
    assert_eq!(metadata_exit, Some(SupervisorEvent::ProcessExited));
    assert_eq!(supervisors.portal.child_id(), None);
    assert_eq!(supervisors.metadata.child_id(), None);
}

#[test]
fn process_supervisor_rejects_wrong_process_command() {
    let mut supervisor = ProcessSupervisor::new(
        SupervisedProcessKind::WindowManager,
        ProcessLaunchSpec::new("/usr/bin/true"),
    );

    let error = supervisor
        .apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::PortalBroker,
            delay: Duration::ZERO,
        })
        .unwrap_err();

    assert_eq!(
        error,
        ProcessSupervisorError::WrongProcess {
            expected: SupervisedProcessKind::WindowManager,
            actual: SupervisedProcessKind::PortalBroker
        }
    );
}

#[test]
fn process_supervisor_rejects_start_while_child_is_running() {
    let mut supervisor = ProcessSupervisor::new(
        SupervisedProcessKind::WindowManager,
        ProcessLaunchSpec::new("/usr/bin/sleep").arg("1"),
    );

    supervisor
        .apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::ZERO,
        })
        .unwrap();
    let error = supervisor
        .apply(SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::ZERO,
        })
        .unwrap_err();

    assert_eq!(
        error,
        ProcessSupervisorError::AlreadyRunning {
            process: SupervisedProcessKind::WindowManager
        }
    );

    supervisor.terminate().unwrap();
    assert_eq!(supervisor.child_id(), None);
}

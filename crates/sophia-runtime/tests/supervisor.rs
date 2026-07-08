use std::time::Duration;

use sophia_runtime::{
    RestartPolicy, SupervisedProcessKind, SupervisorCommand, SupervisorEvent, SupervisorState,
    update_supervisor,
};

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

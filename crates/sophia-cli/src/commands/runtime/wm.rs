use super::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "wm-supervisor-smoke") {
        let wm_path = arg_value(args, "--wm")
            .or_else(|| std::env::var("SOPHIA_WM_DEMO").ok())
            .unwrap_or_else(|| "target/debug/sophia-wm-demo".to_owned());
        let socket_path = std::env::temp_dir().join(format!(
            "sophia-wm-{}-{}.sock",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let socket_arg = format!("--socket={}", socket_path.display());
        let mut supervisor = ProcessSupervisor::new(
            SupervisedProcessKind::WindowManager,
            ProcessLaunchSpec::new(&wm_path)
                .arg("serve-socket")
                .arg(socket_arg),
        );
        let policy = RestartPolicy {
            max_attempts: 2,
            initial_backoff: Duration::ZERO,
            max_backoff: Duration::ZERO,
        };
        let mut state = sophia_runtime::SupervisorState::new(SupervisedProcessKind::WindowManager);
        let (next_state, command) =
            update_supervisor(state, SupervisorEvent::StartRequested, policy);
        state = next_state;
        let start_event = supervisor
            .apply(command)?
            .ok_or("supervisor did not start WM process")?;
        let (next_state, _) = update_supervisor(state, start_event, policy);
        state = next_state;
        let first_pid = supervisor.child_id().ok_or("missing first WM pid")?;
        wait_for_socket(&socket_path)?;
        let first = request_supervised_wm(&socket_path, TransactionId::from_raw(21))?;

        supervisor.terminate()?;
        let (next_state, restart_command) =
            update_supervisor(state, SupervisorEvent::ProcessExited, policy);
        state = next_state;
        let restart_event = supervisor
            .apply(restart_command)?
            .ok_or("supervisor did not restart WM process")?;
        let (next_state, _) = update_supervisor(state, restart_event, policy);
        state = next_state;
        let second_pid = supervisor.child_id().ok_or("missing restarted WM pid")?;
        wait_for_socket(&socket_path)?;
        let second = request_supervised_wm(&socket_path, TransactionId::from_raw(22))?;

        if first_pid == second_pid {
            return Err("WM supervisor did not restart into a new process".into());
        }
        if first.outcome != sophia_protocol::TransactionOutcome::Committed {
            return Err(format!(
                "first supervised WM transaction did not commit: {:?}",
                first.outcome
            )
            .into());
        }
        if second.outcome != sophia_protocol::TransactionOutcome::Committed {
            return Err(format!(
                "second supervised WM transaction did not commit: {:?}",
                second.outcome
            )
            .into());
        }

        supervisor.terminate()?;
        let _ = std::fs::remove_file(&socket_path);

        println!(
            "wm-supervisor-smoke wm={} socket={} first_pid={} second_pid={} restarted={} running={} restart_attempts={} first_outcome={:?} second_outcome={:?} first_commands={} second_commands={}",
            wm_path,
            socket_path.display(),
            first_pid,
            second_pid,
            first_pid != second_pid,
            state.running,
            state.restart_attempts,
            first.outcome,
            second.outcome,
            first.commands,
            second.commands,
        );
        return Ok(true);
    }

    Ok(false)
}

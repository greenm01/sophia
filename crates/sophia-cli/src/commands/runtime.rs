use super::prelude::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "runtime-damage-epoch-smoke") {
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let surface = SurfaceId::new(1, 1);
        let frame_serial = 5;
        let mut epoch = LayoutEpochState::with_timing(1, [surface], 0, 300);
        let damage = DamageFrame {
            output: output.id,
            frame_serial,
            buffer_age: 1,
            root_generation: 1,
            affected_surfaces: vec![surface],
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }),
        };
        let tick = FrameClockTick {
            output: output.id,
            frame_serial,
            target_msec: 80,
        };
        let decision = schedule_frame_from_damage(tick, Some(damage), Some(&mut epoch));
        let (scheduled_frame_serial, completed_epoch) = match decision {
            FrameScheduleDecision::Render {
                frame_serial,
                completed_epoch,
                ..
            } => (frame_serial, completed_epoch),
            other => {
                return Err(std::io::Error::other(format!(
                    "expected render decision, got {other:?}"
                ))
                .into());
            }
        };

        let mut driver = HeadlessSessionDriver::new(engine);
        let report = driver.run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: scheduled_frame_serial,
            x_event_count: 1,
            layers: synthetic_layers(),
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 0,
        })?;

        println!(
            "runtime-damage-epoch-smoke output={} frame_serial={} completed_epoch={:?} pending_surfaces={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={}",
            output.id.raw(),
            scheduled_frame_serial,
            completed_epoch,
            epoch.pending_surfaces().len(),
            report.runtime_state.phase,
            report.runtime_commands.len(),
            report.runtime_state.frames_rendered,
            report.runtime_state.x_events_polled
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "headless-session-driver-smoke")
    {
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let mut driver = HeadlessSessionDriver::new(engine);
        let transaction = TransactionId::from_raw(30);
        let report = driver.run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: 6,
            x_event_count: 1,
            layers: synthetic_layers(),
            wm_update: Some(WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![SurfaceId::new(1, 1)],
                },
                ipc_error: None,
            }),
            portal_commands: vec![PortalCommand::DropNotification {
                transfer: PortalTransferId::from_raw(1),
            }],
            chrome_command_count: 1,
        })?;
        let session_tick = report
            .session_tick
            .as_ref()
            .ok_or("headless session driver did not render a frame")?;

        println!(
            "headless-session-driver-smoke output={} frame_serial={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={} runtime_portal={} runtime_chrome={} cached_layers={} frame_layers={} replay_steps={}",
            output.id.raw(),
            session_tick.frame.frame_serial,
            report.runtime_state.phase,
            report.runtime_commands.len(),
            report.runtime_state.frames_rendered,
            report.runtime_state.x_events_polled,
            report.runtime_state.portal_commands_drained,
            report.runtime_state.chrome_commands_presented,
            report.cached_layers,
            session_tick.frame.layers.len(),
            session_tick.replay.steps.len()
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "runtime-brokers-smoke") {
        let portal = arg_value(args, "--portal").unwrap_or_else(|| "/usr/bin/true".to_owned());
        let metadata = arg_value(args, "--metadata").unwrap_or_else(|| "/usr/bin/true".to_owned());
        let mut supervisors = RuntimeBrokerSupervisors::new(
            ProcessLaunchSpec::new(&portal),
            ProcessLaunchSpec::new(&metadata),
        );
        let report = supervisors.start_placeholders()?;
        let mut portal_exit = report.portal_poll;
        let mut metadata_exit = report.metadata_poll;

        for _ in 0..100 {
            if portal_exit == Some(SupervisorEvent::ProcessExited)
                && metadata_exit == Some(SupervisorEvent::ProcessExited)
            {
                break;
            }
            let (portal_event, metadata_event) = supervisors.poll_all()?;
            portal_exit = portal_exit.or(portal_event);
            metadata_exit = metadata_exit.or(metadata_event);
            std::thread::sleep(Duration::from_millis(1));
        }
        supervisors.terminate_all()?;

        println!(
            "runtime-brokers-smoke portal={} metadata={} portal_start={:?} metadata_start={:?} portal_exit={:?} metadata_exit={:?}",
            portal,
            metadata,
            report.portal_start,
            report.metadata_start,
            portal_exit,
            metadata_exit
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "portal-broker-health-smoke") {
        let packet = BrokerHealthPacket::new(
            BrokerKind::Portal,
            BrokerHealthState::Ready,
            1,
            Some("placeholder ready".to_owned()),
        )
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let frame = encode_broker_health_frame(&packet)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let decoded = decode_broker_health_frame(&frame)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let message_len = decoded.message.as_deref().map(str::len).unwrap_or(0);
        let mut runtime = SessionRuntimeLoop::default();
        let runtime_report =
            runtime.step_observations([SessionRuntimeObservation::BrokerHealthChanged {
                broker: decoded.broker,
                state: decoded.state,
                generation: decoded.generation,
                status_message_len: message_len,
            }])?;

        println!(
            "portal-broker-health-smoke broker={:?} state={:?} generation={} message_len={} frame_bytes={} runtime_health={:?} runtime_command={:?}",
            decoded.broker,
            decoded.state,
            decoded.generation,
            message_len,
            frame.len(),
            runtime.state().portal_broker_health,
            runtime_report
                .commands
                .first()
                .copied()
                .unwrap_or(SessionRuntimeCommand::None)
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "metadata-broker-health-smoke") {
        let packet = BrokerHealthPacket::new(
            BrokerKind::Metadata,
            BrokerHealthState::Ready,
            1,
            Some("placeholder ready".to_owned()),
        )
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let frame = encode_broker_health_frame(&packet)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let decoded = decode_broker_health_frame(&frame)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let message_len = decoded.message.as_deref().map(str::len).unwrap_or(0);
        let mut runtime = SessionRuntimeLoop::default();
        let runtime_report =
            runtime.step_observations([SessionRuntimeObservation::BrokerHealthChanged {
                broker: decoded.broker,
                state: decoded.state,
                generation: decoded.generation,
                status_message_len: message_len,
            }])?;

        println!(
            "metadata-broker-health-smoke broker={:?} state={:?} generation={} message_len={} frame_bytes={} runtime_health={:?} runtime_command={:?}",
            decoded.broker,
            decoded.state,
            decoded.generation,
            message_len,
            frame.len(),
            runtime.state().metadata_broker_health,
            runtime_report
                .commands
                .first()
                .copied()
                .unwrap_or(SessionRuntimeCommand::None)
        );
        return Ok(true);
    }

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

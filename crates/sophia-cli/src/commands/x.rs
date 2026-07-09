use super::prelude::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "x-test-client") {
        let config = TestClientConfig {
            display_name: arg_value(args, "--display"),
            size: Size {
                width: arg_value(args, "--width")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(320),
                height: arg_value(args, "--height")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(200),
            },
            hold_millis: arg_value(args, "--seconds")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(5)
                .saturating_mul(1_000),
        };
        let window = run_test_client_window(config)?;
        println!(
            "x-test-client window={:#x} size={}x{}",
            window.window.xid(),
            window.size.width,
            window.size.height
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-smoke-readback") {
        let display = arg_value(args, "--display");
        let report = capture_readback_display(display.as_deref())?.report;
        println!(
            "x-smoke-readback display={} windows={} surfaces={} layers={} targets={} readbacks={} bytes={}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.mirrored_windows,
            report.surfaces,
            report.renderable_layers,
            report.redirect_targets,
            report.readbacks,
            report.total_bytes
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-smoke-frame") {
        let display = arg_value(args, "--display");
        let capture = capture_readback_display(display.as_deref())?;
        let engine = HeadlessEngine::default();
        let frame = engine.plan_frame(
            FramePlanRequest {
                output: engine.output().id,
                frame_serial: 1,
            },
            capture.layers,
        )?;
        let replay = engine.replay_frame(&frame)?;
        println!(
            "x-smoke-frame display={} windows={} surfaces={} layers={} readbacks={} bytes={} commands={} replay_steps={} damage_rects={}",
            capture
                .report
                .display_name
                .as_deref()
                .unwrap_or("<default>"),
            capture.report.mirrored_windows,
            capture.report.surfaces,
            capture.report.renderable_layers,
            capture.report.readbacks,
            capture.report.total_bytes,
            frame.commands.len(),
            replay.steps.len(),
            replay.damage.rects.len()
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-smoke-policy-frame") {
        let display = arg_value(args, "--display");
        let capture = capture_readback_display(display.as_deref())?;
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let workspace = WorkspaceId::from_raw(1);
        let nodes = capture
            .layers
            .iter()
            .map(|layer| LayoutNodeSnapshot {
                surface: layer.surface,
                workspace,
                kind: LayoutNodeKind::Toplevel,
                capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
                state: LayoutNodeState::NORMAL,
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                geometry: layer.geometry,
                generation: layer.generation,
            })
            .collect::<Vec<_>>();
        let transaction = tile_workspace(
            TransactionId::from_raw(1),
            workspace,
            Rect {
                x: 0,
                y: 0,
                width: output.size.width,
                height: output.size.height,
            },
            &nodes,
        );
        let layers = engine.apply_layout_transaction(&transaction, capture.layers)?;
        let frame = engine.plan_frame(
            FramePlanRequest {
                output: output.id,
                frame_serial: 2,
            },
            layers,
        )?;
        let replay = engine.replay_frame(&frame)?;
        println!(
            "x-smoke-policy-frame display={} windows={} surfaces={} placements={} focus={} commands={} replay_steps={} damage_rects={}",
            capture
                .report
                .display_name
                .as_deref()
                .unwrap_or("<default>"),
            capture.report.mirrored_windows,
            capture.report.surfaces,
            transaction.render_positions.len(),
            transaction.focus.is_some(),
            frame.commands.len(),
            replay.steps.len(),
            replay.damage.rects.len()
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-smoke-runtime-tick") {
        let display = arg_value(args, "--display");
        let capture = capture_readback_display(display.as_deref())?;
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let frame_serial = 4;
        let mut driver = HeadlessSessionDriver::new(engine);
        let report = driver.run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial,
            x_event_count: u32::try_from(capture.report.mirrored_windows).unwrap_or(u32::MAX),
            layers: capture.layers,
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 0,
        })?;
        let session_tick = report
            .session_tick
            .as_ref()
            .ok_or("runtime tick did not render a frame")?;

        println!(
            "x-smoke-runtime-tick display={} windows={} surfaces={} layers={} readbacks={} bytes={} restored={} commands={} replay_steps={} damage_rects={} cached_layers={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={} runtime_portal={} runtime_chrome={}",
            capture
                .report
                .display_name
                .as_deref()
                .unwrap_or("<default>"),
            capture.report.mirrored_windows,
            capture.report.surfaces,
            session_tick.frame.layers.len(),
            capture.report.readbacks,
            capture.report.total_bytes,
            session_tick.restored_last_committed,
            session_tick.frame.commands.len(),
            session_tick.replay.steps.len(),
            session_tick.replay.damage.rects.len(),
            report.cached_layers,
            report.runtime_state.phase,
            report.runtime_commands.len(),
            report.runtime_state.frames_rendered,
            report.runtime_state.x_events_polled,
            report.runtime_state.portal_commands_drained,
            report.runtime_state.chrome_commands_presented
        );
        return Ok(true);
    }
    if args.iter().any(|arg| arg == "x-smoke-external-wm") {
        let display = arg_value(args, "--display");
        let wm_path = arg_value(args, "--wm")
            .or_else(|| std::env::var("SOPHIA_WM_DEMO").ok())
            .unwrap_or_else(|| "target/debug/sophia-wm-demo".to_owned());
        let capture = capture_readback_display(display.as_deref())?;
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let workspace = WorkspaceId::from_raw(1);
        let nodes = layout_nodes_from_layers(&capture.layers, workspace);
        let request = sophia_protocol::WmRequestPacket {
            transaction: TransactionId::from_raw(3),
            kind: sophia_protocol::WmRequestKind::RelayoutWorkspace(
                sophia_protocol::WmRelayoutWorkspace {
                    output: output.id,
                    workspace,
                    bounds: Rect {
                        x: 0,
                        y: 0,
                        width: output.size.width,
                        height: output.size.height,
                    },
                    nodes,
                },
            ),
        };
        let response = ExternalWmClient::new(&wm_path).request(&request)?;
        let transaction = response.into_layout_transaction();
        let mut layers = capture.layers;
        let commit = engine.commit_layout_transaction(&transaction, &mut layers);
        let update = WmTransactionUpdate {
            commit: commit.clone(),
            ipc_error: None,
        };
        let mut runtime = SessionRuntimeLoop::default();
        let runtime_report =
            runtime.step_observations([runtime_observation_from_wm_transaction_update(&update)])?;
        let frame = engine.plan_frame(
            FramePlanRequest {
                output: output.id,
                frame_serial: 3,
            },
            layers,
        )?;
        let replay = engine.replay_frame(&frame)?;
        println!(
            "x-smoke-external-wm display={} wm={} windows={} surfaces={} placements={} focus={} outcome={:?} commands={} replay_steps={} damage_rects={} runtime_phase={:?} runtime_commands={}",
            capture
                .report
                .display_name
                .as_deref()
                .unwrap_or("<default>"),
            wm_path,
            capture.report.mirrored_windows,
            capture.report.surfaces,
            transaction.render_positions.len(),
            transaction.focus.is_some(),
            commit.outcome,
            frame.commands.len(),
            replay.steps.len(),
            replay.damage.rects.len(),
            runtime.state().phase,
            runtime_report.commands.len()
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-smoke-live-runtime-wm-socket")
    {
        let display = arg_value(args, "--display");
        let wm_path = arg_value(args, "--wm")
            .or_else(|| std::env::var("SOPHIA_WM_DEMO").ok())
            .unwrap_or_else(|| "target/debug/sophia-wm-demo".to_owned());
        let socket_path = std::env::temp_dir().join(format!(
            "sophia-live-runtime-wm-{}-{}.sock",
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
            max_attempts: 1,
            initial_backoff: Duration::ZERO,
            max_backoff: Duration::ZERO,
        };
        let state = sophia_runtime::SupervisorState::new(SupervisedProcessKind::WindowManager);
        let (_, start_command) = update_supervisor(state, SupervisorEvent::StartRequested, policy);
        let start_event = supervisor
            .apply(start_command)
            .map_err(|error| format!("failed to start WM socket process: {error}"))?
            .ok_or("WM socket process did not report a start event")?;
        if start_event != SupervisorEvent::ProcessStarted {
            return Err(format!("unexpected WM socket start event: {start_event:?}").into());
        }
        wait_for_socket(&socket_path)?;

        let capture = capture_readback_display(display.as_deref())?;
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let workspace = WorkspaceId::from_raw(1);
        let mut layers = capture.layers;
        let request = WmRequestPacket {
            transaction: TransactionId::from_raw(32),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: output.id,
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: output.size.width,
                    height: output.size.height,
                },
                nodes: layout_nodes_from_layers(&layers, workspace),
            }),
        };
        let stream = UnixStream::connect(&socket_path)?;
        let mut transport = WmSocketTransport::new(stream, WmSocketTransportConfig::default());
        let response = transport.request(&request)?;
        let wm_command_count = response.commands.len();
        let transaction = response.into_layout_transaction();
        let commit = engine.commit_layout_transaction(&transaction, &mut layers);
        let update = WmTransactionUpdate {
            commit: commit.clone(),
            ipc_error: None,
        };

        let mut driver = HeadlessSessionDriver::new(engine);
        let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
            x_event_count: u32::try_from(capture.report.mirrored_windows).unwrap_or(u32::MAX),
            wm_update: Some(update),
            portal_commands: Vec::new(),
            chrome_command_count: 0,
            layers,
            committed_surfaces: Vec::new(),
        });
        let report = driver.run_with_adapter(output.id, 7, &mut adapter)?;
        let session_tick = report
            .session_tick
            .as_ref()
            .ok_or("live runtime WM socket smoke did not render a frame")?;

        supervisor.terminate()?;
        let _ = std::fs::remove_file(&socket_path);

        println!(
            "x-smoke-live-runtime-wm-socket display={} wm={} socket={} windows={} surfaces={} placements={} focus={} outcome={:?} wm_commands={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={} cached_layers={} frame_layers={} replay_steps={} damage_rects={}",
            capture
                .report
                .display_name
                .as_deref()
                .unwrap_or("<default>"),
            wm_path,
            socket_path.display(),
            capture.report.mirrored_windows,
            capture.report.surfaces,
            transaction.render_positions.len(),
            transaction.focus.is_some(),
            commit.outcome,
            wm_command_count,
            report.runtime_state.phase,
            report.runtime_commands.len(),
            report.runtime_state.frames_rendered,
            report.runtime_state.x_events_polled,
            report.cached_layers,
            session_tick.frame.layers.len(),
            session_tick.replay.steps.len(),
            session_tick.replay.damage.rects.len()
        );
        return Ok(true);
    }

    Ok(false)
}

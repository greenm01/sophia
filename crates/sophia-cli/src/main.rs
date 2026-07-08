use sophia_engine::{
    FrameClockTick, FramePlanRequest, FrameScheduleDecision, HeadlessEngine, LastCommittedLayout,
    LayoutEpochState, SessionLayerSource, SessionTickRequest, WmSocketTransport,
    WmSocketTransportConfig, schedule_frame_from_damage,
};
use sophia_portal::{ClipboardPortal, ClipboardTarget, ClipboardTransferRequest, PortalCommand};
use sophia_protocol::{
    BrokerHealthPacket, BrokerHealthState, BrokerKind, BufferSource, DamageFrame, LayerSnapshot,
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, NamespaceId,
    PortalTransferId, Rect, Region, Size, SurfaceConstraints, SurfaceId, TransactionId, Transform,
    WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WorkspaceId, XWindowId, XWindowMirror,
    decode_broker_health_frame, encode_broker_health_frame,
};
use sophia_runtime::{
    ProcessLaunchSpec, ProcessSupervisor, RestartPolicy, RuntimeBrokerSupervisors,
    SessionRuntimeCommand, SessionRuntimeEvent, SessionRuntimeState, SupervisedProcessKind,
    SupervisorEvent, TraceLevel, init_tracing, update_session_runtime, update_supervisor,
};
use sophia_wm_demo::{ExternalWmClient, tile_workspace};
use sophia_x_bridge::{
    ClipboardSelectionFailureRequest, TestClientConfig, XMirrorState, XSelectionChangeKind,
    XSelectionEvent, XSelectionMonitor, capture_readback_display,
    clipboard_portal_request_from_selection_request, clipboard_selection_failure_notify,
    run_test_client_window, smoke_routed_input, smoke_routed_input_edges, stress_routed_input,
};
use std::os::unix::net::UnixStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use x11rb::protocol::xproto::SelectionRequestEvent;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let verbose = args.iter().any(|arg| arg == "-v" || arg == "--verbose");
    let level = if verbose {
        TraceLevel::Debug
    } else {
        TraceLevel::Info
    };

    init_tracing(level)?;

    if args.iter().any(|arg| arg == "x-test-client") {
        let config = TestClientConfig {
            display_name: arg_value(&args, "--display"),
            size: Size {
                width: arg_value(&args, "--width")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(320),
                height: arg_value(&args, "--height")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(200),
            },
            hold_millis: arg_value(&args, "--seconds")
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
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-readback") {
        let display = arg_value(&args, "--display");
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
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-frame") {
        let display = arg_value(&args, "--display");
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
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-policy-frame") {
        let display = arg_value(&args, "--display");
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
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-runtime-tick") {
        let display = arg_value(&args, "--display");
        let mut runtime = SessionRuntimeState::default();
        let mut runtime_commands = Vec::new();
        let (next_runtime, command) =
            update_session_runtime(runtime, SessionRuntimeEvent::TickStarted);
        runtime = next_runtime;
        runtime_commands.push(command);

        let capture = capture_readback_display(display.as_deref())?;
        let (next_runtime, command) = update_session_runtime(
            runtime,
            SessionRuntimeEvent::XEventsPolled {
                count: u32::try_from(capture.report.mirrored_windows).unwrap_or(u32::MAX),
            },
        );
        runtime = next_runtime;
        runtime_commands.push(command);
        if command == SessionRuntimeCommand::RequestWmLayout {
            let (next_runtime, command) =
                update_session_runtime(runtime, SessionRuntimeEvent::WmLayoutReady);
            runtime = next_runtime;
            runtime_commands.push(command);
        }

        let engine = HeadlessEngine::default();
        let output = engine.output();
        let mut last_committed = LastCommittedLayout::default();
        let frame_serial = 4;
        let (next_runtime, command) = update_session_runtime(
            runtime,
            SessionRuntimeEvent::FrameScheduled { frame_serial },
        );
        runtime = next_runtime;
        runtime_commands.push(command);

        let report = engine.run_session_tick(
            SessionTickRequest {
                output: output.id,
                frame_serial,
                layers: SessionLayerSource::Fresh(capture.layers),
            },
            &mut last_committed,
        )?;
        let (next_runtime, command) =
            update_session_runtime(runtime, SessionRuntimeEvent::FrameRendered { frame_serial });
        runtime = next_runtime;
        runtime_commands.push(command);
        let (next_runtime, command) = update_session_runtime(
            runtime,
            SessionRuntimeEvent::PortalCommandsReady { count: 0 },
        );
        runtime = next_runtime;
        runtime_commands.push(command);
        let (next_runtime, command) = update_session_runtime(
            runtime,
            SessionRuntimeEvent::ChromeCommandsReady { count: 0 },
        );
        runtime = next_runtime;
        runtime_commands.push(command);

        println!(
            "x-smoke-runtime-tick display={} windows={} surfaces={} layers={} readbacks={} bytes={} restored={} commands={} replay_steps={} damage_rects={} cached_layers={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={} runtime_portal={} runtime_chrome={}",
            capture
                .report
                .display_name
                .as_deref()
                .unwrap_or("<default>"),
            capture.report.mirrored_windows,
            capture.report.surfaces,
            report.frame.layers.len(),
            capture.report.readbacks,
            capture.report.total_bytes,
            report.restored_last_committed,
            report.frame.commands.len(),
            report.replay.steps.len(),
            report.replay.damage.rects.len(),
            last_committed.layers().len(),
            runtime.phase,
            runtime_commands.len(),
            runtime.frames_rendered,
            runtime.x_events_polled,
            runtime.portal_commands_drained,
            runtime.chrome_commands_presented
        );
        return Ok(());
    }

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

        let mut runtime = SessionRuntimeState::default();
        let mut runtime_commands = Vec::new();
        for event in [
            SessionRuntimeEvent::TickStarted,
            SessionRuntimeEvent::XEventsPolled { count: 1 },
            SessionRuntimeEvent::WmLayoutReady,
            SessionRuntimeEvent::FrameScheduled {
                frame_serial: scheduled_frame_serial,
            },
            SessionRuntimeEvent::FrameRendered {
                frame_serial: scheduled_frame_serial,
            },
            SessionRuntimeEvent::PortalCommandsReady { count: 0 },
            SessionRuntimeEvent::ChromeCommandsReady { count: 0 },
        ] {
            let (next_runtime, command) = update_session_runtime(runtime, event);
            runtime = next_runtime;
            runtime_commands.push(command);
        }

        println!(
            "runtime-damage-epoch-smoke output={} frame_serial={} completed_epoch={:?} pending_surfaces={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={}",
            output.id.raw(),
            scheduled_frame_serial,
            completed_epoch,
            epoch.pending_surfaces().len(),
            runtime.phase,
            runtime_commands.len(),
            runtime.frames_rendered,
            runtime.x_events_polled
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "runtime-brokers-smoke") {
        let portal = arg_value(&args, "--portal").unwrap_or_else(|| "/usr/bin/true".to_owned());
        let metadata = arg_value(&args, "--metadata").unwrap_or_else(|| "/usr/bin/true".to_owned());
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
        return Ok(());
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
        let (runtime, runtime_command) = update_session_runtime(
            SessionRuntimeState::default(),
            SessionRuntimeEvent::BrokerHealthChanged {
                broker: decoded.broker,
                state: decoded.state,
                generation: decoded.generation,
                status_message_len: message_len,
            },
        );

        println!(
            "portal-broker-health-smoke broker={:?} state={:?} generation={} message_len={} frame_bytes={} runtime_health={:?} runtime_command={:?}",
            decoded.broker,
            decoded.state,
            decoded.generation,
            message_len,
            frame.len(),
            runtime.portal_broker_health,
            runtime_command
        );
        return Ok(());
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
        let (runtime, runtime_command) = update_session_runtime(
            SessionRuntimeState::default(),
            SessionRuntimeEvent::BrokerHealthChanged {
                broker: decoded.broker,
                state: decoded.state,
                generation: decoded.generation,
                status_message_len: message_len,
            },
        );

        println!(
            "metadata-broker-health-smoke broker={:?} state={:?} generation={} message_len={} frame_bytes={} runtime_health={:?} runtime_command={:?}",
            decoded.broker,
            decoded.state,
            decoded.generation,
            message_len,
            frame.len(),
            runtime.metadata_broker_health,
            runtime_command
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-external-wm") {
        let display = arg_value(&args, "--display");
        let wm_path = arg_value(&args, "--wm")
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
        let frame = engine.plan_frame(
            FramePlanRequest {
                output: output.id,
                frame_serial: 3,
            },
            layers,
        )?;
        let replay = engine.replay_frame(&frame)?;
        println!(
            "x-smoke-external-wm display={} wm={} windows={} surfaces={} placements={} focus={} outcome={:?} commands={} replay_steps={} damage_rects={}",
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
            replay.damage.rects.len()
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "wm-supervisor-smoke") {
        let wm_path = arg_value(&args, "--wm")
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
        return Ok(());
    }

    if args.iter().any(|arg| arg == "portal-clipboard-deny-smoke") {
        let transfer = PortalTransferId::from_raw(1);
        let source_namespace = NamespaceId::from_raw(10);
        let target_namespace = NamespaceId::from_raw(20);
        let generation = 7;
        let mut portal = ClipboardPortal::new();
        portal
            .request_import(ClipboardTransferRequest {
                transfer,
                source_namespace,
                target_namespace,
                target: ClipboardTarget::Atom("UTF8_STRING".to_owned()),
                byte_size: 128,
                generation,
            })
            .map_err(|error| format!("clipboard portal import failed: {error:?}"))?;
        let command = portal
            .deny(transfer)
            .map_err(|error| format!("clipboard portal denial failed: {error:?}"))?;
        let PortalCommand::FailSelection { transfer } = command else {
            return Err(format!("expected FailSelection, got {command:?}").into());
        };
        let failure = clipboard_selection_failure_notify(ClipboardSelectionFailureRequest {
            transfer,
            requestor: 0x44,
            selection: 0x100,
            target: 0x200,
            time: 55,
        });

        if !failure.failed_normally() {
            return Err("clipboard denial did not map to SelectionNotify property=None".into());
        }

        println!(
            "portal-clipboard-deny-smoke transfer={} source_ns={} target_ns={} generation={} command=FailSelection selection_notify_property={} normal_failure={}",
            transfer.raw(),
            source_namespace.raw(),
            target_namespace.raw(),
            generation,
            failure.event.property,
            failure.failed_normally(),
        );
        return Ok(());
    }

    if args
        .iter()
        .any(|arg| arg == "portal-clipboard-request-smoke")
    {
        let transfer = PortalTransferId::from_raw(2);
        let source_namespace = NamespaceId::from_raw(10);
        let target_namespace = NamespaceId::from_raw(20);
        let owner = XWindowId::new(0x40, 1);
        let requestor = XWindowId::new(0x44, 1);
        let mut mirror = XMirrorState::default();
        mirror.ingest_window(clipboard_mirror(owner, source_namespace));
        mirror.ingest_window(clipboard_mirror(requestor, target_namespace));

        let mut monitor = XSelectionMonitor::new();
        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(owner),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &mirror,
        );
        let request = SelectionRequestEvent {
            response_type: 0,
            sequence: 1,
            time: 55,
            owner: owner.xid(),
            requestor: requestor.xid(),
            selection: 0x100,
            target: 0x200,
            property: 0x300,
        };
        let portal_request = clipboard_portal_request_from_selection_request(
            &request,
            "UTF8_STRING",
            &monitor,
            &mirror,
            transfer,
        )
        .map_err(|error| format!("selection request conversion failed: {error:?}"))?;
        let mut portal = ClipboardPortal::new();
        portal
            .request_import(portal_request.request.clone())
            .map_err(|error| format!("clipboard portal import failed: {error:?}"))?;
        let PortalCommand::FailSelection { transfer } = portal
            .deny(transfer)
            .map_err(|error| format!("clipboard portal denial failed: {error:?}"))?
        else {
            return Err("expected clipboard denial to fail selection".into());
        };
        let failure = clipboard_selection_failure_notify(portal_request.failure);

        println!(
            "portal-clipboard-request-smoke transfer={} source_ns={} target_ns={} owner_generation={} requestor={:#x} selection={:#x} target={:#x} property={:#x} failure_property={} normal_failure={}",
            transfer.raw(),
            portal_request.request.source_namespace.raw(),
            portal_request.request.target_namespace.raw(),
            update.current.generation,
            failure.event.requestor,
            failure.event.selection,
            failure.event.target,
            portal_request.property,
            failure.event.property,
            failure.failed_normally(),
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-routed-input") {
        let display = arg_value(&args, "--display");
        let report = smoke_routed_input(display.as_deref())?;
        println!(
            "x-smoke-routed-input display={} opcode={} target={:#x} device={} outcome={:?} event=button{}@{},{} request_bytes={} dispatch_us={}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.extension_opcode,
            report.target_window.xid(),
            report.device.raw(),
            report.decision.outcome,
            report.button,
            report.event_x,
            report.event_y,
            report.request_bytes,
            report.dispatch_elapsed.as_micros()
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-routed-input-edges") {
        let reports = smoke_routed_input_edges(sophia_protocol::XWindowId::new(0x30, 1));
        for report in reports {
            println!(
                "x-smoke-routed-input-edges edge={:?} target={:#x} outcome={:?} delivery_allowed={}",
                report.edge,
                report.decision.target_window.xid(),
                report.decision.outcome,
                report.delivery_allowed
            );
        }
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-stress-routed-input") {
        let display = arg_value(&args, "--display");
        let iterations = arg_value(&args, "--iterations")
            .as_deref()
            .map(parse_usize)
            .transpose()?
            .unwrap_or(1_000);
        let threshold_us = arg_value(&args, "--threshold-us")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .unwrap_or(500);
        let threshold = std::time::Duration::from_micros(threshold_us);
        let report = stress_routed_input(display.as_deref(), iterations, threshold)?;
        println!(
            "x-stress-routed-input display={} opcode={} target={:#x} device={} iterations={} accepted={} request_bytes={} min_us={} avg_us={} p95_us={} max_us={} threshold_us={} recommendation={:?}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.extension_opcode,
            report.target_window.xid(),
            report.device.raw(),
            report.iterations,
            report.accepted,
            report.request_bytes,
            duration_us(report.stats.min()),
            duration_us(report.stats.average()),
            duration_us(report.stats.percentile_nearest(95)),
            duration_us(report.stats.max()),
            report.threshold.as_micros(),
            report.recommendation
        );
        return Ok(());
    }

    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-bridge, protocol, wm-demo");
    println!("commands: x-test-client [--display=:99] [--seconds=5] [--width=320] [--height=200]");
    println!("commands: x-smoke-readback [--display=:99]");
    println!("commands: x-smoke-frame [--display=:99]");
    println!("commands: x-smoke-policy-frame [--display=:99]");
    println!("commands: x-smoke-runtime-tick [--display=:99]");
    println!("commands: runtime-damage-epoch-smoke");
    println!("commands: runtime-brokers-smoke [--portal=/usr/bin/true] [--metadata=/usr/bin/true]");
    println!("commands: portal-broker-health-smoke");
    println!("commands: metadata-broker-health-smoke");
    println!("commands: x-smoke-external-wm [--display=:99] [--wm=target/debug/sophia-wm-demo]");
    println!("commands: wm-supervisor-smoke [--wm=target/debug/sophia-wm-demo]");
    println!("commands: portal-clipboard-deny-smoke");
    println!("commands: portal-clipboard-request-smoke");
    println!("commands: x-smoke-routed-input [--display=:99]");
    println!("commands: x-smoke-routed-input-edges");
    println!(
        "commands: x-stress-routed-input [--display=:99] [--iterations=1000] [--threshold-us=500]"
    );

    if verbose {
        tracing::debug!("verbose tracing enabled");
        println!("logging: tracing subscriber initialized");
    }

    Ok(())
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

fn parse_usize(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    value
        .parse::<usize>()
        .map_err(|error| format!("invalid usize value {value:?}: {error}").into())
}

fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid u64 value {value:?}: {error}").into())
}

fn duration_us(duration: Option<std::time::Duration>) -> u128 {
    duration.map_or(0, |duration| duration.as_micros())
}

fn clipboard_mirror(window: XWindowId, namespace: NamespaceId) -> XWindowMirror {
    XWindowMirror {
        window,
        parent: None,
        children: Vec::new(),
        toplevel: Some(window),
        client: Some(window),
        mapped: true,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        },
        namespace: Some(namespace),
        stale_metadata: 0,
    }
}

#[derive(Clone, Copy, Debug)]
struct SupervisedWmRequestReport {
    outcome: sophia_protocol::TransactionOutcome,
    commands: usize,
}

fn wait_for_socket(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut last_error = None;

    while std::time::Instant::now() < deadline {
        match UnixStream::connect(path) {
            Ok(_) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    Err(format!(
        "timed out waiting for WM socket {}: {}",
        path.display(),
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "not attempted".to_owned())
    )
    .into())
}

fn request_supervised_wm(
    path: &std::path::Path,
    transaction: TransactionId,
) -> Result<SupervisedWmRequestReport, Box<dyn std::error::Error>> {
    let stream = UnixStream::connect(path)?;
    let mut transport = WmSocketTransport::new(stream, WmSocketTransportConfig::default());
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let workspace = WorkspaceId::from_raw(1);
    let mut layers = synthetic_layers();
    let request = WmRequestPacket {
        transaction,
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
    let response = transport.request(&request)?;
    let command_count = response.commands.len();
    let transaction = response.into_layout_transaction();
    let commit = engine.commit_layout_transaction(&transaction, &mut layers);

    Ok(SupervisedWmRequestReport {
        outcome: commit.outcome,
        commands: command_count,
    })
}

fn synthetic_layers() -> Vec<LayerSnapshot> {
    vec![LayerSnapshot {
        surface: SurfaceId::new(1, 1),
        window: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 10,
            y: 10,
            width: 320,
            height: 200,
        },
        source: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::single(Rect {
            x: 10,
            y: 10,
            width: 320,
            height: 200,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
    }]
}

fn layout_nodes_from_layers(
    layers: &[sophia_protocol::LayerSnapshot],
    workspace: WorkspaceId,
) -> Vec<LayoutNodeSnapshot> {
    layers
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
        .collect()
}

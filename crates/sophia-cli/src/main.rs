use sophia_engine::{FramePlanRequest, HeadlessEngine};
use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, Rect, Size,
    SurfaceConstraints, TransactionId, WorkspaceId,
};
use sophia_runtime::{TraceLevel, init_tracing};
use sophia_wm_demo::{ExternalWmClient, tile_workspace};
use sophia_x_bridge::{
    TestClientConfig, capture_readback_display, run_test_client_window, smoke_routed_input,
};

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

    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-bridge, protocol, wm-demo");
    println!("commands: x-test-client [--display=:99] [--seconds=5] [--width=320] [--height=200]");
    println!("commands: x-smoke-readback [--display=:99]");
    println!("commands: x-smoke-frame [--display=:99]");
    println!("commands: x-smoke-policy-frame [--display=:99]");
    println!("commands: x-smoke-external-wm [--display=:99] [--wm=target/debug/sophia-wm-demo]");
    println!("commands: x-smoke-routed-input [--display=:99]");

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

use std::path::PathBuf;

use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, Rect,
    SurfaceConstraints, SurfaceId, TransactionId, WmCommand, WmRelayoutWorkspace, WmRequestKind,
    WmRequestPacket, WorkspaceId,
};
use sophia_x11_wm_bridge::{XmonadBridgeRuntime, run_wm_socket_server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let xmonad = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--xmonad="))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("SOPHIA_XMONAD_BIN").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("xmonad"));
    match args.first().map(String::as_str) {
        Some("serve-socket") => {
            let socket = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--socket="))
                .ok_or("missing --socket=PATH")?;
            run_wm_socket_server(socket, xmonad)?;
        }
        Some("smoke") => run_smoke(xmonad)?,
        _ => {
            return Err(
                "usage: sophia-x11-wm-bridge <serve-socket --socket=PATH|smoke> [--xmonad=PATH]"
                    .into(),
            );
        }
    }
    Ok(())
}

fn run_smoke(xmonad: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = WorkspaceId::from_raw(1);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    };
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(1),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace,
            bounds,
            nodes: vec![node(10, workspace, bounds), node(11, workspace, bounds)],
        }),
    };
    let mut runtime = XmonadBridgeRuntime::start(xmonad)?;
    let response = runtime.handle_request(&request)?;
    let placements = response
        .commands
        .iter()
        .filter_map(|command| match command {
            WmCommand::RenderSurface(placement) => Some(placement),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut actual = placements
        .iter()
        .map(|placement| (placement.surface.index(), placement.geometry))
        .collect::<Vec<_>>();
    actual.sort_by_key(|(_, geometry)| geometry.x);
    let expected = vec![
        (
            11,
            Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 720,
            },
        ),
        (
            10,
            Rect {
                x: 640,
                y: 0,
                width: 640,
                height: 720,
            },
        ),
    ];
    if response.transaction != request.transaction || actual != expected {
        return Err(format!(
            "xmonad did not produce the strict two-tile response: transaction={:?} actual={actual:?}",
            response.transaction
        )
        .into());
    }
    println!(
        "real-xmonad-two-window-smoke: pass transaction={} left={:?} right={:?}",
        response.transaction.raw(),
        actual[0].1,
        actual[1].1
    );
    Ok(())
}

fn node(raw: u32, workspace: WorkspaceId, geometry: Rect) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(raw, 1),
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry,
        generation: 1,
    }
}

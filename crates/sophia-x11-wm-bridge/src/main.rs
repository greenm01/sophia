use std::path::PathBuf;

use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, Rect,
    SurfaceConstraints, SurfaceId, TransactionId, WmCommand, WmRelayoutWorkspace, WmRequestKind,
    WmRequestPacket, WorkspaceId,
};
use sophia_x11_wm_bridge::{
    LegacyWmLaunchSpec, LegacyWmProfile, LegacyX11WmBridgeRuntime, run_wm_socket_server,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("serve-socket") => {
            let socket = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--socket="))
                .ok_or("missing --socket=PATH")?;
            let executable = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--wm="))
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("SOPHIA_LEGACY_X11_WM").map(PathBuf::from))
                .ok_or("missing --wm=PATH or SOPHIA_LEGACY_X11_WM")?;
            let launch = args
                .iter()
                .filter_map(|arg| arg.strip_prefix("--wm-arg="))
                .fold(LegacyWmLaunchSpec::new(executable), |launch, argument| {
                    launch.arg(argument)
                });
            let launch = match args
                .iter()
                .find_map(|arg| arg.strip_prefix("--wm-private-alias="))
            {
                Some(alias) => launch.with_private_executable_alias(alias),
                None => launch,
            };
            let profile = match args.iter().find_map(|arg| arg.strip_prefix("--profile=")) {
                None | Some("layout-only") => LegacyWmProfile::LayoutOnly,
                Some("xmonad") => LegacyWmProfile::Xmonad,
                Some(profile) => {
                    return Err(format!("unsupported legacy WM profile {profile:?}").into());
                }
            };
            let launch = launch.with_profile(profile);
            run_wm_socket_server(socket, launch)?;
        }
        Some("xmonad-smoke" | "smoke") => {
            let xmonad = args
                .iter()
                .find_map(|arg| arg.strip_prefix("--xmonad="))
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("SOPHIA_XMONAD_BIN").map(PathBuf::from))
                .unwrap_or_else(|| PathBuf::from("xmonad"));
            run_xmonad_smoke(xmonad)?;
        }
        _ => {
            return Err(
                "usage: sophia-x11-wm-bridge serve-socket --socket=PATH --wm=PATH [--profile=layout-only|xmonad] [--wm-arg=ARG ...] [--wm-private-alias=RELATIVE]\n       sophia-x11-wm-bridge xmonad-smoke [--xmonad=PATH]"
                    .into(),
            );
        }
    }
    Ok(())
}

fn run_xmonad_smoke(xmonad: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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
    let launch =
        LegacyWmLaunchSpec::new(xmonad).with_private_executable_alias("xmonad/xmonad-x86_64-linux");
    let mut runtime = LegacyX11WmBridgeRuntime::start(launch)?;
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

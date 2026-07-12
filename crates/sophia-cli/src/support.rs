use sophia_engine::{HeadlessEngine, WmSocketTransport, WmSocketTransportConfig};
use sophia_protocol::{
    BufferSource, LayerSnapshot, LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot,
    LayoutNodeState, Rect, Region, ResizeSyncCapability, SurfaceConstraints, SurfaceId,
    TransactionId, Transform, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WorkspaceId,
};
#[cfg(feature = "xlibre-research")]
use sophia_protocol::{NamespaceId, XWindowId, XWindowMirror};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub(crate) fn arg_value(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

#[cfg(any(feature = "atomic-scanout-live", feature = "xlibre-research"))]
pub(crate) fn parse_usize(value: &str) -> Result<usize, Box<dyn std::error::Error>> {
    value
        .parse::<usize>()
        .map_err(|error| format!("invalid usize value {value:?}: {error}").into())
}

#[cfg(any(feature = "atomic-scanout-live", feature = "xlibre-research"))]
pub(crate) fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid u64 value {value:?}: {error}").into())
}

#[cfg(feature = "xlibre-research")]
pub(crate) fn duration_us(duration: Option<std::time::Duration>) -> u128 {
    duration.map_or(0, |duration| duration.as_micros())
}

#[cfg(feature = "xlibre-research")]
pub(crate) fn clipboard_mirror(window: XWindowId, namespace: NamespaceId) -> XWindowMirror {
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
pub(crate) struct SupervisedWmRequestReport {
    pub(crate) outcome: sophia_protocol::TransactionOutcome,
    pub(crate) commands: usize,
}

pub(crate) fn wait_for_socket(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
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

pub(crate) fn request_supervised_wm(
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

pub(crate) fn synthetic_layers() -> Vec<LayerSnapshot> {
    vec![LayerSnapshot {
        surface: SurfaceId::new(1, 1),
        authority_local_id: None,
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
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }]
}

pub(crate) fn layout_nodes_from_layers(
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

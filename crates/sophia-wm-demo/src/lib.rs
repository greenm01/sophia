use core::fmt;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::{
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, LayoutTransaction,
    OutputId, Rect, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, Size, SurfaceConstraints,
    SurfaceId, SurfacePlacement, SurfaceSizeRequest, TransactionId, Transform, WmCommand,
    WmManageSurface, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WmResponsePacket,
    WorkspaceId, decode_wm_request_frame, encode_wm_response_frame,
};

pub fn empty_transaction(transaction: TransactionId) -> LayoutTransaction {
    LayoutTransaction {
        transaction,
        requested_sizes: Vec::new(),
        focus: None,
        render_positions: Vec::new(),
        timeout_msec: 300,
    }
}

pub fn tile_workspace(
    transaction: TransactionId,
    workspace: WorkspaceId,
    bounds: Rect,
    nodes: &[LayoutNodeSnapshot],
) -> LayoutTransaction {
    let visible_nodes = nodes
        .iter()
        .filter(|node| node.workspace == workspace && node.state.visible)
        .collect::<Vec<_>>();

    if visible_nodes.is_empty() || bounds.is_empty() {
        return empty_transaction(transaction);
    }

    let width = bounds.width / i32::try_from(visible_nodes.len()).expect("visible node overflow");
    let mut render_positions = Vec::with_capacity(visible_nodes.len());
    let mut requested_sizes = Vec::with_capacity(visible_nodes.len());
    let mut focus = None;

    for (index, node) in visible_nodes.iter().enumerate() {
        let index = i32::try_from(index).expect("visible node index overflow");
        let is_last =
            usize::try_from(index + 1).expect("visible node index overflow") == visible_nodes.len();
        let x = bounds.x + width * index;
        let tile_width = if is_last {
            bounds.x + bounds.width - x
        } else {
            width
        };
        let geometry = Rect {
            x,
            y: bounds.y,
            width: tile_width.max(1),
            height: bounds.height,
        };
        let requested = clamp_size(
            Size {
                width: geometry.width,
                height: geometry.height,
            },
            node.constraints.min_size,
            node.constraints.max_size,
        );

        if focus.is_none() && node.capabilities.focusable {
            focus = Some(node.surface);
        }

        requested_sizes.push(SurfaceSizeRequest {
            surface: node.surface,
            size: requested,
        });
        render_positions.push(SurfacePlacement {
            surface: node.surface,
            geometry,
            z_index: index,
            crop: None,
            transform: Transform::IDENTITY,
        });
    }

    LayoutTransaction {
        transaction,
        requested_sizes,
        focus,
        render_positions,
        timeout_msec: 300,
    }
}

pub fn handle_wm_request(request: WmRequestPacket) -> WmResponsePacket {
    match request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let transaction = tile_workspace(
                request.transaction,
                manage.workspace,
                manage.bounds,
                &[manage.node],
            );
            response_from_layout_transaction(transaction, Some(manage.workspace))
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let transaction = tile_workspace(
                request.transaction,
                relayout.workspace,
                relayout.bounds,
                &relayout.nodes,
            );
            response_from_layout_transaction(transaction, None)
        }
        WmRequestKind::SurfaceRemoved { .. } => WmResponsePacket {
            transaction: request.transaction,
            commands: Vec::new(),
            timeout_msec: 300,
        },
    }
}

pub fn response_from_layout_transaction(
    transaction: LayoutTransaction,
    assigned_workspace: Option<WorkspaceId>,
) -> WmResponsePacket {
    let mut commands = Vec::new();

    if let Some(workspace) = assigned_workspace {
        for placement in &transaction.render_positions {
            commands.push(WmCommand::AssignWorkspace {
                surface: placement.surface,
                workspace,
            });
        }
    }

    commands.extend(
        transaction
            .requested_sizes
            .iter()
            .copied()
            .map(WmCommand::ConfigureSurface),
    );

    if let Some(focus) = transaction.focus {
        commands.push(WmCommand::FocusSurface(focus));
    }

    commands.extend(
        transaction
            .render_positions
            .iter()
            .copied()
            .map(WmCommand::RenderSurface),
    );

    WmResponsePacket {
        transaction: transaction.transaction,
        commands,
        timeout_msec: transaction.timeout_msec,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmProcessError {
    message: String,
}

impl WmProcessError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WmProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WmProcessError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalWmClient {
    program: PathBuf,
}

impl ExternalWmClient {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn request(&self, request: &WmRequestPacket) -> Result<WmResponsePacket, WmProcessError> {
        let output = Command::new(&self.program)
            .args(request_to_process_args(request))
            .output()
            .map_err(|error| {
                WmProcessError::new(format!(
                    "failed to spawn external WM {}: {error}",
                    self.program.display()
                ))
            })?;

        if !output.status.success() {
            return Err(WmProcessError::new(format!(
                "external WM {} exited with status {}: {}",
                self.program.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let stdout = String::from_utf8(output.stdout).map_err(|error| {
            WmProcessError::new(format!("external WM emitted non-UTF8: {error}"))
        })?;
        decode_process_response(stdout.trim())
    }
}

pub fn run_process_request(args: &[String]) -> Result<String, WmProcessError> {
    if args.first().map(String::as_str) == Some("serve-socket") {
        return Err(WmProcessError::new(
            "serve-socket is long-running; call run_socket_server instead",
        ));
    }

    let request = parse_process_request(args)?;
    let response = handle_wm_request(request);
    Ok(format!("{}\n", encode_process_response(&response)))
}

#[cfg(unix)]
pub fn run_socket_server(path: impl AsRef<Path>) -> Result<(), WmProcessError> {
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(WmProcessError::new(format!(
                "failed to remove stale socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        WmProcessError::new(format!(
            "failed to bind WM socket {}: {error}",
            path.display()
        ))
    })?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => serve_socket_client(&mut stream)?,
            Err(error) => {
                return Err(WmProcessError::new(format!(
                    "failed to accept WM socket client on {}: {error}",
                    path.display()
                )));
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn serve_socket_client(stream: &mut UnixStream) -> Result<(), WmProcessError> {
    while let Some(request) = read_wm_request(stream)? {
        let response = handle_wm_request(request);
        let frame = encode_wm_response_frame(&response).map_err(|error| {
            WmProcessError::new(format!("failed to encode WM response: {error:?}"))
        })?;
        stream.write_all(&frame).map_err(|error| {
            WmProcessError::new(format!("failed to write WM response: {error}"))
        })?;
        stream.flush().map_err(|error| {
            WmProcessError::new(format!("failed to flush WM response: {error}"))
        })?;
    }

    Ok(())
}

#[cfg(unix)]
fn read_wm_request(stream: &mut UnixStream) -> Result<Option<WmRequestPacket>, WmProcessError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(WmProcessError::new(format!(
                "failed to read WM request header: {error}"
            )));
        }
    }

    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(WmProcessError::new(format!(
            "WM request payload too large: {payload_len}"
        )));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| {
            WmProcessError::new(format!("failed to read WM request payload: {error}"))
        })?;

    decode_wm_request_frame(&frame)
        .map(Some)
        .map_err(|error| WmProcessError::new(format!("failed to decode WM request: {error:?}")))
}

pub fn request_to_process_args(request: &WmRequestPacket) -> Vec<String> {
    match &request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let mut args = vec![
                "manage".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--output={}", manage.output.raw()),
                format!("--workspace={}", manage.workspace.raw()),
                format!("--bounds={}", encode_rect(manage.bounds)),
                format!("--node={}", encode_node(&manage.node)),
            ];
            args.shrink_to_fit();
            args
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let mut args = vec![
                "relayout".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--output={}", relayout.output.raw()),
                format!("--workspace={}", relayout.workspace.raw()),
                format!("--bounds={}", encode_rect(relayout.bounds)),
            ];
            args.extend(
                relayout
                    .nodes
                    .iter()
                    .map(|node| format!("--node={}", encode_node(node))),
            );
            args
        }
        WmRequestKind::SurfaceRemoved { surface, workspace } => vec![
            "remove".to_owned(),
            format!("--transaction={}", request.transaction.raw()),
            format!("--workspace={}", workspace.raw()),
            format!("--surface={}:{}", surface.index(), surface.generation()),
        ],
    }
}

pub fn parse_process_request(args: &[String]) -> Result<WmRequestPacket, WmProcessError> {
    let Some(kind) = args.first().map(String::as_str) else {
        return Err(WmProcessError::new(process_usage()));
    };
    let transaction = TransactionId::from_raw(required_u64(args, "--transaction")?);
    let workspace = WorkspaceId::from_raw(required_u64(args, "--workspace")?);

    match kind {
        "manage" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let bounds = required_rect(args, "--bounds")?;
            let node = required_node(args, "--node", workspace)?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::ManageSurface(WmManageSurface {
                    node,
                    output,
                    workspace,
                    bounds,
                }),
            })
        }
        "relayout" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let bounds = required_rect(args, "--bounds")?;
            let nodes = arg_values(args, "--node")
                .into_iter()
                .map(|value| parse_node(value, workspace))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                    output,
                    workspace,
                    bounds,
                    nodes,
                }),
            })
        }
        "remove" => {
            let surface = required_surface(args, "--surface")?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::SurfaceRemoved { surface, workspace },
            })
        }
        _ => Err(WmProcessError::new(process_usage())),
    }
}

pub fn encode_process_response(response: &WmResponsePacket) -> String {
    let mut assign = Vec::new();
    let mut configure = Vec::new();
    let mut focus = String::from("-");
    let mut render = Vec::new();

    for command in &response.commands {
        match command {
            WmCommand::AssignWorkspace { surface, workspace } => assign.push(format!(
                "{}:{}:{}",
                surface.index(),
                surface.generation(),
                workspace.raw()
            )),
            WmCommand::ConfigureSurface(request) => configure.push(format!(
                "{}:{}:{},{}",
                request.surface.index(),
                request.surface.generation(),
                request.size.width,
                request.size.height
            )),
            WmCommand::FocusSurface(surface) => {
                focus = format!("{}:{}", surface.index(), surface.generation());
            }
            WmCommand::RenderSurface(placement) => render.push(format!(
                "{}:{}:{}:{}:{}",
                placement.surface.index(),
                placement.surface.generation(),
                encode_rect(placement.geometry),
                placement.z_index,
                placement.crop.map_or_else(|| "-".to_owned(), encode_rect)
            )),
        }
    }

    format!(
        "ok tx={} timeout={} assign={} configure={} focus={} render={}",
        response.transaction.raw(),
        response.timeout_msec,
        encode_list(&assign),
        encode_list(&configure),
        focus,
        encode_list(&render)
    )
}

pub fn decode_process_response(line: &str) -> Result<WmResponsePacket, WmProcessError> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.first().copied() != Some("ok") {
        return Err(WmProcessError::new(format!(
            "external WM returned invalid response: {line}"
        )));
    }

    let transaction = TransactionId::from_raw(response_u64(&parts, "tx")?);
    let timeout_msec = u32::try_from(response_u64(&parts, "timeout")?)
        .map_err(|_| WmProcessError::new("timeout does not fit u32"))?;
    let mut commands = Vec::new();

    for encoded in response_value(&parts, "assign")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(WmProcessError::new(format!(
                "invalid assign command: {encoded}"
            )));
        }
        let surface = parse_surface_pair(fields[0], fields[1])?;
        let workspace = WorkspaceId::from_raw(
            fields[2]
                .parse::<u64>()
                .map_err(|_| WmProcessError::new(format!("invalid workspace: {}", fields[2])))?,
        );
        commands.push(WmCommand::AssignWorkspace { surface, workspace });
    }

    for encoded in response_value(&parts, "configure")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(WmProcessError::new(format!(
                "invalid configure command: {encoded}"
            )));
        }
        let surface = parse_surface_pair(fields[0], fields[1])?;
        let size = parse_size(fields[2])?;
        commands.push(WmCommand::ConfigureSurface(SurfaceSizeRequest {
            surface,
            size,
        }));
    }

    let focus = response_value(&parts, "focus")?;
    if focus != "-" {
        commands.push(WmCommand::FocusSurface(parse_surface_token(focus)?));
    }

    for encoded in response_value(&parts, "render")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(WmProcessError::new(format!(
                "invalid render command: {encoded}"
            )));
        }
        let surface = parse_surface_pair(fields[0], fields[1])?;
        let geometry = parse_rect(fields[2])?;
        let z_index = fields[3]
            .parse::<i32>()
            .map_err(|_| WmProcessError::new(format!("invalid z-index: {}", fields[3])))?;
        let crop = if fields[4] == "-" {
            None
        } else {
            Some(parse_rect(fields[4])?)
        };
        commands.push(WmCommand::RenderSurface(SurfacePlacement {
            surface,
            geometry,
            z_index,
            crop,
            transform: Transform::IDENTITY,
        }));
    }

    Ok(WmResponsePacket {
        transaction,
        commands,
        timeout_msec,
    })
}

fn clamp_size(size: Size, min_size: Option<Size>, max_size: Option<Size>) -> Size {
    let mut width = size.width;
    let mut height = size.height;

    if let Some(min_size) = min_size {
        width = width.max(min_size.width);
        height = height.max(min_size.height);
    }

    if let Some(max_size) = max_size {
        width = width.min(max_size.width);
        height = height.min(max_size.height);
    }

    Size { width, height }
}

fn process_usage() -> &'static str {
    "usage: sophia-wm-demo relayout --transaction=N --output=N --workspace=N --bounds=x,y,w,h --node=index:generation[:x,y,w,h]"
}

fn required_u64(args: &[String], key: &str) -> Result<u64, WmProcessError> {
    required_value(args, key)?
        .parse::<u64>()
        .map_err(|_| WmProcessError::new(format!("invalid {key} value")))
}

fn required_rect(args: &[String], key: &str) -> Result<Rect, WmProcessError> {
    parse_rect(required_value(args, key)?)
}

fn required_node(
    args: &[String],
    key: &str,
    workspace: WorkspaceId,
) -> Result<LayoutNodeSnapshot, WmProcessError> {
    parse_node(required_value(args, key)?, workspace)
}

fn required_surface(args: &[String], key: &str) -> Result<SurfaceId, WmProcessError> {
    parse_surface_token(required_value(args, key)?)
}

fn required_value<'a>(args: &'a [String], key: &str) -> Result<&'a str, WmProcessError> {
    arg_values(args, key)
        .into_iter()
        .next()
        .ok_or_else(|| WmProcessError::new(format!("missing {key}")))
}

fn arg_values<'a>(args: &'a [String], key: &str) -> Vec<&'a str> {
    let prefix = format!("{key}=");
    args.iter()
        .filter_map(|arg| arg.strip_prefix(&prefix))
        .collect()
}

fn response_u64(parts: &[&str], key: &str) -> Result<u64, WmProcessError> {
    response_value(parts, key)?
        .parse::<u64>()
        .map_err(|_| WmProcessError::new(format!("invalid response {key} value")))
}

fn response_value<'a>(parts: &'a [&str], key: &str) -> Result<&'a str, WmProcessError> {
    let prefix = format!("{key}=");
    parts
        .iter()
        .find_map(|part| part.strip_prefix(&prefix))
        .ok_or_else(|| WmProcessError::new(format!("missing response {key}")))
}

fn parse_node(value: &str, workspace: WorkspaceId) -> Result<LayoutNodeSnapshot, WmProcessError> {
    let fields = value.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&fields.len()) {
        return Err(WmProcessError::new(format!("invalid node: {value}")));
    }
    let surface = parse_surface_pair(fields[0], fields[1])?;
    let geometry = if let Some(rect) = fields.get(2) {
        parse_rect(rect)?
    } else {
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        }
    };

    Ok(LayoutNodeSnapshot {
        surface,
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
    })
}

fn parse_surface_token(value: &str) -> Result<SurfaceId, WmProcessError> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(WmProcessError::new(format!("invalid surface: {value}")));
    }
    parse_surface_pair(fields[0], fields[1])
}

fn parse_surface_pair(index: &str, generation: &str) -> Result<SurfaceId, WmProcessError> {
    let index = index
        .parse::<u32>()
        .map_err(|_| WmProcessError::new(format!("invalid surface index: {index}")))?;
    let generation = generation
        .parse::<u32>()
        .map_err(|_| WmProcessError::new(format!("invalid surface generation: {generation}")))?;
    Ok(SurfaceId::new(index, generation))
}

fn parse_size(value: &str) -> Result<Size, WmProcessError> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(WmProcessError::new(format!("invalid size: {value}")));
    }
    Ok(Size {
        width: parse_i32(fields[0])?,
        height: parse_i32(fields[1])?,
    })
}

fn parse_rect(value: &str) -> Result<Rect, WmProcessError> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 4 {
        return Err(WmProcessError::new(format!("invalid rect: {value}")));
    }
    Ok(Rect {
        x: parse_i32(fields[0])?,
        y: parse_i32(fields[1])?,
        width: parse_i32(fields[2])?,
        height: parse_i32(fields[3])?,
    })
}

fn parse_i32(value: &str) -> Result<i32, WmProcessError> {
    value
        .parse::<i32>()
        .map_err(|_| WmProcessError::new(format!("invalid integer: {value}")))
}

fn encode_node(node: &LayoutNodeSnapshot) -> String {
    format!(
        "{}:{}:{}",
        node.surface.index(),
        node.surface.generation(),
        encode_rect(node.geometry)
    )
}

fn encode_rect(rect: Rect) -> String {
    format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height)
}

fn encode_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(";")
    }
}

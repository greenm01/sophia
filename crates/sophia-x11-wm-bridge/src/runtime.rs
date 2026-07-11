use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{ErrorKind, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use sophia_protocol::{
    Rect, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, WmRequestPacket, WmResponsePacket,
    decode_wm_request_frame, encode_wm_response_frame,
};
use sophia_x_authority::{XByteOrder, serve_x11_setup_socket_client};

use crate::{
    BridgeEngineUpdate, LegacyWmRequest, SYNTHETIC_ROOT_XID, SyntheticXEvent, SyntheticXWindowId,
    X11WmBridgeError, X11WmBridgeState,
};

const FIRST_DYNAMIC_ATOM: u32 = 256;
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(3);
const QUIET_PERIOD: Duration = Duration::from_millis(80);
const IO_POLL: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub struct BridgeRuntimeError(String);

impl BridgeRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for BridgeRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BridgeRuntimeError {}

impl From<X11WmBridgeError> for BridgeRuntimeError {
    fn from(error: X11WmBridgeError) -> Self {
        Self::new(format!("bridge state rejected request: {error:?}"))
    }
}

#[derive(Clone, Copy, Debug)]
enum ServerCommand {
    Root(Rect),
    Map(SyntheticXWindowId, Rect),
    Configure(SyntheticXWindowId, Rect),
    Unmap(SyntheticXWindowId),
    Destroy(SyntheticXWindowId),
}

pub struct XmonadBridgeRuntime {
    bridge: X11WmBridgeState,
    commands: Option<SyncSender<ServerCommand>>,
    legacy: Receiver<LegacyWmRequest>,
    worker: Option<JoinHandle<Result<(), BridgeRuntimeError>>>,
    child: Child,
    socket_path: PathBuf,
    config_dir: PathBuf,
}

impl XmonadBridgeRuntime {
    pub fn start(xmonad: impl AsRef<Path>) -> Result<Self, BridgeRuntimeError> {
        let (listener, display, socket_path) = bind_private_display()?;
        let xmonad = resolve_executable(xmonad.as_ref())?;
        let config_dir = std::env::temp_dir().join(format!(
            "sophia-xmonad-bridge-{}-{display}",
            std::process::id()
        ));
        fs::create_dir_all(&config_dir).map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to create private xmonad config directory {}: {error}",
                config_dir.display()
            ))
        })?;
        let xmonad_config_dir = config_dir.join("xmonad");
        fs::create_dir_all(&xmonad_config_dir).map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to create private xmonad runtime directory {}: {error}",
                xmonad_config_dir.display()
            ))
        })?;
        symlink(&xmonad, xmonad_config_dir.join("xmonad-x86_64-linux")).map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to stage private xmonad executable: {error}"
            ))
        })?;

        let staged_xmonad = xmonad_config_dir.join("xmonad-x86_64-linux");
        let mut child = Command::new(&staged_xmonad)
            .env_clear()
            .env("DISPLAY", format!(":{display}"))
            .env("HOME", &config_dir)
            .env("XDG_CONFIG_HOME", &config_dir)
            .env("XDG_CACHE_HOME", &config_dir)
            .env("XDG_DATA_HOME", &config_dir)
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                BridgeRuntimeError::new(format!(
                    "failed to start xmonad {}: {error}",
                    xmonad.display()
                ))
            })?;

        let stream = match accept_private_xmonad(&listener, &mut child) {
            Ok(stream) => stream,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&socket_path);
                let _ = fs::remove_dir_all(&config_dir);
                return Err(error);
            }
        };

        let (command_tx, command_rx) = mpsc::sync_channel(128);
        let (legacy_tx, legacy_rx) = mpsc::sync_channel(256);
        let worker = thread::spawn(move || {
            let mut stream = stream;
            serve_xmonad(&mut stream, command_rx, legacy_tx)
        });

        Ok(Self {
            bridge: X11WmBridgeState::new(),
            commands: Some(command_tx),
            legacy: legacy_rx,
            worker: Some(worker),
            child,
            socket_path,
            config_dir,
        })
    }

    pub fn handle_request(
        &mut self,
        request: &WmRequestPacket,
    ) -> Result<WmResponsePacket, BridgeRuntimeError> {
        while self.legacy.try_recv().is_ok() {}
        let update = self.bridge.apply_engine_request(request)?;
        let expected = send_engine_update(
            &self.bridge,
            &update,
            self.commands
                .as_ref()
                .ok_or_else(|| BridgeRuntimeError::new("xmonad server stopped"))?,
        )?;

        let started = Instant::now();
        let mut last_activity = started;
        let mut configured = BTreeMap::new();
        let mut focus = None;
        loop {
            let elapsed = started.elapsed();
            if elapsed >= BRIDGE_TIMEOUT {
                break;
            }
            let wait = IO_POLL.min(BRIDGE_TIMEOUT.saturating_sub(elapsed));
            match self.legacy.recv_timeout(wait) {
                Ok(request) => {
                    last_activity = Instant::now();
                    match request {
                        request @ LegacyWmRequest::ConfigureWindow { window, .. } => {
                            configured.insert(window, request);
                        }
                        request @ LegacyWmRequest::FocusWindow { .. } => focus = Some(request),
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if expected
                        .iter()
                        .all(|window| configured.contains_key(window))
                        && last_activity.elapsed() >= QUIET_PERIOD
                    {
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(BridgeRuntimeError::new("xmonad server disconnected"));
                }
            }
        }

        if !expected
            .iter()
            .all(|window| configured.contains_key(window))
        {
            return Err(BridgeRuntimeError::new(format!(
                "xmonad did not configure all {} synthetic windows within {} ms (configured {})",
                expected.len(),
                BRIDGE_TIMEOUT.as_millis(),
                configured.len()
            )));
        }
        let mut requests = configured.into_values().collect::<Vec<_>>();
        if let Some(focus) = focus {
            requests.push(focus);
        }
        self.bridge
            .translate_legacy_requests(request.transaction, &requests, 300)
            .map_err(Into::into)
    }

    pub fn bridge(&self) -> &X11WmBridgeState {
        &self.bridge
    }
}

fn accept_private_xmonad(
    listener: &UnixListener,
    child: &mut Child,
) -> Result<UnixStream, BridgeRuntimeError> {
    listener.set_nonblocking(true).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to configure private X listener: {error}"))
    })?;
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => {
                return Err(BridgeRuntimeError::new(format!(
                    "failed to accept private xmonad socket: {error}"
                )));
            }
        }
        if let Some(status) = child.try_wait().map_err(|error| {
            BridgeRuntimeError::new(format!("failed to inspect xmonad process: {error}"))
        })? {
            return Err(BridgeRuntimeError::new(format!(
                "xmonad exited before connecting to its private display: {status}"
            )));
        }
        if started.elapsed() >= BRIDGE_TIMEOUT {
            return Err(BridgeRuntimeError::new(format!(
                "xmonad did not connect to its private display within {} ms",
                BRIDGE_TIMEOUT.as_millis()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn resolve_executable(path: &Path) -> Result<PathBuf, BridgeRuntimeError> {
    if path.components().count() > 1 {
        return fs::canonicalize(path).map_err(|error| {
            BridgeRuntimeError::new(format!(
                "failed to resolve xmonad executable {}: {error}",
                path.display()
            ))
        });
    }
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&search_path)
        .map(|directory| directory.join(path))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| fs::canonicalize(candidate).ok())
        .ok_or_else(|| {
            BridgeRuntimeError::new(format!(
                "xmonad executable '{}' was not found in PATH",
                path.display()
            ))
        })
}

impl Drop for XmonadBridgeRuntime {
    fn drop(&mut self) {
        self.commands.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_dir_all(&self.config_dir);
    }
}

pub fn run_wm_socket_server(
    path: impl AsRef<Path>,
    xmonad: impl AsRef<Path>,
) -> Result<(), BridgeRuntimeError> {
    let path = path.as_ref();
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(BridgeRuntimeError::new(format!(
                "failed to remove stale WM socket {}: {error}",
                path.display()
            )));
        }
    }
    let listener = UnixListener::bind(path).map_err(|error| {
        BridgeRuntimeError::new(format!(
            "failed to bind WM socket {}: {error}",
            path.display()
        ))
    })?;
    let mut runtime = XmonadBridgeRuntime::start(xmonad)?;
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|error| {
            BridgeRuntimeError::new(format!("failed to accept WM socket client: {error}"))
        })?;
        while let Some(request) = read_wm_request(&mut stream)? {
            let response = runtime.handle_request(&request)?;
            let frame = encode_wm_response_frame(&response).map_err(|error| {
                BridgeRuntimeError::new(format!("failed to encode WM response: {error:?}"))
            })?;
            stream.write_all(&frame).map_err(|error| {
                BridgeRuntimeError::new(format!("failed to write WM response: {error}"))
            })?;
            stream.flush().map_err(|error| {
                BridgeRuntimeError::new(format!("failed to flush WM response: {error}"))
            })?;
        }
    }
    Ok(())
}

fn read_wm_request(stream: &mut UnixStream) -> Result<Option<WmRequestPacket>, BridgeRuntimeError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(BridgeRuntimeError::new(format!(
                "failed to read WM request header: {error}"
            )));
        }
    }
    let payload_len = u32::from_le_bytes(header[16..20].try_into().expect("fixed header")) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(BridgeRuntimeError::new(format!(
            "WM request payload too large: {payload_len}"
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| BridgeRuntimeError::new(format!("failed to read WM payload: {error}")))?;
    decode_wm_request_frame(&frame)
        .map(Some)
        .map_err(|error| BridgeRuntimeError::new(format!("failed to decode WM request: {error:?}")))
}

fn bind_private_display() -> Result<(UnixListener, u16, PathBuf), BridgeRuntimeError> {
    fs::create_dir_all("/tmp/.X11-unix").map_err(|error| {
        BridgeRuntimeError::new(format!("failed to create /tmp/.X11-unix: {error}"))
    })?;
    for display in 90..200 {
        let path = PathBuf::from(format!("/tmp/.X11-unix/X{display}"));
        match UnixListener::bind(&path) {
            Ok(listener) => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|error| {
                    BridgeRuntimeError::new(format!(
                        "failed to secure private X socket {}: {error}",
                        path.display()
                    ))
                })?;
                return Ok((listener, display, path));
            }
            Err(error) if error.kind() == ErrorKind::AddrInUse => continue,
            Err(error) => {
                return Err(BridgeRuntimeError::new(format!(
                    "failed to bind private X socket {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Err(BridgeRuntimeError::new("no private X display available"))
}

fn send_engine_update(
    bridge: &X11WmBridgeState,
    update: &BridgeEngineUpdate,
    commands: &SyncSender<ServerCommand>,
) -> Result<BTreeSet<SyntheticXWindowId>, BridgeRuntimeError> {
    let mut expected = BTreeSet::new();
    for event in &update.events {
        let command = match *event {
            SyntheticXEvent::RootConfigured { bounds } => ServerCommand::Root(bounds),
            SyntheticXEvent::MapRequest { window } => {
                expected.insert(window);
                ServerCommand::Map(
                    window,
                    bridge
                        .synthetic_geometry(window)
                        .ok_or_else(|| BridgeRuntimeError::new("synthetic map has no geometry"))?,
                )
            }
            SyntheticXEvent::ConfigureNotify { window, geometry } => {
                expected.insert(window);
                ServerCommand::Configure(window, geometry)
            }
            SyntheticXEvent::UnmapNotify { window } => ServerCommand::Unmap(window),
            SyntheticXEvent::DestroyNotify { window } => ServerCommand::Destroy(window),
        };
        commands
            .send(command)
            .map_err(|_| BridgeRuntimeError::new("xmonad command channel disconnected"))?;
    }
    Ok(expected)
}

#[derive(Clone, Copy)]
struct WindowState {
    geometry: Rect,
    mapped: bool,
}

struct XServerState {
    sequence: u16,
    root: Rect,
    windows: BTreeMap<u32, WindowState>,
    atoms_by_name: BTreeMap<Vec<u8>, u32>,
    atom_names: BTreeMap<u32, Vec<u8>>,
    next_atom: u32,
    input_focus: u32,
}

impl XServerState {
    fn new() -> Self {
        Self {
            sequence: 0,
            root: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            windows: BTreeMap::new(),
            atoms_by_name: BTreeMap::new(),
            atom_names: BTreeMap::new(),
            next_atom: FIRST_DYNAMIC_ATOM,
            input_focus: SYNTHETIC_ROOT_XID,
        }
    }
}

fn serve_xmonad(
    stream: &mut UnixStream,
    commands: Receiver<ServerCommand>,
    legacy: SyncSender<LegacyWmRequest>,
) -> Result<(), BridgeRuntimeError> {
    let setup = serve_x11_setup_socket_client(stream)
        .map_err(|error| BridgeRuntimeError::new(format!("X11 setup failed: {error}")))?;
    if setup.byte_order != XByteOrder::LittleEndian {
        return Err(BridgeRuntimeError::new(
            "private xmonad server currently requires little-endian X11",
        ));
    }
    stream.set_read_timeout(Some(IO_POLL)).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to configure X11 socket timeout: {error}"))
    })?;
    let mut state = XServerState::new();
    loop {
        loop {
            match commands.try_recv() {
                Ok(command) => apply_server_command(stream, &mut state, command)?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let mut header = [0_u8; 4];
        match stream.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                continue;
            }
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => {
                return Err(BridgeRuntimeError::new(format!(
                    "failed to read xmonad request header: {error}"
                )));
            }
        }
        let units = usize::from(u16::from_le_bytes([header[2], header[3]]));
        if units == 0 || units > 65_535 {
            return Err(BridgeRuntimeError::new(format!(
                "invalid xmonad request length {units}"
            )));
        }
        let mut body = vec![0_u8; units * 4 - 4];
        stream.read_exact(&mut body).map_err(|error| {
            BridgeRuntimeError::new(format!("failed to read xmonad request body: {error}"))
        })?;
        state.sequence = state.sequence.wrapping_add(1);
        if std::env::var_os("SOPHIA_X11_WM_TRACE").is_some() {
            eprintln!(
                "sophia-x11-wm-bridge: seq={} opcode={}",
                state.sequence, header[0]
            );
        }
        dispatch_request(stream, &mut state, &legacy, header[0], header[1], &body)?;
    }
}

fn apply_server_command(
    stream: &mut UnixStream,
    state: &mut XServerState,
    command: ServerCommand,
) -> Result<(), BridgeRuntimeError> {
    match command {
        ServerCommand::Root(bounds) => state.root = bounds,
        ServerCommand::Map(window, geometry) => {
            state.windows.insert(
                window.raw(),
                WindowState {
                    geometry,
                    mapped: false,
                },
            );
            let mut event = vec![20, 0];
            push_u16(&mut event, state.sequence);
            push_u32(&mut event, SYNTHETIC_ROOT_XID);
            push_u32(&mut event, window.raw());
            event.resize(32, 0);
            write_packet(stream, &event)?;
        }
        ServerCommand::Configure(window, geometry) => {
            if let Some(entry) = state.windows.get_mut(&window.raw()) {
                entry.geometry = geometry;
            }
            // A root ConfigureNotify is the bounded, metadata-free signal that
            // makes xmonad re-run its current layout for an existing set.
            write_configure_notify(stream, state.sequence, SYNTHETIC_ROOT_XID, state.root)?;
        }
        ServerCommand::Unmap(window) => {
            state.windows.remove(&window.raw());
            write_window_event(stream, 18, window.raw())?;
        }
        ServerCommand::Destroy(window) => {
            state.windows.remove(&window.raw());
            write_window_event(stream, 17, window.raw())?;
        }
    }
    Ok(())
}

fn dispatch_request(
    stream: &mut UnixStream,
    state: &mut XServerState,
    legacy: &SyncSender<LegacyWmRequest>,
    opcode: u8,
    detail: u8,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    match opcode {
        2 => {}
        3 => reply_window_attributes(stream, state, read_u32(body, 0))?,
        8 => {
            let window = read_u32(body, 0);
            if let Some(entry) = state.windows.get_mut(&window) {
                entry.mapped = true;
            }
        }
        10 => {
            let window = read_u32(body, 0);
            if let Some(entry) = state.windows.get_mut(&window) {
                entry.mapped = false;
            }
        }
        12 => configure_window(state, legacy, body)?,
        14 => reply_geometry(stream, state, read_u32(body, 0))?,
        15 => reply_query_tree(stream, state)?,
        16 => reply_intern_atom(stream, state, detail != 0, body)?,
        17 => reply_atom_name(stream, state, read_u32(body, 0))?,
        18 | 19 => {}
        20 => reply_empty_property(stream, state.sequence)?,
        21 => reply_list_properties(stream, state.sequence)?,
        22 => {}
        23 => reply_u32(stream, state.sequence, 0, 0)?,
        25 | 28 | 29 | 30 | 32 | 33 | 34 | 35 | 36 | 37 | 39 | 41 => {}
        26 => reply_simple(stream, state.sequence, 0)?,
        31 => reply_simple(stream, state.sequence, 0)?,
        38 => reply_query_pointer(stream, state)?,
        40 => reply_translate_coordinates(stream, state)?,
        42 => {
            let window = read_u32(body, 0);
            state.input_focus = window;
            if let Some(window) = synthetic_id(state, window) {
                legacy
                    .send(LegacyWmRequest::FocusWindow { window })
                    .map_err(|_| BridgeRuntimeError::new("legacy request channel disconnected"))?;
            }
        }
        43 => reply_u32(stream, state.sequence, 0, state.input_focus)?,
        44 => reply_query_keymap(stream, state.sequence)?,
        53..=83 => {}
        84 => reply_alloc_color(stream, state.sequence, body)?,
        85 => reply_alloc_named_color(stream, state.sequence)?,
        91 => reply_best_size(stream, state, body)?,
        98 => reply_query_extension(stream, state.sequence)?,
        99 => reply_list_extensions(stream, state.sequence)?,
        101 => reply_keyboard_mapping(stream, state.sequence, detail, body)?,
        103 => reply_keyboard_control(stream, state.sequence)?,
        106 => reply_pointer_control(stream, state.sequence)?,
        108 => reply_screen_saver(stream, state.sequence)?,
        117 => reply_pointer_mapping(stream, state.sequence)?,
        119 => reply_modifier_mapping(stream, state.sequence)?,
        127 => {}
        other => {
            return Err(BridgeRuntimeError::new(format!(
                "unsupported xmonad core request opcode {other}"
            )));
        }
    }
    Ok(())
}

fn configure_window(
    state: &mut XServerState,
    legacy: &SyncSender<LegacyWmRequest>,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let raw = read_u32(body, 0);
    let mask = read_u16(body, 4);
    let Some(window) = synthetic_id(state, raw) else {
        return Ok(());
    };
    let entry = state.windows.get_mut(&raw).expect("known synthetic window");
    let mut geometry = entry.geometry;
    let mut cursor = 8;
    for bit in 0..7 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let value = read_u32(body, cursor);
        cursor += 4;
        match bit {
            0 => geometry.x = value as i32,
            1 => geometry.y = value as i32,
            2 => geometry.width = value as i32,
            3 => geometry.height = value as i32,
            _ => {}
        }
    }
    entry.geometry = geometry;
    legacy
        .send(LegacyWmRequest::ConfigureWindow {
            window,
            geometry,
            z_index: 0,
        })
        .map_err(|_| BridgeRuntimeError::new("legacy request channel disconnected"))
}

fn synthetic_id(state: &XServerState, raw: u32) -> Option<SyntheticXWindowId> {
    state
        .windows
        .contains_key(&raw)
        .then_some(SyntheticXWindowId(raw))
}

fn reply_window_attributes(
    stream: &mut UnixStream,
    state: &XServerState,
    window: u32,
) -> Result<(), BridgeRuntimeError> {
    let mapped = state
        .windows
        .get(&window)
        .is_some_and(|window| window.mapped);
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 3);
    push_u32(&mut reply, sophia_x_authority::X_SETUP_DEFAULT_VISUAL);
    push_u16(&mut reply, 1);
    reply.extend_from_slice(&[0, 0]);
    push_u32(&mut reply, u32::MAX);
    push_u32(&mut reply, 0);
    reply.extend_from_slice(&[0, 1, if mapped { 2 } else { 0 }, 0]);
    push_u32(&mut reply, sophia_x_authority::X_SETUP_DEFAULT_COLORMAP);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, 0);
    reply.resize(44, 0);
    write_packet(stream, &reply)
}

fn reply_geometry(
    stream: &mut UnixStream,
    state: &XServerState,
    window: u32,
) -> Result<(), BridgeRuntimeError> {
    let geometry = if window == SYNTHETIC_ROOT_XID {
        state.root
    } else {
        state
            .windows
            .get(&window)
            .map(|window| window.geometry)
            .unwrap_or(state.root)
    };
    let mut reply = vec![1, 24];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, SYNTHETIC_ROOT_XID);
    push_i16(&mut reply, geometry.x as i16);
    push_i16(&mut reply, geometry.y as i16);
    push_u16(&mut reply, geometry.width as u16);
    push_u16(&mut reply, geometry.height as u16);
    push_u16(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_query_tree(
    stream: &mut UnixStream,
    state: &XServerState,
) -> Result<(), BridgeRuntimeError> {
    let children = state.windows.keys().copied().collect::<Vec<_>>();
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, children.len() as u32);
    push_u32(&mut reply, SYNTHETIC_ROOT_XID);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, children.len() as u16);
    reply.resize(32, 0);
    for child in children {
        push_u32(&mut reply, child);
    }
    write_packet(stream, &reply)
}

fn reply_intern_atom(
    stream: &mut UnixStream,
    state: &mut XServerState,
    only_if_exists: bool,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let len = usize::from(read_u16(body, 0));
    let name = body
        .get(4..4 + len)
        .ok_or_else(|| BridgeRuntimeError::new("truncated InternAtom name"))?
        .to_vec();
    let atom = if let Some(atom) = state.atoms_by_name.get(&name) {
        *atom
    } else if only_if_exists {
        0
    } else {
        let atom = state.next_atom;
        state.next_atom = state.next_atom.saturating_add(1);
        state.atoms_by_name.insert(name.clone(), atom);
        state.atom_names.insert(atom, name);
        atom
    };
    reply_u32(stream, state.sequence, 0, atom)
}

fn reply_atom_name(
    stream: &mut UnixStream,
    state: &XServerState,
    atom: u32,
) -> Result<(), BridgeRuntimeError> {
    let name = state
        .atom_names
        .get(&atom)
        .map(Vec::as_slice)
        .unwrap_or(b"");
    let padded = (name.len() + 3) & !3;
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, (padded / 4) as u32);
    push_u16(&mut reply, name.len() as u16);
    reply.resize(32, 0);
    reply.extend_from_slice(name);
    reply.resize(32 + padded, 0);
    write_packet(stream, &reply)
}

fn reply_empty_property(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_list_properties(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    reply_simple(stream, sequence, 0)
}

fn reply_query_pointer(
    stream: &mut UnixStream,
    state: &XServerState,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 1];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, SYNTHETIC_ROOT_XID);
    push_u32(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_u16(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_translate_coordinates(
    stream: &mut UnixStream,
    state: &XServerState,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 1];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    push_i16(&mut reply, 0);
    push_i16(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_query_keymap(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 2);
    reply.resize(40, 0);
    write_packet(stream, &reply)
}

fn reply_alloc_color(
    stream: &mut UnixStream,
    sequence: u16,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let red = read_u16(body, 4);
    let green = read_u16(body, 6);
    let blue = read_u16(body, 8);
    let pixel = (u32::from(red >> 8) << 16) | (u32::from(green >> 8) << 8) | u32::from(blue >> 8);
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, red);
    push_u16(&mut reply, green);
    push_u16(&mut reply, blue);
    reply.resize(20, 0);
    push_u32(&mut reply, pixel);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_alloc_named_color(
    stream: &mut UnixStream,
    sequence: u16,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_best_size(
    stream: &mut UnixStream,
    state: &XServerState,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, state.sequence);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, read_u16(body, 4));
    push_u16(&mut reply, read_u16(body, 6));
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_query_extension(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_list_extensions(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    reply_simple(stream, sequence, 0)
}

fn reply_keyboard_mapping(
    stream: &mut UnixStream,
    sequence: u16,
    first_keycode: u8,
    body: &[u8],
) -> Result<(), BridgeRuntimeError> {
    let count = usize::from(body.first().copied().unwrap_or(0));
    let mut reply = vec![1, 1];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, count as u32);
    reply.resize(32, 0);
    for keycode in first_keycode..first_keycode.saturating_add(count as u8) {
        push_u32(&mut reply, u32::from(keycode));
    }
    write_packet(stream, &reply)
}

fn reply_keyboard_control(
    stream: &mut UnixStream,
    sequence: u16,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 5);
    reply.resize(52, 0);
    write_packet(stream, &reply)
}

fn reply_pointer_control(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u16(&mut reply, 1);
    push_u16(&mut reply, 1);
    push_u16(&mut reply, 4);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_screen_saver(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_pointer_mapping(stream: &mut UnixStream, sequence: u16) -> Result<(), BridgeRuntimeError> {
    reply_simple(stream, sequence, 0)
}

fn reply_modifier_mapping(
    stream: &mut UnixStream,
    sequence: u16,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, 0];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_simple(
    stream: &mut UnixStream,
    sequence: u16,
    detail: u8,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, detail];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn reply_u32(
    stream: &mut UnixStream,
    sequence: u16,
    detail: u8,
    value: u32,
) -> Result<(), BridgeRuntimeError> {
    let mut reply = vec![1, detail];
    push_u16(&mut reply, sequence);
    push_u32(&mut reply, 0);
    push_u32(&mut reply, value);
    reply.resize(32, 0);
    write_packet(stream, &reply)
}

fn write_configure_notify(
    stream: &mut UnixStream,
    sequence: u16,
    window: u32,
    geometry: Rect,
) -> Result<(), BridgeRuntimeError> {
    let mut event = vec![22, 0];
    push_u16(&mut event, sequence);
    push_u32(&mut event, window);
    push_u32(&mut event, window);
    push_u32(&mut event, 0);
    push_i16(&mut event, geometry.x as i16);
    push_i16(&mut event, geometry.y as i16);
    push_u16(&mut event, geometry.width as u16);
    push_u16(&mut event, geometry.height as u16);
    push_u16(&mut event, 0);
    event.resize(32, 0);
    write_packet(stream, &event)
}

fn write_window_event(
    stream: &mut UnixStream,
    event_type: u8,
    window: u32,
) -> Result<(), BridgeRuntimeError> {
    let mut event = vec![event_type, 0, 0, 0];
    push_u32(&mut event, window);
    push_u32(&mut event, window);
    event.resize(32, 0);
    write_packet(stream, &event)
}

fn write_packet(stream: &mut UnixStream, bytes: &[u8]) -> Result<(), BridgeRuntimeError> {
    stream.write_all(bytes).map_err(|error| {
        BridgeRuntimeError::new(format!("failed to write xmonad packet: {error}"))
    })?;
    stream
        .flush()
        .map_err(|error| BridgeRuntimeError::new(format!("failed to flush xmonad packet: {error}")))
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .unwrap_or(0)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0)
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn push_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

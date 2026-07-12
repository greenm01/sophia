#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
#[cfg(unix)]
use std::{
    collections::BTreeMap,
    io::{ErrorKind, Read, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    },
    time::Duration,
};

#[cfg(unix)]
use crate::{
    X_SETUP_CLIENT_PREFIX_LEN, X_SETUP_DEFAULT_ROOT, X_SETUP_MAX_AUTH_FIELD_LEN, XAtomTable,
    XAuthorityCpuBufferSnapshot, XAuthorityObservedTransactionBatch, XAuthorityRuntime, XByteOrder,
    XClientEvent, XDispatchContext, XDispatchResult, XPropertyTable, XResourceId, XSetupRequest,
    XSetupSuccess, XWireClientContext, decode_x11_core_request, dispatch_x11_parse_error,
    dispatch_x11_wire_request, encode_x_client_event, encode_x11_setup_success,
    parse_x11_setup_request, try_emit_x_authority_trace, try_emit_x_authority_transactions,
    x11_setup_request_total_len,
};
#[cfg(unix)]
use sophia_protocol::{NamespaceId, SurfaceId, TransactionId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X11SetupSocketError {
    message: String,
}

impl X11SetupSocketError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl core::fmt::Display for X11SetupSocketError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for X11SetupSocketError {}

#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct X11CoreDispatchTrace<'a> {
    pub sequence: u16,
    pub major_opcode: u8,
    pub request_detail: Option<String>,
    pub parse_error: Option<String>,
    pub result: &'a XDispatchResult,
    pub cpu_buffer_update: Option<&'a XAuthorityCpuBufferSnapshot>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityKeyEvent {
    pub keycode: u8,
    pub pressed: bool,
    pub state: u16,
    pub time_msec: u32,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityPointerEventKind {
    Motion,
    Button { button: u8, pressed: bool },
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityPointerEvent {
    pub kind: XAuthorityPointerEventKind,
    pub surface: SurfaceId,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: u16,
    pub time_msec: u32,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityInputEvent {
    Key(XAuthorityKeyEvent),
    Pointer(XAuthorityPointerEvent),
}

#[cfg(unix)]
impl From<XAuthorityKeyEvent> for XAuthorityInputEvent {
    fn from(event: XAuthorityKeyEvent) -> Self {
        Self::Key(event)
    }
}

/// Authority-owned state shared by every client accepted by one X11 socket
/// listener. Client sequence numbers remain connection-local.
#[cfg(unix)]
#[derive(Debug, Default)]
pub struct X11CoreSocketServerState {
    runtime: XAuthorityRuntime,
    atoms: XAtomTable,
    properties: XPropertyTable,
}

#[cfg(unix)]
impl X11CoreSocketServerState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(unix)]
pub fn run_x11_setup_socket_server_once(path: impl AsRef<Path>) -> Result<(), X11SetupSocketError> {
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(X11SetupSocketError::new(format!(
                "failed to remove stale X11 setup socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to bind X11 setup socket {}: {error}",
            path.display()
        ))
    })?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to accept X11 setup client on {}: {error}",
            path.display()
        ))
    })?;
    serve_x11_setup_socket_client(&mut stream).map(|_| ())
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_observed(path, namespace, |_| {})
}

/// Runs one X11 authority listener until its enclosing process is stopped.
///
/// Clients are served sequentially and share one authority state. Concurrent
/// multi-client dispatch and client-specific resource allocation remain a
/// separate milestone.
#[cfg(unix)]
pub fn run_x11_core_socket_server(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_observed(path, namespace, |_| {})
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_observed(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult),
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_traced(path, namespace, move |trace| {
        observer(trace.result);
        Ok(())
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_traced(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(path)?;
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_listener_traced(&listener, namespace, &mut state, observer)
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_channel(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_traced(path, namespace, move |trace| {
        try_emit_x_authority_trace(&sender, &trace)
            .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        Ok(())
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_observed(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult),
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_traced(path, namespace, move |trace| {
        let result = trace.result;
        observer(result);
        Ok(())
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_traced(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_trace_observer(path, namespace, None, observer)
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_traced_with_idle_timeout(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    idle_timeout: Duration,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_trace_observer(
        path,
        namespace,
        Some(idle_timeout),
        observer,
    )
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_channel(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_observer(path, namespace, move |result| {
        Ok(try_emit_x_authority_transactions(&sender, result).map(|_| ())?)
    })
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_channels(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    input_receiver: Receiver<XAuthorityInputEvent>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(path)?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!("failed to accept X11 core client: {error}"))
    })?;
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_trace_observer_and_input(
        &mut stream,
        namespace,
        &mut state,
        Some(input_receiver),
        move |trace| {
            try_emit_x_authority_trace(&transaction_sender, &trace)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            Ok(())
        },
    )
}

#[cfg(unix)]
fn run_x11_core_socket_server_once_with_observer(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    run_x11_core_socket_server_once_with_trace_observer(path, namespace, None, move |trace| {
        observer(trace.result)
    })
}

#[cfg(unix)]
fn run_x11_core_socket_server_once_with_trace_observer(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    idle_timeout: Option<Duration>,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(path)?;
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_listener_once_with_trace_observer(
        &listener,
        namespace,
        &mut state,
        idle_timeout,
        observer,
    )
}

#[cfg(unix)]
pub fn bind_x11_core_socket_server(
    path: impl AsRef<Path>,
) -> Result<UnixListener, X11SetupSocketError> {
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(X11SetupSocketError::new(format!(
                "failed to remove stale X11 core socket {}: {error}",
                path.display()
            )));
        }
    }

    UnixListener::bind(path).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to bind X11 core socket {}: {error}",
            path.display()
        ))
    })
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_once(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_once_traced(listener, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_once_traced(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_once_with_trace_observer(
        listener, namespace, state, None, observer,
    )
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_traced(listener, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_traced(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    mut observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    loop {
        serve_x11_core_socket_listener_once_with_trace_observer(
            listener,
            namespace,
            state,
            None,
            &mut observer,
        )?;
    }
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_once_with_trace_observer(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    idle_timeout: Option<Duration>,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!("failed to accept X11 core client: {error}"))
    })?;
    if let Some(timeout) = idle_timeout {
        stream.set_read_timeout(Some(timeout)).map_err(|error| {
            X11SetupSocketError::new(format!("failed to set X11 core read timeout: {error}"))
        })?;
    }
    serve_x11_core_socket_client_with_trace_observer(&mut stream, namespace, state, observer)
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let request = read_x11_setup_request(stream)?;
    let response =
        encode_x11_setup_success(request.byte_order, &XSetupSuccess::client_compatible()).map_err(
            |error| {
                X11SetupSocketError::new(format!("failed to encode X11 setup success: {error}"))
            },
        )?;
    stream
        .write_all(&response)
        .map_err(|error| X11SetupSocketError::new(format!("failed to write X11 setup: {error}")))?;
    stream
        .flush()
        .map_err(|error| X11SetupSocketError::new(format!("failed to flush X11 setup: {error}")))?;
    Ok(request)
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client(
    stream: &mut UnixStream,
    namespace: NamespaceId,
) -> Result<(), X11SetupSocketError> {
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_state(stream, namespace, &mut state)
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client_with_state(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer(stream, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client_observed(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    mut observer: impl FnMut(&XDispatchResult),
) -> Result<(), X11SetupSocketError> {
    let mut state = X11CoreSocketServerState::new();
    serve_x11_core_socket_client_with_state_observed(stream, namespace, &mut state, move |result| {
        observer(result);
        Ok(())
    })
}

#[cfg(unix)]
pub fn serve_x11_core_socket_client_with_state_observed(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    mut observer: impl FnMut(&XDispatchResult) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer(stream, namespace, state, move |trace| {
        observer(trace.result)
    })
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer_and_input(
        stream, namespace, state, None, observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_input(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    input_receiver: Option<Receiver<XAuthorityInputEvent>>,
    mut observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let setup = serve_x11_setup_socket_client(stream)?;
    let mut sequence = 0u16;
    let event_sequence = Arc::new(AtomicU16::new(0));
    let focused_window = Arc::new(AtomicU64::new(u64::from(X_SETUP_DEFAULT_ROOT)));
    let mut keyboard_target_selected = false;
    let surface_windows = Arc::new(Mutex::new(BTreeMap::new()));
    let output_stream = Arc::new(Mutex::new(stream.try_clone().map_err(|error| {
        X11SetupSocketError::new(format!("failed to clone X11 output socket: {error}"))
    })?));
    let input_writer = input_receiver
        .map(|receiver| {
            spawn_x11_input_event_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                focused_window.clone(),
                surface_windows.clone(),
                receiver,
            )
        })
        .transpose()?;

    let result = (|| {
        while let Some((major_opcode, request)) = read_x11_core_request(stream, setup.byte_order)? {
            sequence = sequence.wrapping_add(1);
            event_sequence.store(sequence, Ordering::Release);
            if let Some(window) = x11_keyboard_event_target(&request, setup.byte_order) {
                focused_window.store(window.local.raw(), Ordering::Release);
                keyboard_target_selected = true;
            }
            let dispatch_context = XDispatchContext {
                byte_order: setup.byte_order,
                namespace,
                sequence,
                major_opcode,
            };
            let mut parse_error = None;
            let mut request_detail = None;
            let output = match decode_x11_core_request(
                XWireClientContext {
                    byte_order: setup.byte_order,
                    namespace,
                    transaction: TransactionId::from_raw(u64::from(sequence)),
                },
                &request,
            ) {
                Ok(request) => {
                    if let crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
                        kind:
                            crate::XAuthorityRequestKind::CreateWindow {
                                window, surface, ..
                            },
                        ..
                    }) = &request
                    {
                        surface_windows
                            .lock()
                            .map_err(|_| {
                                X11SetupSocketError::new("X11 surface/window map lock poisoned")
                            })?
                            .insert(*surface, *window);
                    }
                    if !keyboard_target_selected
                        && let crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
                            kind: crate::XAuthorityRequestKind::MapWindow { window, .. },
                            ..
                        }) = &request
                    {
                        focused_window.store(window.local.raw(), Ordering::Release);
                    }
                    request_detail = x11_core_request_trace_detail(&request);
                    dispatch_x11_wire_request(
                        dispatch_context,
                        request,
                        &mut state.runtime,
                        &mut state.atoms,
                        &mut state.properties,
                    )
                }
                Err(error) => {
                    let head = request
                        .iter()
                        .take(24)
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join("");
                    parse_error = Some(format!("{error:?}:len={}:head={head}", request.len()));
                    dispatch_x11_parse_error(dispatch_context, error)
                }
            };
            let cpu_buffer_update = state.runtime.take_cpu_buffer_update();
            observer(X11CoreDispatchTrace {
                sequence,
                major_opcode,
                request_detail,
                parse_error,
                result: &output,
                cpu_buffer_update: cpu_buffer_update.as_ref(),
            })?;
            let mut output_stream = output_stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            for record in output.encoded_outputs(setup.byte_order) {
                if let Err(error) = output_stream.write_all(&record) {
                    if is_x11_client_disconnect(&error) {
                        return Ok(());
                    }
                    return Err(X11SetupSocketError::new(format!(
                        "failed to write X11 output: {error}"
                    )));
                }
            }
            if let Err(error) = output_stream.flush() {
                if matches!(
                    error.kind(),
                    ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof
                ) {
                    return Ok(());
                }
                return Err(X11SetupSocketError::new(format!(
                    "failed to flush X11 output: {error}"
                )));
            }
        }
        Ok(())
    })();

    if let Some(writer) = input_writer {
        writer.stop.store(true, Ordering::Release);
        let writer_result = writer
            .thread
            .join()
            .map_err(|_| X11SetupSocketError::new("X11 input event writer thread panicked"))?;
        result?;
        writer_result
    } else {
        result
    }
}

#[cfg(unix)]
fn x11_keyboard_event_target(request: &[u8], byte_order: XByteOrder) -> Option<XResourceId> {
    const X_CREATE_WINDOW: u8 = 1;
    const X_CHANGE_WINDOW_ATTRIBUTES: u8 = 2;
    const X_CW_EVENT_MASK: u32 = 1 << 11;
    const X_KEY_EVENT_MASKS: u32 = (1 << 0) | (1 << 1);

    let (value_mask_offset, values_offset) = match request.first().copied()? {
        X_CREATE_WINDOW if request.len() >= 32 => (28, 32),
        X_CHANGE_WINDOW_ATTRIBUTES if request.len() >= 12 => (8, 12),
        _ => return None,
    };
    let value_mask = byte_order.u32(&request[value_mask_offset..value_mask_offset + 4]);
    if value_mask & X_CW_EVENT_MASK == 0 {
        return None;
    }
    let preceding_values = (value_mask & (X_CW_EVENT_MASK - 1)).count_ones() as usize;
    let event_mask_offset = values_offset + preceding_values.saturating_mul(4);
    if event_mask_offset + 4 > request.len() {
        return None;
    }
    let event_mask = byte_order.u32(&request[event_mask_offset..event_mask_offset + 4]);
    if event_mask & X_KEY_EVENT_MASKS == 0 {
        return None;
    }
    Some(XResourceId::new(
        u64::from(byte_order.u32(&request[4..8])),
        1,
    ))
}

#[cfg(unix)]
struct X11InputEventWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}

#[cfg(unix)]
fn spawn_x11_input_event_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_window: Arc<AtomicU64>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    receiver: Receiver<XAuthorityInputEvent>,
) -> Result<X11InputEventWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut focus_sent_to = None;
        while !writer_stop.load(Ordering::Acquire) {
            let event = match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let focused_window = XResourceId::new(focused_window.load(Ordering::Acquire), 1);
            let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
            let sequence = sequence.load(Ordering::Acquire);
            let record = encode_x_client_event(
                byte_order,
                match event {
                    XAuthorityInputEvent::Key(event) => XClientEvent::Key {
                        sequence,
                        pressed: event.pressed,
                        keycode: event.keycode,
                        time: event.time_msec,
                        root,
                        event: focused_window,
                        state: event.state,
                    },
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Motion,
                        surface,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerMotion {
                        sequence,
                        time: time_msec,
                        root,
                        event: *surface_windows
                            .lock()
                            .map_err(|_| {
                                X11SetupSocketError::new("X11 surface/window map lock poisoned")
                            })?
                            .get(&surface)
                            .ok_or_else(|| {
                                X11SetupSocketError::new("X11 pointer target surface is unknown")
                            })?,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                    XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                        kind: XAuthorityPointerEventKind::Button { button, pressed },
                        surface,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                        time_msec,
                    }) => XClientEvent::PointerButton {
                        sequence,
                        pressed,
                        button,
                        time: time_msec,
                        root,
                        event: *surface_windows
                            .lock()
                            .map_err(|_| {
                                X11SetupSocketError::new("X11 surface/window map lock poisoned")
                            })?
                            .get(&surface)
                            .ok_or_else(|| {
                                X11SetupSocketError::new("X11 pointer target surface is unknown")
                            })?,
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                },
            );
            let mut stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            if matches!(event, XAuthorityInputEvent::Key(_))
                && focus_sent_to != Some(focused_window)
            {
                let focus = encode_x_client_event(
                    byte_order,
                    XClientEvent::Focus {
                        sequence,
                        focused: true,
                        detail: 3,
                        event: focused_window,
                        mode: 0,
                    },
                );
                stream.write_all(&focus).map_err(|error| {
                    X11SetupSocketError::new(format!("failed to write X11 focus event: {error}"))
                })?;
                focus_sent_to = Some(focused_window);
            }
            if let Err(error) = stream.write_all(&record) {
                if is_x11_client_disconnect(&error) {
                    return Ok(());
                }
                return Err(X11SetupSocketError::new(format!(
                    "failed to write X11 input event: {error}"
                )));
            }
            stream.flush().map_err(|error| {
                X11SetupSocketError::new(format!("failed to flush X11 input event: {error}"))
            })?;
        }
        Ok(())
    });
    Ok(X11InputEventWriter { stop, thread })
}

#[cfg(unix)]
fn is_x11_client_disconnect(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof
    )
}

#[cfg(unix)]
fn x11_core_request_trace_detail(request: &crate::XWireRequest) -> Option<String> {
    match request {
        crate::XWireRequest::Authority(packet) => match &packet.kind {
            crate::XAuthorityRequestKind::CreateWindow {
                window, geometry, ..
            } => Some(format!(
                "CreateWindow:window={:#x}:{}x{}+{}+{}",
                window.local.raw(),
                geometry.width,
                geometry.height,
                geometry.x,
                geometry.y
            )),
            crate::XAuthorityRequestKind::MapWindow { window, .. } => {
                Some(format!("MapWindow:window={:#x}", window.local.raw()))
            }
            crate::XAuthorityRequestKind::PresentPixmap { window, pixmap, .. } => Some(format!(
                "SOPHIA-PRESENT:PresentPixmap:window={:#x}:pixmap={pixmap:#x}",
                window.local.raw()
            )),
            crate::XAuthorityRequestKind::SetSelectionOwner { selection, .. } => {
                Some(format!("SetSelectionOwner:{selection}"))
            }
            crate::XAuthorityRequestKind::RequestSelection {
                requestor,
                target_name,
                ..
            } => Some(format!(
                "RequestSelection:requestor={:#x}:target={target_name}",
                requestor.local.raw()
            )),
        },
        crate::XWireRequest::QueryExtension { name } => Some(format!("QueryExtension:{name}")),
        crate::XWireRequest::InternAtom { name, .. } => Some(format!("InternAtom:{name}")),
        crate::XWireRequest::ChangeWindowAttributes { window } => Some(format!(
            "ChangeWindowAttributes:window={:#x}",
            window.local.raw()
        )),
        crate::XWireRequest::ConfigureWindow {
            window,
            value_mask,
            x,
            y,
            width,
            height,
        } => Some(format!(
            "ConfigureWindow:window={:#x}:mask={value_mask:#x}:x={x:?}:y={y:?}:width={width:?}:height={height:?}",
            window.local.raw()
        )),
        crate::XWireRequest::ChangeProperty(change) => Some(format!(
            "ChangeProperty:window={:#x}:property={}",
            change.window.local.raw(),
            change.property
        )),
        crate::XWireRequest::GetProperty(read) => Some(format!(
            "GetProperty:window={:#x}:property={}",
            read.window.local.raw(),
            read.property
        )),
        crate::XWireRequest::CreateGraphicsContext { gc, drawable } => Some(format!(
            "CreateGC:gc={:#x}:drawable={:#x}",
            gc.local.raw(),
            drawable.local.raw()
        )),
        crate::XWireRequest::CreatePixmap {
            pixmap,
            drawable,
            width,
            height,
            ..
        } => Some(format!(
            "CreatePixmap:pixmap={:#x}:drawable={:#x}:{}x{}",
            pixmap.local.raw(),
            drawable.local.raw(),
            width,
            height
        )),
        crate::XWireRequest::PutImage {
            drawable,
            width,
            height,
            dst_x,
            dst_y,
            ..
        } => Some(format!(
            "PutImage:drawable={:#x}:{}x{}+{}+{}",
            drawable.local.raw(),
            width,
            height,
            dst_x,
            dst_y
        )),
        crate::XWireRequest::ImageText8 {
            drawable,
            x,
            y,
            text,
            ..
        } => Some(format!(
            "ImageText8:drawable={:#x}:glyphs={}+{x}+{y}",
            drawable.local.raw(),
            text.len()
        )),
        crate::XWireRequest::CopyArea {
            source,
            destination,
            width,
            height,
            dst_x,
            dst_y,
            ..
        } => Some(format!(
            "CopyArea:source={:#x}:destination={:#x}:{}x{}+{}+{}",
            source.local.raw(),
            destination.local.raw(),
            width,
            height,
            dst_x,
            dst_y
        )),
        crate::XWireRequest::OpenFont { name, .. } => Some(format!("OpenFont:{name}")),
        crate::XWireRequest::QueryFont { font } => {
            Some(format!("QueryFont:font={:#x}", font.local.raw()))
        }
        crate::XWireRequest::CloseFont { font } => {
            Some(format!("CloseFont:font={:#x}", font.local.raw()))
        }
        crate::XWireRequest::CreateGlyphCursor { cursor, .. } => Some(format!(
            "CreateGlyphCursor:cursor={:#x}",
            cursor.local.raw()
        )),
        crate::XWireRequest::RecolorCursor { cursor } => {
            Some(format!("RecolorCursor:cursor={:#x}", cursor.local.raw()))
        }
        crate::XWireRequest::GetModifierMapping => Some("GetModifierMapping".to_owned()),
        crate::XWireRequest::GetKeyboardMapping {
            first_keycode,
            count,
        } => Some(format!(
            "GetKeyboardMapping:first_keycode={first_keycode}:count={count}"
        )),
        crate::XWireRequest::GetSelectionOwner { selection } => {
            Some(format!("GetSelectionOwner:{selection}"))
        }
        crate::XWireRequest::GrabButton {
            window,
            event_mask,
            button,
            modifiers,
            owner_events,
        } => Some(format!(
            "GrabButton:window={:#x}:button={button}:modifiers={modifiers:#x}:event_mask={event_mask:#x}:owner_events={owner_events}",
            window.local.raw()
        )),
        crate::XWireRequest::UngrabButton {
            window,
            button,
            modifiers,
        } => Some(format!(
            "UngrabButton:window={:#x}:button={button}:modifiers={modifiers:#x}",
            window.local.raw()
        )),
        crate::XWireRequest::CreateColormap {
            colormap,
            window,
            visual,
            ..
        } => Some(format!(
            "CreateColormap:colormap={:#x}:window={:#x}:visual={visual:#x}",
            colormap.local.raw(),
            window.local.raw()
        )),
        crate::XWireRequest::AllocColor {
            colormap,
            red,
            green,
            blue,
        } => Some(format!(
            "AllocColor:colormap={:#x}:rgb={red:#06x},{green:#06x},{blue:#06x}",
            colormap.local.raw()
        )),
        crate::XWireRequest::ShmQueryVersion => Some("MIT-SHM:QueryVersion".to_string()),
        crate::XWireRequest::ShmAttach { segment, .. } => {
            Some(format!("MIT-SHM:Attach:{:#x}", segment.local.raw()))
        }
        crate::XWireRequest::ShmDetach { segment } => {
            Some(format!("MIT-SHM:Detach:{:#x}", segment.local.raw()))
        }
        crate::XWireRequest::ShmPutImage {
            drawable, segment, ..
        } => Some(format!(
            "MIT-SHM:PutImage:drawable={:#x}:segment={:#x}",
            drawable.local.raw(),
            segment.local.raw()
        )),
        crate::XWireRequest::RandrQueryVersion { .. } => Some("RANDR:QueryVersion".to_string()),
        crate::XWireRequest::RandrSelectInput { window, .. } => {
            Some(format!("RANDR:SelectInput:{:#x}", window.local.raw()))
        }
        crate::XWireRequest::RandrGetOutputPrimary { window } => {
            Some(format!("RANDR:GetOutputPrimary:{:#x}", window.local.raw()))
        }
        crate::XWireRequest::RandrGetMonitors { window, .. } => {
            Some(format!("RANDR:GetMonitors:{:#x}", window.local.raw()))
        }
        crate::XWireRequest::XkbUseExtension { .. } => Some("XKEYBOARD:UseExtension".to_string()),
        crate::XWireRequest::BigRequestsEnable => Some("BIG-REQUESTS:Enable".to_string()),
        _ => None,
    }
}

#[cfg(unix)]
impl From<crate::XAuthorityTransportError> for X11SetupSocketError {
    fn from(error: crate::XAuthorityTransportError) -> Self {
        Self::new(error.to_string())
    }
}

#[cfg(unix)]
pub fn read_x11_setup_request(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let mut bytes = vec![0; X_SETUP_CLIENT_PREFIX_LEN];
    stream.read_exact(&mut bytes).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 setup prefix: {error}"))
    })?;
    let total_len = x11_setup_request_total_len(&bytes)
        .map_err(|error| X11SetupSocketError::new(format!("invalid X11 setup prefix: {error}")))?;
    bytes.resize(total_len, 0);
    stream
        .read_exact(&mut bytes[X_SETUP_CLIENT_PREFIX_LEN..])
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to read X11 setup auth fields: {error}"))
        })?;
    parse_x11_setup_request(&bytes)
        .map_err(|error| X11SetupSocketError::new(format!("invalid X11 setup request: {error}")))
}

#[cfg(unix)]
fn read_x11_core_request(
    stream: &mut UnixStream,
    byte_order: crate::XByteOrder,
) -> Result<Option<(u8, Vec<u8>)>, X11SetupSocketError> {
    let mut header = [0; 4];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionReset
                    | ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
            ) =>
        {
            return Ok(None);
        }
        Err(error) => {
            return Err(X11SetupSocketError::new(format!(
                "failed to read X11 request header: {error}"
            )));
        }
    }

    let length = usize::from(byte_order.u16(&header[2..4])) * 4;
    if length < 4 {
        return Ok(Some((header[0], header.to_vec())));
    }
    let max_len = X_SETUP_MAX_AUTH_FIELD_LEN * 64;
    if length > max_len {
        return Err(X11SetupSocketError::new(format!(
            "X11 request payload too large: {length}"
        )));
    }

    let mut request = Vec::with_capacity(length);
    request.extend_from_slice(&header);
    request.resize(length, 0);
    stream.read_exact(&mut request[4..]).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 request payload: {error}"))
    })?;

    Ok(Some((header[0], request)))
}

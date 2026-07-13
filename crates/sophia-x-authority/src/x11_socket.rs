#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
#[cfg(unix)]
use std::{
    collections::BTreeMap,
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    },
    time::Duration,
};

#[cfg(unix)]
use crate::{
    X_SETUP_CLIENT_PREFIX_LEN, X_SETUP_DEFAULT_RESOURCE_ID_MASK, X_SETUP_DEFAULT_ROOT,
    X_SETUP_MAX_AUTH_FIELD_LEN, XAtomTable, XAuthorityCpuBufferUpdate,
    XAuthorityObservedTransactionBatch, XAuthorityRuntime, XByteOrder, XClientEvent,
    XDispatchContext, XDispatchResult, XPropertyTable, XResourceId, XSetupFailure, XSetupRequest,
    XSetupSuccess, XWireClientContext, decode_x11_core_request, dispatch_x11_parse_error,
    dispatch_x11_wire_request, encode_x_client_event, encode_x11_setup_failure,
    encode_x11_setup_success, parse_x11_setup_request, try_emit_x_authority_trace,
    try_emit_x_authority_transactions, x11_setup_request_total_len,
};
#[cfg(unix)]
use sophia_protocol::{NamespaceId, Size, SurfaceId, TransactionId};

#[cfg(unix)]
const X11_CLIENT_RESOURCE_RANGE_SIZE: u32 = X_SETUP_DEFAULT_RESOURCE_ID_MASK + 1;
#[cfg(unix)]
const X11_MAX_CLIENT_RESOURCE_RANGES: u16 = (u32::MAX / X11_CLIENT_RESOURCE_RANGE_SIZE) as u16;

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

/// The X11 setup authorization policy for one local frontend listener.
///
/// `UnauthenticatedLocal` retains the bounded smoke-helper behavior and relies
/// on the listener's owner-only Unix-socket permissions. Production callers
/// should instead provide a session-scoped MIT-MAGIC-COOKIE-1 value.
#[cfg(unix)]
#[derive(Clone, Eq, PartialEq)]
pub enum XServerFrontendSetupAuthorization {
    UnauthenticatedLocal,
    MitMagicCookie([u8; 16]),
}

#[cfg(unix)]
impl Default for XServerFrontendSetupAuthorization {
    fn default() -> Self {
        Self::UnauthenticatedLocal
    }
}

#[cfg(unix)]
impl core::fmt::Debug for XServerFrontendSetupAuthorization {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnauthenticatedLocal => formatter.write_str("UnauthenticatedLocal"),
            Self::MitMagicCookie(_) => formatter.write_str("MitMagicCookie([redacted])"),
        }
    }
}

#[cfg(unix)]
impl XServerFrontendSetupAuthorization {
    fn permits(&self, request: &XSetupRequest) -> bool {
        match self {
            Self::UnauthenticatedLocal => true,
            Self::MitMagicCookie(expected) => {
                request.authorization_protocol_name == b"MIT-MAGIC-COOKIE-1"
                    && x11_authorization_data_eq(&request.authorization_data, expected)
            }
        }
    }
}

#[cfg(unix)]
fn x11_authorization_data_eq(actual: &[u8], expected: &[u8]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .fold(0u8, |difference, (actual, expected)| {
                difference | (actual ^ expected)
            })
            == 0
}

/// Configuration owned by one local Sophia X Server Frontend listener.
///
/// This deliberately describes only the boundary that exists today: one
/// owner-only Unix socket, one Sophia namespace, and explicit setup
/// authorization. Output/RandR facts and multi-client resource allocation are
/// explicit follow-up work rather than implicit defaults hidden in a smoke
/// helper.
#[cfg(unix)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XServerFrontendConfig {
    socket_path: PathBuf,
    namespace: NamespaceId,
    setup_authorization: XServerFrontendSetupAuthorization,
}

#[cfg(unix)]
impl XServerFrontendConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        namespace: NamespaceId,
    ) -> Result<Self, X11SetupSocketError> {
        let socket_path = socket_path.into();
        if socket_path.as_os_str().is_empty() {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend socket path must not be empty",
            ));
        }
        if !namespace.is_valid() {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend namespace must be valid",
            ));
        }
        Ok(Self {
            socket_path,
            namespace,
            setup_authorization: XServerFrontendSetupAuthorization::default(),
        })
    }

    pub fn with_setup_authorization(
        mut self,
        setup_authorization: XServerFrontendSetupAuthorization,
    ) -> Self {
        self.setup_authorization = setup_authorization;
        self
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub const fn namespace(&self) -> NamespaceId {
        self.namespace
    }

    pub const fn setup_authorization(&self) -> &XServerFrontendSetupAuthorization {
        &self.setup_authorization
    }
}

/// A long-running, local X11 listener owned by the Sophia X Server Frontend.
///
/// The frontend owns only X11 protocol state. It has no DRM/KMS, physical-input,
/// scene-graph, or layout ownership. Current dispatch is intentionally
/// sequential; sharing the resource state across simultaneously live clients
/// requires client-specific XID allocation and is tracked as a separate
/// compatibility milestone.
#[cfg(unix)]
#[derive(Debug)]
pub struct XServerFrontend {
    config: XServerFrontendConfig,
    listener: UnixListener,
    state: X11CoreSocketServerState,
}

#[cfg(unix)]
impl XServerFrontend {
    pub fn bind(config: XServerFrontendConfig) -> Result<Self, X11SetupSocketError> {
        let listener = bind_x11_core_socket_server(config.socket_path())?;
        Ok(Self {
            config,
            listener,
            state: X11CoreSocketServerState::new(),
        })
    }

    pub fn config(&self) -> &XServerFrontendConfig {
        &self.config
    }

    pub fn serve_next(&mut self) -> Result<(), X11SetupSocketError> {
        serve_x11_core_socket_listener_once_with_setup_authorization(
            &self.listener,
            self.config.namespace(),
            &mut self.state,
            self.config.setup_authorization(),
            None,
            |_| Ok(()),
        )
    }

    pub fn serve_next_traced(
        &mut self,
        observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
    ) -> Result<(), X11SetupSocketError> {
        serve_x11_core_socket_listener_once_with_setup_authorization(
            &self.listener,
            self.config.namespace(),
            &mut self.state,
            self.config.setup_authorization(),
            None,
            observer,
        )
    }

    pub fn serve_forever(&mut self) -> Result<(), X11SetupSocketError> {
        self.serve_forever_traced(|_| Ok(()))
    }

    pub fn serve_forever_traced(
        &mut self,
        observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
    ) -> Result<(), X11SetupSocketError> {
        serve_x11_core_socket_listener_with_setup_authorization(
            &self.listener,
            self.config.namespace(),
            &mut self.state,
            self.config.setup_authorization(),
            observer,
        )
    }
}

#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct X11CoreDispatchTrace<'a> {
    pub sequence: u16,
    pub major_opcode: u8,
    pub request_detail: Option<String>,
    pub parse_error: Option<String>,
    pub result: &'a XDispatchResult,
    pub cpu_buffer_update: Option<&'a XAuthorityCpuBufferUpdate>,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityControlCommand {
    ConfigureSurface {
        transaction: TransactionId,
        surface: SurfaceId,
        size: Size,
    },
    FocusSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
}

#[cfg(unix)]
impl XAuthorityControlCommand {
    pub const fn transaction(self) -> TransactionId {
        match self {
            Self::ConfigureSurface { transaction, .. } | Self::FocusSurface { transaction, .. } => {
                transaction
            }
        }
    }

    pub const fn surface(self) -> SurfaceId {
        match self {
            Self::ConfigureSurface { surface, .. } | Self::FocusSurface { surface, .. } => surface,
        }
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityControlOutcome {
    Applied,
    UnknownSurface,
    InvalidSize,
    AuthorityRejected,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityControlAck {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub outcome: XAuthorityControlOutcome,
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
#[derive(Debug)]
pub struct X11CoreSocketServerState {
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    atoms: XAtomTable,
    properties: XPropertyTable,
    next_client_resource_range: u16,
}

#[cfg(unix)]
impl Default for X11CoreSocketServerState {
    fn default() -> Self {
        Self {
            runtime: Default::default(),
            atoms: Default::default(),
            properties: Default::default(),
            next_client_resource_range: 1,
        }
    }
}

#[cfg(unix)]
impl X11CoreSocketServerState {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_client_setup_success(&mut self) -> Result<XSetupSuccess, X11SetupSocketError> {
        if self.next_client_resource_range > X11_MAX_CLIENT_RESOURCE_RANGES {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend exhausted X11 client resource ranges",
            ));
        }
        let resource_id_base =
            u32::from(self.next_client_resource_range) * X11_CLIENT_RESOURCE_RANGE_SIZE;
        self.next_client_resource_range = self.next_client_resource_range.saturating_add(1);
        Ok(XSetupSuccess {
            resource_id_base,
            resource_id_mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
            ..XSetupSuccess::client_compatible()
        })
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
    let config = XServerFrontendConfig::new(path.as_ref(), namespace)?;
    let mut frontend = XServerFrontend::bind(config)?;
    frontend.serve_forever_traced(observer)
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
        None,
        &XServerFrontendSetupAuthorization::default(),
        move |trace| {
            try_emit_x_authority_trace(&transaction_sender, &trace)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            Ok(())
        },
    )
}

#[cfg(unix)]
pub fn run_x11_core_socket_server_once_session_channels(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    input_receiver: Receiver<XAuthorityInputEvent>,
    control_receiver: Receiver<XAuthorityControlCommand>,
    control_ack_sender: SyncSender<XAuthorityControlAck>,
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
        Some((control_receiver, control_ack_sender)),
        &XServerFrontendSetupAuthorization::default(),
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
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_once_with_setup_authorization(
        &listener,
        namespace,
        &mut state,
        &authorization,
        idle_timeout,
        observer,
    )
}

#[cfg(unix)]
pub fn bind_x11_core_socket_server(
    path: impl AsRef<Path>,
) -> Result<UnixListener, X11SetupSocketError> {
    let path = path.as_ref();
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path).map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to remove stale X11 core socket {}: {error}",
                    path.display()
                ))
            })?;
        }
        Ok(_) => {
            return Err(X11SetupSocketError::new(format!(
                "refusing to replace non-socket X11 core path {}",
                path.display()
            )));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(X11SetupSocketError::new(format!(
                "failed to inspect X11 core socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to bind X11 core socket {}: {error}",
            path.display()
        ))
    })?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to restrict X11 core socket {} to its owner: {error}",
            path.display()
        ))
    })?;
    Ok(listener)
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
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_once_with_setup_authorization(
        listener,
        namespace,
        state,
        &authorization,
        None,
        observer,
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
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_with_setup_authorization(
        listener,
        namespace,
        state,
        &authorization,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_with_setup_authorization(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    mut observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    loop {
        serve_x11_core_socket_listener_once_with_setup_authorization(
            listener,
            namespace,
            state,
            authorization,
            None,
            &mut observer,
        )?;
    }
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_once_with_setup_authorization(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
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
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
        &mut stream,
        namespace,
        state,
        authorization,
        observer,
    )
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_setup_socket_client_with_setup_authorization(stream, &authorization, || {
        Ok(XSetupSuccess::client_compatible())
    })?
    .ok_or_else(|| {
        X11SetupSocketError::new("default X11 setup authorization unexpectedly rejected")
    })
}

#[cfg(unix)]
fn serve_x11_setup_socket_client_with_setup_authorization(
    stream: &mut UnixStream,
    authorization: &XServerFrontendSetupAuthorization,
    setup_success: impl FnOnce() -> Result<XSetupSuccess, X11SetupSocketError>,
) -> Result<Option<XSetupRequest>, X11SetupSocketError> {
    let request = read_x11_setup_request(stream)?;
    if !authorization.permits(&request) {
        let response = encode_x11_setup_failure(
            request.byte_order,
            &XSetupFailure::new(b"Sophia X11 authorization failed"),
        )
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup failure: {error}"))
        })?;
        stream.write_all(&response).map_err(|error| {
            X11SetupSocketError::new(format!("failed to write X11 setup failure: {error}"))
        })?;
        stream.flush().map_err(|error| {
            X11SetupSocketError::new(format!("failed to flush X11 setup failure: {error}"))
        })?;
        return Ok(None);
    }
    let response =
        encode_x11_setup_success(request.byte_order, &setup_success()?).map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup success: {error}"))
        })?;
    stream
        .write_all(&response)
        .map_err(|error| X11SetupSocketError::new(format!("failed to write X11 setup: {error}")))?;
    stream
        .flush()
        .map_err(|error| X11SetupSocketError::new(format!("failed to flush X11 setup: {error}")))?;
    Ok(Some(request))
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
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
        stream,
        namespace,
        state,
        &authorization,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer_and_input(
        stream,
        namespace,
        state,
        None,
        None,
        authorization,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_input(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &mut X11CoreSocketServerState,
    input_receiver: Option<Receiver<XAuthorityInputEvent>>,
    control_channels: Option<(
        Receiver<XAuthorityControlCommand>,
        SyncSender<XAuthorityControlAck>,
    )>,
    authorization: &XServerFrontendSetupAuthorization,
    mut observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let Some(setup) =
        serve_x11_setup_socket_client_with_setup_authorization(stream, authorization, || {
            state.next_client_setup_success()
        })?
    else {
        return Ok(());
    };
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
    let control_writer = control_channels
        .map(|(receiver, acknowledgements)| {
            spawn_x11_control_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                focused_window.clone(),
                surface_windows.clone(),
                state.runtime.clone(),
                namespace,
                receiver,
                acknowledgements,
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
                    if let crate::XWireRequest::CreateWindow {
                        packet:
                            crate::XAuthorityRequestPacket {
                                kind:
                                    crate::XAuthorityRequestKind::CreateWindow {
                                        window, surface, ..
                                    },
                                ..
                            },
                        ..
                    } = &request
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
                    let mut runtime = state.runtime.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 authority runtime lock poisoned")
                    })?;
                    dispatch_x11_wire_request(
                        dispatch_context,
                        request,
                        &mut runtime,
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
            let cpu_buffer_update = state
                .runtime
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
                .take_cpu_buffer_update();
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                eprintln!(
                    "sophia-x-authority: seq={} opcode={}",
                    sequence, major_opcode
                );
            }
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
        result.as_ref().map_err(Clone::clone)?;
        writer_result?;
    }
    if let Some(writer) = control_writer {
        writer.stop.store(true, Ordering::Release);
        let writer_result = writer
            .thread
            .join()
            .map_err(|_| X11SetupSocketError::new("X11 control writer thread panicked"))?;
        result.as_ref().map_err(Clone::clone)?;
        writer_result?;
    }
    result
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
struct X11ControlWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn spawn_x11_control_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_window: Arc<AtomicU64>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    namespace: NamespaceId,
    receiver: Receiver<XAuthorityControlCommand>,
    acknowledgements: SyncSender<XAuthorityControlAck>,
) -> Result<X11ControlWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Acquire) {
            let command = match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let transaction = command.transaction();
            let surface = command.surface();
            let window = surface_windows
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 surface/window map lock poisoned"))?
                .get(&surface)
                .copied();
            let Some(window) = window else {
                send_x11_control_ack(
                    &acknowledgements,
                    XAuthorityControlAck {
                        transaction,
                        surface,
                        outcome: XAuthorityControlOutcome::UnknownSurface,
                    },
                )?;
                continue;
            };

            let event_sequence = sequence.load(Ordering::Acquire);
            let records = match command {
                XAuthorityControlCommand::ConfigureSurface { size, .. } => {
                    if size.width <= 0
                        || size.height <= 0
                        || size.width > i32::from(u16::MAX)
                        || size.height > i32::from(u16::MAX)
                    {
                        send_x11_control_ack(
                            &acknowledgements,
                            XAuthorityControlAck {
                                transaction,
                                surface,
                                outcome: XAuthorityControlOutcome::InvalidSize,
                            },
                        )?;
                        continue;
                    }
                    let geometry = match runtime
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 authority runtime lock poisoned")
                        })?
                        .configure_window_size_from_engine(namespace, window, size)
                    {
                        Ok(geometry) => geometry,
                        Err(_) => {
                            send_x11_control_ack(
                                &acknowledgements,
                                XAuthorityControlAck {
                                    transaction,
                                    surface,
                                    outcome: XAuthorityControlOutcome::AuthorityRejected,
                                },
                            )?;
                            continue;
                        }
                    };
                    let width = u16::try_from(geometry.width).expect("validated above");
                    let height = u16::try_from(geometry.height).expect("validated above");
                    vec![
                        encode_x_client_event(
                            byte_order,
                            XClientEvent::ConfigureNotify {
                                sequence: event_sequence,
                                event: window,
                                window,
                                above_sibling: None,
                                x: clamp_engine_i16(geometry.x),
                                y: clamp_engine_i16(geometry.y),
                                width,
                                height,
                                border_width: 0,
                                override_redirect: false,
                            },
                        ),
                        encode_x_client_event(
                            byte_order,
                            XClientEvent::Expose {
                                sequence: event_sequence,
                                window,
                                x: 0,
                                y: 0,
                                width,
                                height,
                                count: 0,
                            },
                        ),
                    ]
                }
                XAuthorityControlCommand::FocusSurface { .. } => {
                    let previous = XResourceId::new(
                        focused_window.swap(window.local.raw(), Ordering::AcqRel),
                        1,
                    );
                    let mut records = Vec::with_capacity(2);
                    if previous != window && previous.local.raw() != u64::from(X_SETUP_DEFAULT_ROOT)
                    {
                        records.push(encode_x_client_event(
                            byte_order,
                            XClientEvent::Focus {
                                sequence: event_sequence,
                                focused: false,
                                detail: 3,
                                event: previous,
                                mode: 0,
                            },
                        ));
                    }
                    records.push(encode_x_client_event(
                        byte_order,
                        XClientEvent::Focus {
                            sequence: event_sequence,
                            focused: true,
                            detail: 3,
                            event: window,
                            mode: 0,
                        },
                    ));
                    records
                }
            };

            let mut stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            for record in records {
                if let Err(error) = stream.write_all(&record) {
                    if is_x11_client_disconnect(&error) {
                        return Ok(());
                    }
                    return Err(X11SetupSocketError::new(format!(
                        "failed to write X11 control event: {error}"
                    )));
                }
            }
            stream.flush().map_err(|error| {
                X11SetupSocketError::new(format!("failed to flush X11 control event: {error}"))
            })?;
            drop(stream);
            send_x11_control_ack(
                &acknowledgements,
                XAuthorityControlAck {
                    transaction,
                    surface,
                    outcome: XAuthorityControlOutcome::Applied,
                },
            )?;
        }
        Ok(())
    });
    Ok(X11ControlWriter { stop, thread })
}

#[cfg(unix)]
fn send_x11_control_ack(
    sender: &SyncSender<XAuthorityControlAck>,
    acknowledgement: XAuthorityControlAck,
) -> Result<(), X11SetupSocketError> {
    match sender.try_send(acknowledgement) {
        Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
        Err(TrySendError::Full(_)) => Err(X11SetupSocketError::new(
            "X11 control acknowledgement channel is full",
        )),
    }
}

#[cfg(unix)]
fn clamp_engine_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
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
        crate::XWireRequest::CreateWindow { packet, .. } => match &packet.kind {
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
            _ => None,
        },
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
        crate::XWireRequest::CreateGraphicsContext { gc, drawable, .. } => Some(format!(
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

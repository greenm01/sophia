#[cfg(unix)]
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, Sender, SyncSender, TryRecvError, TrySendError, sync_channel,
};
#[cfg(unix)]
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{ErrorKind, IoSlice, IoSliceMut, Read, Write},
    mem::MaybeUninit,
    num::NonZeroUsize,
    os::fd::{AsFd, OwnedFd},
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[cfg(unix)]
use crate::{
    X_ATOM_NAME_WM_DELETE_WINDOW, X_ATOM_NAME_WM_PROTOCOLS, X_SETUP_CLIENT_PREFIX_LEN,
    X_SETUP_DEFAULT_RESOURCE_ID_MASK, X_SETUP_DEFAULT_ROOT, XAtomTable, XAuthorityCpuBufferUpdate,
    XAuthorityObservedTransactionBatch, XAuthorityResponsePacket, XAuthorityRuntime, XByteOrder,
    XClientEvent, XDispatchContext, XDispatchResult, XPropertyTable, XResourceId, XSetupFailure,
    XSetupRequest, XSetupSuccess, XWireClientContext, decode_x11_core_request,
    dispatch_x11_parse_error, dispatch_x11_wire_request, encode_x_client_event,
    encode_x11_setup_failure, encode_x11_setup_success, parse_x11_setup_request,
    try_emit_x_authority_trace, x11_setup_request_total_len,
};
#[cfg(unix)]
use sophia_protocol::{
    ClientAdmissionContext, ClientAdmissionId, ClientAuthenticationMethod, InputEventKind,
    NamespaceCapabilities, NamespaceContext, NamespaceId, NamespaceProfile, RoutedInputRequest,
    SeatId, Size, SurfaceId, TransactionId,
};

#[cfg(unix)]
const X11_CLIENT_RESOURCE_RANGE_SIZE: u32 = X_SETUP_DEFAULT_RESOURCE_ID_MASK + 1;
#[cfg(unix)]
const X11_MAX_CLIENT_RESOURCE_RANGES: u16 = (u32::MAX / X11_CLIENT_RESOURCE_RANGE_SIZE) as u16;
#[cfg(unix)]
const X_SERVER_FRONTEND_DEFAULT_MAX_CONCURRENT_CLIENTS: NonZeroUsize = match NonZeroUsize::new(16) {
    Some(value) => value,
    None => unreachable!(),
};

/// One ordered X11 socket write and the descriptors attached to its first byte.
///
/// Protocol dispatch remains byte-only and data-oriented. Native descriptor
/// ownership starts at this Unix-socket boundary and ends after the record is
/// sent or rejected, so descriptors cannot leak into authority runtime state.
#[cfg(unix)]
#[derive(Debug)]
pub struct X11SocketOutputRecord {
    bytes: Vec<u8>,
    fds: Vec<OwnedFd>,
}

#[cfg(unix)]
impl X11SocketOutputRecord {
    pub fn new(bytes: Vec<u8>, fds: Vec<OwnedFd>) -> Result<Self, X11SetupSocketError> {
        if bytes.is_empty() {
            return Err(X11SetupSocketError::new(
                "X11 socket output record cannot be empty",
            ));
        }
        if fds.len() > sophia_protocol::DMA_BUF_MAX_PLANES {
            return Err(X11SetupSocketError::new(format!(
                "X11 socket output record carried {} file descriptors; maximum is {}",
                fds.len(),
                sophia_protocol::DMA_BUF_MAX_PLANES,
            )));
        }
        Ok(Self { bytes, fds })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn fd_count(&self) -> usize {
        self.fds.len()
    }
}

#[cfg(unix)]
impl TryFrom<Vec<u8>> for X11SocketOutputRecord {
    type Error = X11SetupSocketError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::new(bytes, Vec::new())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X11SetupSocketError {
    message: String,
    client_disconnect: bool,
    client_failure: bool,
}

impl X11SetupSocketError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_disconnect: false,
            client_failure: false,
        }
    }

    fn client_disconnect(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_disconnect: true,
            client_failure: false,
        }
    }

    fn client_failure(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            client_disconnect: false,
            client_failure: true,
        }
    }
}

impl core::fmt::Display for X11SetupSocketError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for X11SetupSocketError {}

/// Monotonically assigned identity for one live X11 client connection.
///
/// The XID range identifies resources the client is allowed to create. This
/// identity identifies the connection that owns lifecycle cleanup, event
/// delivery, and later concurrent-dispatch bookkeeping.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct XServerFrontendClientId(u64);

#[cfg(unix)]
impl XServerFrontendClientId {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// The setup allocation retained for one connected X11 client.
///
/// The range is a connection lease, not a namespace boundary: in a classic
/// shared-X session other trusted clients may still reference a resource after
/// its creator made it. It is retained so disconnect cleanup can reclaim only
/// resources whose XIDs this client was allowed to create.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XServerFrontendClientLease {
    client: XServerFrontendClientId,
    resource_id_range: crate::XWireClientResourceRange,
}

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

    const fn authentication_method(&self) -> ClientAuthenticationMethod {
        match self {
            Self::UnauthenticatedLocal => ClientAuthenticationMethod::TrustedLocal,
            Self::MitMagicCookie(_) => ClientAuthenticationMethod::MitMagicCookie1,
        }
    }
}

/// Kernel-authenticated identity of one local Unix-socket peer.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct XServerFrontendPeerCredentials {
    pub process_id: u32,
    pub user_id: u32,
    pub group_id: u32,
}

/// Bounded facts supplied to session admission after X setup authentication.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XServerFrontendAdmissionRequest {
    pub setup_authentication: ClientAuthenticationMethod,
    pub peer_credentials: Option<XServerFrontendPeerCredentials>,
}

/// Fail-closed result from the session admission boundary.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendAdmissionError {
    Denied,
    Unavailable,
}

#[cfg(unix)]
impl core::fmt::Display for XServerFrontendAdmissionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Denied => formatter.write_str("X11 client admission denied"),
            Self::Unavailable => formatter.write_str("X11 client admission unavailable"),
        }
    }
}

#[cfg(unix)]
impl std::error::Error for XServerFrontendAdmissionError {}

/// Session policy called once after setup authentication and once at teardown.
///
/// Implementations may allocate and revoke identities in a session registry.
/// They receive no raw cookie bytes or X11 resource identity.
#[cfg(unix)]
pub trait XServerFrontendAdmissionPolicy: Send + Sync + 'static {
    fn admit(
        &self,
        request: XServerFrontendAdmissionRequest,
    ) -> Result<ClientAdmissionContext, XServerFrontendAdmissionError>;

    fn revoke(&self, context: ClientAdmissionContext) -> Result<(), XServerFrontendAdmissionError>;
}

/// Backend-owned capability for independently opening the Engine-selected render device.
///
/// The frontend receives a one-shot descriptor and never learns or retains a
/// device path. Each call must return a new kernel file description rather
/// than `dup`ing the backend's descriptor: DRM driver contexts and virtual
/// address state are scoped to that file description and must not be shared
/// between the server renderer and a DRI3 client.
#[cfg(unix)]
pub trait XServerFrontendRenderDeviceProvider: Send + Sync + 'static {
    fn open_render_device_fd(&self) -> Result<OwnedFd, XServerFrontendRenderDeviceError>;
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendRenderDeviceError {
    Unavailable,
    OpenFailed,
}

#[cfg(unix)]
impl core::fmt::Display for XServerFrontendRenderDeviceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("render device unavailable"),
            Self::OpenFailed => formatter.write_str("render device open failed"),
        }
    }
}

#[cfg(unix)]
impl std::error::Error for XServerFrontendRenderDeviceError {}

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
/// A production session should construct this from its session-owned namespace
/// registry. The legacy constructor retains fixed classic-shared behavior for
/// existing smoke helpers while callers migrate to an immutable context.
#[cfg(unix)]
#[derive(Clone)]
pub struct XServerFrontendConfig {
    socket_path: PathBuf,
    namespace: NamespaceContext,
    setup_authorization: XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    render_device_provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
    max_concurrent_clients: NonZeroUsize,
    output_topology: sophia_protocol::OutputTopologySnapshot,
    xkb_config: crate::XkbRmlvoConfig,
}

#[cfg(unix)]
impl core::fmt::Debug for XServerFrontendConfig {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("XServerFrontendConfig")
            .field("socket_path", &self.socket_path)
            .field("namespace", &self.namespace)
            .field("setup_authorization", &self.setup_authorization)
            .field("has_admission_policy", &self.admission_policy.is_some())
            .field(
                "has_render_device_provider",
                &self.render_device_provider.is_some(),
            )
            .field("max_concurrent_clients", &self.max_concurrent_clients)
            .field("output_topology", &self.output_topology)
            .field("xkb_config", &self.xkb_config)
            .finish()
    }
}

#[cfg(unix)]
impl XServerFrontendConfig {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        namespace: NamespaceId,
    ) -> Result<Self, X11SetupSocketError> {
        let namespace = NamespaceContext::new(
            namespace,
            NamespaceProfile::ClassicShared,
            NamespaceCapabilities::NONE,
        )
        .ok_or_else(|| {
            X11SetupSocketError::new("Sophia X Server Frontend namespace must be valid")
        })?;
        Self::new_with_namespace_context(socket_path, namespace)
    }

    pub fn new_with_namespace_context(
        socket_path: impl Into<PathBuf>,
        namespace: NamespaceContext,
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
            admission_policy: None,
            render_device_provider: None,
            max_concurrent_clients: X_SERVER_FRONTEND_DEFAULT_MAX_CONCURRENT_CLIENTS,
            output_topology: sophia_protocol::OutputTopologySnapshot::deterministic(),
            xkb_config: crate::XkbRmlvoConfig::default(),
        })
    }

    pub fn with_setup_authorization(
        mut self,
        setup_authorization: XServerFrontendSetupAuthorization,
    ) -> Self {
        self.setup_authorization = setup_authorization;
        self
    }

    pub fn with_admission_policy(
        mut self,
        admission_policy: Arc<dyn XServerFrontendAdmissionPolicy>,
    ) -> Self {
        self.admission_policy = Some(admission_policy);
        self
    }

    pub fn with_render_device_provider(
        mut self,
        provider: Arc<dyn XServerFrontendRenderDeviceProvider>,
    ) -> Self {
        self.render_device_provider = Some(provider);
        self
    }

    /// Sets the upper bound for simultaneously dispatched X11 clients.
    ///
    /// The default allows sixteen connections. This bound applies only to the
    /// opt-in concurrent dispatcher; the existing sequential APIs still serve
    /// one connection at a time.
    pub fn with_max_concurrent_clients(mut self, max_concurrent_clients: NonZeroUsize) -> Self {
        self.max_concurrent_clients = max_concurrent_clients;
        self
    }

    pub fn with_output_topology(
        mut self,
        output_topology: sophia_protocol::OutputTopologySnapshot,
    ) -> Result<Self, X11SetupSocketError> {
        output_topology.validate().map_err(|error| {
            X11SetupSocketError::new(format!("invalid Engine output topology: {error:?}"))
        })?;
        self.output_topology = output_topology;
        Ok(self)
    }

    pub fn output_topology(&self) -> &sophia_protocol::OutputTopologySnapshot {
        &self.output_topology
    }

    pub fn with_xkb_config(
        mut self,
        xkb_config: crate::XkbRmlvoConfig,
    ) -> Result<Self, X11SetupSocketError> {
        xkb_config.validate().map_err(|error| {
            X11SetupSocketError::new(format!("invalid XKB configuration: {error}"))
        })?;
        self.xkb_config = xkb_config;
        Ok(self)
    }

    pub const fn xkb_config(&self) -> &crate::XkbRmlvoConfig {
        &self.xkb_config
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub const fn namespace(&self) -> NamespaceId {
        self.namespace.id
    }

    pub const fn namespace_context(&self) -> NamespaceContext {
        self.namespace
    }

    pub const fn setup_authorization(&self) -> &XServerFrontendSetupAuthorization {
        &self.setup_authorization
    }

    fn admission_policy(&self) -> Option<Arc<dyn XServerFrontendAdmissionPolicy>> {
        self.admission_policy.clone()
    }

    fn render_device_provider(&self) -> Option<Arc<dyn XServerFrontendRenderDeviceProvider>> {
        self.render_device_provider.clone()
    }

    pub const fn max_concurrent_clients(&self) -> NonZeroUsize {
        self.max_concurrent_clients
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn x11_peer_credentials(
    stream: &UnixStream,
) -> Result<Option<XServerFrontendPeerCredentials>, X11SetupSocketError> {
    let credentials = rustix::net::sockopt::socket_peercred(stream).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 peer credentials: {error}"))
    })?;
    let process_id = u32::try_from(credentials.pid.as_raw_pid()).map_err(|_| {
        X11SetupSocketError::new("X11 peer process ID is outside the supported range")
    })?;
    Ok(Some(XServerFrontendPeerCredentials {
        process_id,
        user_id: credentials.uid.as_raw(),
        group_id: credentials.gid.as_raw(),
    }))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn x11_peer_credentials(
    _stream: &UnixStream,
) -> Result<Option<XServerFrontendPeerCredentials>, X11SetupSocketError> {
    Ok(None)
}

#[cfg(unix)]
struct XServerFrontendAdmissionLease {
    policy: Arc<dyn XServerFrontendAdmissionPolicy>,
    context: Option<ClientAdmissionContext>,
}

#[cfg(unix)]
impl XServerFrontendAdmissionLease {
    fn new(
        policy: Arc<dyn XServerFrontendAdmissionPolicy>,
        context: ClientAdmissionContext,
    ) -> Self {
        Self {
            policy,
            context: Some(context),
        }
    }

    fn context(&self) -> ClientAdmissionContext {
        self.context
            .expect("active X11 admission lease must retain its context")
    }

    fn revoke(&mut self) -> Result<(), XServerFrontendAdmissionError> {
        let Some(context) = self.context.take() else {
            return Ok(());
        };
        self.policy.revoke(context)
    }
}

#[cfg(unix)]
impl Drop for XServerFrontendAdmissionLease {
    fn drop(&mut self) {
        let _ = self.revoke();
    }
}

/// A local X11 listener owned by the Sophia X Server Frontend.
///
/// The frontend owns only X11 protocol state. It has no DRM/KMS, physical-input,
/// scene-graph, or layout ownership. Its established APIs serve one client at
/// a time. The explicit concurrent APIs use bounded workers that share the
/// independently synchronized frontend state.
#[cfg(unix)]
#[derive(Debug)]
pub struct XServerFrontend {
    config: XServerFrontendConfig,
    listener: UnixListener,
    state: X11CoreSocketServerState,
    workers: BTreeMap<u64, X11CoreClientWorker>,
    worker_completions: Receiver<X11CoreClientWorkerCompletion>,
    worker_completion_sender: Sender<X11CoreClientWorkerCompletion>,
    worker_admissions: BTreeMap<ClientAdmissionId, u64>,
    pending_admission_revocations: BTreeSet<ClientAdmissionId>,
    worker_admission_events: Receiver<X11CoreClientWorkerAdmission>,
    worker_admission_event_sender: Sender<X11CoreClientWorkerAdmission>,
    next_worker_id: u64,
}

#[cfg(unix)]
impl XServerFrontend {
    pub fn bind(config: XServerFrontendConfig) -> Result<Self, X11SetupSocketError> {
        let listener = bind_x11_core_socket_server(config.socket_path())?;
        let state = X11CoreSocketServerState::with_output_topology_and_xkb_config(
            config.output_topology().clone(),
            config.xkb_config(),
        )?
        .with_optional_render_device_provider(config.render_device_provider());
        let (worker_completion_sender, worker_completions) = std::sync::mpsc::channel();
        let (worker_admission_event_sender, worker_admission_events) = std::sync::mpsc::channel();
        Ok(Self {
            config,
            listener,
            state,
            workers: BTreeMap::new(),
            worker_completions,
            worker_completion_sender,
            worker_admissions: BTreeMap::new(),
            pending_admission_revocations: BTreeSet::new(),
            worker_admission_events,
            worker_admission_event_sender,
            next_worker_id: 1,
        })
    }

    pub fn config(&self) -> &XServerFrontendConfig {
        &self.config
    }

    pub fn update_output_topology(
        &mut self,
        snapshot: sophia_protocol::OutputTopologySnapshot,
    ) -> Result<XAuthorityOutputUpdateOutcome, X11SetupSocketError> {
        let generation = snapshot.generation;
        let mut runtime = self
            .state
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
        match runtime.update_output_topology(snapshot) {
            Ok(true) => Ok(XAuthorityOutputUpdateOutcome::Applied {
                generation,
                notifications: 0,
            }),
            Ok(false) => Ok(XAuthorityOutputUpdateOutcome::RejectedStale { generation }),
            Err(error) => Ok(XAuthorityOutputUpdateOutcome::RejectedInvalid { generation, error }),
        }
    }

    /// Number of X11 clients currently holding a frontend connection lease.
    ///
    /// With the present sequential dispatcher this is normally zero between
    /// `serve_next` calls. Concurrent workers retain their lease until stream
    /// teardown finishes, so the value is also useful for supervision.
    pub fn active_client_count(&self) -> usize {
        self.state.active_client_count()
    }

    /// Number of worker threads currently supervised by the concurrent APIs.
    ///
    /// This includes a worker while it is completing X11 setup, before that
    /// connection receives its client lease.
    pub fn active_client_worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn clipboard_executor(
        &self,
        broker: &XServerFrontendRouteBroker,
    ) -> XServerFrontendClipboardExecutor {
        XServerFrontendClipboardExecutor {
            state: self.state.clone(),
            routing: broker.registry.clone(),
        }
    }

    /// Reaps every concurrent worker that has already completed without
    /// waiting for an active client.
    pub fn poll_client_workers(&mut self) -> Result<(), X11SetupSocketError> {
        self.reap_finished_client_workers()
    }

    /// Disconnects the worker holding one session-issued admission.
    ///
    /// The worker retains teardown ownership: it stops its private writers,
    /// releases routes and X resources, emits surface removal, and only then
    /// revokes the admission lease. An admission that is not attached yet is
    /// retained as a pending revocation so a setup race cannot lose the
    /// supervisor command; `Ok(false)` reports that deferred outcome.
    pub fn revoke_admission(
        &mut self,
        admission: ClientAdmissionId,
    ) -> Result<bool, X11SetupSocketError> {
        self.reap_finished_client_workers()?;
        self.observe_worker_admissions()?;
        let Some(worker_id) = self.worker_admissions.remove(&admission) else {
            self.pending_admission_revocations.insert(admission);
            return Ok(false);
        };
        if let Err(error) = self.shutdown_worker(worker_id) {
            self.worker_admissions.insert(admission, worker_id);
            return Err(error);
        }
        Ok(true)
    }

    /// Starts one client worker, if the configured concurrency limit permits
    /// it, and returns as soon as that connection is accepted.
    ///
    /// Call [`Self::wait_for_clients`] before releasing a manually supervised
    /// frontend so every accepted connection is reaped. The observer must be
    /// thread-safe because worker callbacks may run concurrently.
    pub fn serve_next_concurrently(&mut self) -> Result<(), X11SetupSocketError> {
        let observer: Arc<X11CoreTraceObserver> = Arc::new(|_| Ok(()));
        self.serve_next_concurrently_traced(observer)
    }

    /// Like [`Self::serve_next_concurrently`], with an observer for each
    /// completed X11 dispatch.
    pub fn serve_next_concurrently_traced(
        &mut self,
        observer: Arc<X11CoreTraceObserver>,
    ) -> Result<(), X11SetupSocketError> {
        self.serve_next_concurrently_with_routing(observer, None)
    }

    /// Starts one concurrent client worker with the Engine-facing route broker
    /// attached to its private input and control queues.
    pub fn serve_next_concurrently_routed(
        &mut self,
        broker: &XServerFrontendRouteBroker,
    ) -> Result<(), X11SetupSocketError> {
        let observer: Arc<X11CoreTraceObserver> = Arc::new(|_| Ok(()));
        self.serve_next_concurrently_routed_traced(broker, observer)
    }

    /// Like [`Self::serve_next_concurrently_routed`], with a thread-safe
    /// observer for each completed X11 dispatch.
    pub fn serve_next_concurrently_routed_traced(
        &mut self,
        broker: &XServerFrontendRouteBroker,
        observer: Arc<X11CoreTraceObserver>,
    ) -> Result<(), X11SetupSocketError> {
        self.serve_next_concurrently_with_routing(observer, Some(broker.registry.clone()))
    }

    /// Attempts to accept one routed concurrent client without blocking.
    ///
    /// `Ok(false)` means no connection is ready. The configured worker limit
    /// remains enforced as an error, so a service must reap completed workers
    /// before attempting another accept.
    pub fn try_serve_next_concurrently_routed_traced(
        &mut self,
        broker: &XServerFrontendRouteBroker,
        observer: Arc<X11CoreTraceObserver>,
    ) -> Result<bool, X11SetupSocketError> {
        self.try_serve_next_concurrently_with_routing(observer, Some(broker.registry.clone()))
    }

    fn serve_next_concurrently_with_routing(
        &mut self,
        observer: Arc<X11CoreTraceObserver>,
        routing: Option<XServerFrontendRouteRegistry>,
    ) -> Result<(), X11SetupSocketError> {
        self.accept_next_concurrently_with_routing(observer, routing, false)
            .map(|_| ())
    }

    fn try_serve_next_concurrently_with_routing(
        &mut self,
        observer: Arc<X11CoreTraceObserver>,
        routing: Option<XServerFrontendRouteRegistry>,
    ) -> Result<bool, X11SetupSocketError> {
        self.accept_next_concurrently_with_routing(observer, routing, true)
    }

    fn accept_next_concurrently_with_routing(
        &mut self,
        observer: Arc<X11CoreTraceObserver>,
        routing: Option<XServerFrontendRouteRegistry>,
        nonblocking: bool,
    ) -> Result<bool, X11SetupSocketError> {
        self.reap_finished_client_workers()?;
        let limit = self.config.max_concurrent_clients().get();
        if self.workers.len() >= limit {
            return Err(X11SetupSocketError::new(format!(
                "Sophia X Server Frontend concurrent-client limit ({limit}) reached"
            )));
        }
        let accepted = if nonblocking {
            self.listener.set_nonblocking(true).map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to make X11 core listener nonblocking: {error}"
                ))
            })?;
            let accepted = self.listener.accept();
            self.listener.set_nonblocking(false).map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to restore blocking X11 core listener: {error}"
                ))
            })?;
            accepted
        } else {
            self.listener.accept()
        };
        match accepted {
            Ok((stream, _)) => {
                self.spawn_client_worker(stream, observer, routing)?;
                Ok(true)
            }
            Err(error) if nonblocking && error.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(X11SetupSocketError::new(format!(
                "failed to accept X11 core client: {error}"
            ))),
        }
    }

    /// Reaps every connection worker started by the concurrent APIs.
    ///
    /// This is the explicit supervision boundary for a caller that accepts a
    /// bounded batch of local clients. It waits only for already accepted
    /// clients; it does not accept another connection.
    pub fn wait_for_clients(&mut self) -> Result<(), X11SetupSocketError> {
        let mut first_error = self.reap_finished_client_workers().err();
        while !self.workers.is_empty() {
            self.observe_worker_admissions()?;
            let completion = if self.pending_admission_revocations.is_empty() {
                self.worker_completions.recv().map_err(|_| {
                    X11SetupSocketError::new(
                        "Sophia X Server Frontend concurrent worker supervisor disconnected",
                    )
                })?
            } else {
                match self
                    .worker_completions
                    .recv_timeout(Duration::from_millis(1))
                {
                    Ok(completion) => completion,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err(X11SetupSocketError::new(
                            "Sophia X Server Frontend concurrent worker supervisor disconnected",
                        ));
                    }
                }
            };
            if let Err(error) = self.reap_client_worker(completion)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn serve_next(&mut self) -> Result<(), X11SetupSocketError> {
        serve_x11_core_socket_listener_once_with_setup_authorization(
            &self.listener,
            self.config.namespace(),
            &self.state,
            self.config.setup_authorization(),
            self.config.admission_policy(),
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
            &self.state,
            self.config.setup_authorization(),
            self.config.admission_policy(),
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
            &self.state,
            self.config.setup_authorization(),
            self.config.admission_policy(),
            observer,
        )
    }

    fn spawn_client_worker(
        &mut self,
        mut stream: UnixStream,
        observer: Arc<X11CoreTraceObserver>,
        routing: Option<XServerFrontendRouteRegistry>,
    ) -> Result<(), X11SetupSocketError> {
        let worker_id = self.next_worker_id;
        self.next_worker_id = self.next_worker_id.checked_add(1).ok_or_else(|| {
            X11SetupSocketError::new("Sophia X Server Frontend exhausted worker identities")
        })?;
        let state = self.state.clone();
        let namespace = self.config.namespace();
        let authorization = self.config.setup_authorization().clone();
        let admission_policy = self.config.admission_policy();
        let completion_sender = self.worker_completion_sender.clone();
        let admission_event_sender = self.worker_admission_event_sender.clone();
        let shutdown = stream.try_clone().map_err(|error| {
            X11SetupSocketError::new(format!(
                "failed to clone X11 client socket for supervision: {error}"
            ))
        })?;
        let worker = std::thread::Builder::new()
            .name(format!("sophia-x11-client-{worker_id}"))
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization_and_routing(
                        &mut stream,
                        namespace,
                        &state,
                        &authorization,
                        admission_policy,
                        routing,
                        Some((worker_id, admission_event_sender)),
                        move |trace| observer(trace),
                    )
                }))
                .unwrap_or_else(|_| {
                    Err(X11SetupSocketError::new(
                        "Sophia X Server Frontend client worker panicked",
                    ))
                });
                let _ = completion_sender.send(X11CoreClientWorkerCompletion { worker_id, result });
            })
            .map_err(|error| {
                X11SetupSocketError::new(format!("failed to start X11 client worker: {error}"))
            })?;
        self.workers.insert(
            worker_id,
            X11CoreClientWorker {
                thread: worker,
                shutdown,
            },
        );
        Ok(())
    }

    fn reap_finished_client_workers(&mut self) -> Result<(), X11SetupSocketError> {
        self.observe_worker_admissions()?;
        loop {
            match self.worker_completions.try_recv() {
                Ok(completion) => self.reap_client_worker(completion)?,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) if self.workers.is_empty() => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    return Err(X11SetupSocketError::new(
                        "Sophia X Server Frontend concurrent worker supervisor disconnected",
                    ));
                }
            }
        }
    }

    fn reap_client_worker(
        &mut self,
        completion: X11CoreClientWorkerCompletion,
    ) -> Result<(), X11SetupSocketError> {
        let worker = self.workers.remove(&completion.worker_id).ok_or_else(|| {
            X11SetupSocketError::new("Sophia X Server Frontend lost a concurrent client worker")
        })?;
        self.worker_admissions
            .retain(|_, worker_id| *worker_id != completion.worker_id);
        worker.thread.join().map_err(|_| {
            X11SetupSocketError::new("Sophia X Server Frontend client worker panicked")
        })?;
        match completion.result {
            Err(error) if error.client_failure || error.client_disconnect => {
                eprintln!("Sophia X Server Frontend disconnected one client: {error}");
                Ok(())
            }
            result => result,
        }
    }

    fn observe_worker_admissions(&mut self) -> Result<(), X11SetupSocketError> {
        loop {
            match self.worker_admission_events.try_recv() {
                Ok(event) if self.workers.contains_key(&event.worker_id) => {
                    match self.worker_admissions.get(&event.admission).copied() {
                        Some(existing) if existing != event.worker_id => {
                            return Err(X11SetupSocketError::new(
                                "Sophia X Server Frontend admission is attached to multiple workers",
                            ));
                        }
                        Some(_) => {}
                        None => {
                            self.worker_admissions
                                .insert(event.admission, event.worker_id);
                        }
                    }
                    if self.pending_admission_revocations.remove(&event.admission) {
                        self.shutdown_worker(event.worker_id)?;
                        self.worker_admissions.remove(&event.admission);
                    }
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) if self.workers.is_empty() => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    return Err(X11SetupSocketError::new(
                        "Sophia X Server Frontend admission observer disconnected",
                    ));
                }
            }
        }
    }

    fn shutdown_worker(&self, worker_id: u64) -> Result<(), X11SetupSocketError> {
        let Some(worker) = self.workers.get(&worker_id) else {
            return Ok(());
        };
        match worker.shutdown.shutdown(Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotConnected => Ok(()),
            Err(error) => Err(X11SetupSocketError::new(format!(
                "failed to revoke X11 client admission: {error}"
            ))),
        }
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityOutputUpdateOutcome {
    Applied {
        generation: u64,
        /// RandR records queued to subscribed live clients by the routed
        /// service. Direct frontend updates retain zero.
        notifications: usize,
    },
    RejectedStale {
        generation: u64,
    },
    RejectedInvalid {
        generation: u64,
        error: sophia_protocol::OutputTopologyError,
    },
}

/// Authority-side endpoint for a portal executor. Broker-visible values stop
/// at the grant and payload; retained XIDs, atoms, properties, and event
/// routing remain private to this object.
#[cfg(unix)]
#[derive(Clone)]
pub struct XServerFrontendClipboardExecutor {
    state: X11CoreSocketServerState,
    routing: XServerFrontendRouteRegistry,
}

#[cfg(unix)]
impl XServerFrontendClipboardExecutor {
    pub fn request_source(
        &self,
        grant: &sophia_protocol::PortalGrant,
    ) -> Result<crate::ClipboardSelectionProxy, X11SetupSocketError> {
        let proxy = self
            .state
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
            .begin_clipboard_source_request(grant)
            .map_err(|error| {
                X11SetupSocketError::new(format!("clipboard source request rejected: {error:?}"))
            })?;
        let target = self
            .state
            .client_for_resource(proxy.owner)?
            .ok_or_else(|| X11SetupSocketError::new("clipboard owner disconnected"))?;
        self.routing
            .route_protocol(
                target,
                XClientEvent::SelectionRequest {
                    sequence: 0,
                    time: proxy.time,
                    owner: proxy.owner,
                    requestor: proxy.requestor,
                    selection: proxy.selection,
                    target: proxy.target,
                    property: proxy.property,
                },
            )
            .map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to route clipboard source request: {error}"
                ))
            })?;
        Ok(proxy)
    }

    pub fn execute(
        &self,
        grant: &sophia_protocol::PortalGrant,
        payload: &[u8],
    ) -> Result<crate::ClipboardSelectionExecutionOutcome, X11SetupSocketError> {
        let mut runtime = self
            .state
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
        let mut atoms = self
            .state
            .atoms
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 atom table lock poisoned"))?;
        let mut properties = self
            .state
            .properties
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 property table lock poisoned"))?;
        let outcome = runtime
            .execute_clipboard_payload(grant.transfer, grant, payload, &mut atoms, &mut properties)
            .map_err(|error| {
                X11SetupSocketError::new(format!("clipboard executor rejected payload: {error:?}"))
            })?;
        drop(properties);
        drop(atoms);
        drop(runtime);
        self.route_outcome(&outcome)?;
        Ok(outcome)
    }

    pub fn fail(
        &self,
        transfer: sophia_protocol::PortalTransferId,
        error: crate::ClipboardSelectionExecutionError,
    ) -> Result<crate::ClipboardSelectionExecutionOutcome, X11SetupSocketError> {
        let mut runtime = self
            .state
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
        let proxies = runtime.discard_clipboard_proxies(transfer);
        let outcome = runtime
            .fail_clipboard_transfer(transfer, error)
            .map_err(|error| {
                X11SetupSocketError::new(format!("clipboard failure rejected: {error:?}"))
            })?;
        drop(runtime);
        if !proxies.is_empty() {
            let mut properties = self
                .state
                .properties
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 property table lock poisoned"))?;
            for (namespace, proxy) in proxies {
                properties.remove_window(namespace, proxy);
            }
        }
        self.route_outcome(&outcome)?;
        Ok(outcome)
    }

    fn route_outcome(
        &self,
        outcome: &crate::ClipboardSelectionExecutionOutcome,
    ) -> Result<(), X11SetupSocketError> {
        let notify = match &outcome {
            crate::ClipboardSelectionExecutionOutcome::Handoff(handoff) => handoff.notify,
            crate::ClipboardSelectionExecutionOutcome::Failed { notify, .. } => *notify,
        };
        let target = self
            .state
            .client_for_resource(notify.requestor)?
            .ok_or_else(|| X11SetupSocketError::new("clipboard requestor disconnected"))?;
        self.routing
            .route_protocol(
                target,
                XClientEvent::SelectionNotify {
                    sequence: 0,
                    time: notify.time,
                    requestor: notify.requestor,
                    selection: notify.selection,
                    target: notify.target,
                    property: notify.property,
                },
            )
            .map_err(|error| {
                X11SetupSocketError::new(format!("failed to route clipboard notify: {error}"))
            })?;
        Ok(())
    }
}

/// Coordinates one retained X11 selection through the broker socket. The
/// grant is obtained before the source proxy is exposed, and only the bounded
/// captured bytes return over the portal connection.
#[cfg(unix)]
pub fn coordinate_x11_clipboard_transfer(
    path: impl AsRef<Path>,
    request: &sophia_protocol::PortalBrokerRequestPacket,
    executor: &XServerFrontendClipboardExecutor,
    routes: &XServerFrontendRouteBroker,
    timeout: Duration,
) -> Result<sophia_protocol::PortalBrokerResponsePacket, X11SetupSocketError> {
    let mut session =
        sophia_portal::begin_portal_clipboard_request(path, request).map_err(|error| {
            X11SetupSocketError::new(format!("portal broker request failed: {error}"))
        })?;
    let decision = session.response().decision.clone();
    match &decision {
        sophia_protocol::PortalBrokerResponseDecision::Denied => {
            executor.fail(
                session.response().transfer,
                crate::ClipboardSelectionExecutionError::Denied,
            )?;
        }
        sophia_protocol::PortalBrokerResponseDecision::Allowed(grant) => {
            executor.request_source(grant)?;
            let payload = routes
                .recv_clipboard_source_payload_timeout(timeout)
                .map_err(|error| {
                    let _ = executor.fail(
                        grant.transfer,
                        crate::ClipboardSelectionExecutionError::Expired,
                    );
                    X11SetupSocketError::new(format!(
                        "clipboard source payload unavailable: {error}"
                    ))
                })?;
            if payload.transfer != grant.transfer {
                executor.fail(
                    grant.transfer,
                    crate::ClipboardSelectionExecutionError::ExecutorFailure,
                )?;
                return Err(X11SetupSocketError::new(
                    "clipboard source payload correlation mismatch",
                ));
            }
            session.send_payload(&payload.bytes).map_err(|error| {
                let _ = executor.fail(
                    grant.transfer,
                    crate::ClipboardSelectionExecutionError::ExecutorFailure,
                );
                X11SetupSocketError::new(format!("portal payload send failed: {error}"))
            })?;
        }
    }
    Ok(session.into_response())
}

/// A trace callback used by a bounded concurrent frontend worker.
#[cfg(unix)]
pub type X11CoreTraceObserver = dyn for<'trace> Fn(X11CoreDispatchTrace<'trace>) -> Result<(), X11SetupSocketError>
    + Send
    + Sync
    + 'static;

#[cfg(unix)]
struct X11CoreClientWorkerCompletion {
    worker_id: u64,
    result: Result<(), X11SetupSocketError>,
}

#[cfg(unix)]
#[derive(Debug)]
struct X11CoreClientWorker {
    thread: std::thread::JoinHandle<()>,
    shutdown: UnixStream,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct X11CoreClientWorkerAdmission {
    worker_id: u64,
    admission: ClientAdmissionId,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct X11CoreDispatchTrace<'a> {
    pub client: XServerFrontendClientId,
    pub resource_id_range: crate::XWireClientResourceRange,
    pub sequence: u16,
    pub major_opcode: u8,
    pub request_detail: Option<String>,
    pub parse_error: Option<String>,
    pub result: &'a XDispatchResult,
    pub cpu_buffer_update: Option<&'a XAuthorityCpuBufferUpdate>,
    pub received_fd_count: usize,
    pub received_fds: &'a [OwnedFd],
    pub dri3_pixmap_import: Option<XAuthorityDri3PixmapImport>,
    pub dri3_fence_import: Option<XAuthorityDri3FenceImport>,
    pub present_submission: Option<XAuthorityPresentSubmission>,
    pub released_dma_bufs: &'a [sophia_protocol::BufferHandle],
    pub released_fences: &'a [sophia_protocol::FenceHandle],
    pub server_reply_fd_count: usize,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityDri3PixmapImport {
    pub pixmap: XResourceId,
    pub descriptor: sophia_protocol::DmaBufDescriptor,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityDri3FenceImport {
    pub fence: XResourceId,
    pub handle: sophia_protocol::FenceHandle,
    pub initially_triggered: bool,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityPresentSubmission {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub buffer: sophia_protocol::BufferHandle,
    pub acquire_fence: Option<sophia_protocol::FenceHandle>,
    pub idle_fence: Option<sophia_protocol::FenceHandle>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum XPresentCompletionMode {
    Copy = 0,
    Flip = 1,
    Skip = 2,
    SuboptimalCopy = 3,
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

/// An Engine-selected input event addressed to one live X11 connection.
///
/// The Engine chooses the target surface from its committed scene and uses the
/// transaction-side surface route table to select this client. The frontend
/// refuses to deliver a route addressed to another connection.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientInputEvent {
    pub client: XServerFrontendClientId,
    pub event: XAuthorityInputEvent,
    pub target_window: Option<XResourceId>,
    pub xi_event_type: Option<u16>,
    pub xi_transition_mask: u16,
    /// Opaque Engine-assigned token used to prove that the owning X11 worker
    /// flushed this event to its client socket. It deliberately carries no
    /// X11 resource, key, or text identity.
    pub delivery: Option<XAuthorityInputDeliveryId>,
}

/// Protocol-neutral physical input after Engine hit-testing and focus policy.
/// The X authority, not Engine, resolves this Sophia surface to an X11 client
/// and applies its keyboard/pointer protocol state.
#[cfg(unix)]
#[derive(Clone, Debug, PartialEq)]
pub struct XAuthorityRoutedInput {
    pub request: RoutedInputRequest,
    pub delivery: Option<XAuthorityInputDeliveryId>,
}

/// Opaque per-session identifier for one routed input event.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct XAuthorityInputDeliveryId(u64);

#[cfg(unix)]
impl XAuthorityInputDeliveryId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Reduced outcome for one Engine-addressed input event.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityInputDeliveryOutcome {
    /// The owning worker serialized and flushed the event to the X11 client.
    Flushed,
    /// The route could not reach its private client queue.
    RouteRejected,
    /// The owning worker could not write or flush the event.
    WriteFailed,
}

/// Delivery result returned from a routed X11 worker to the Engine.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientInputDelivery {
    pub client: XServerFrontendClientId,
    pub delivery: XAuthorityInputDeliveryId,
    pub outcome: XAuthorityInputDeliveryOutcome,
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
    CloseSurface {
        transaction: TransactionId,
        surface: SurfaceId,
    },
}

/// An Engine control request addressed to the frontend connection that owns
/// the referenced X11 surface.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientControlCommand {
    pub client: XServerFrontendClientId,
    pub command: XAuthorityControlCommand,
}

#[cfg(unix)]
impl XAuthorityControlCommand {
    pub const fn transaction(self) -> TransactionId {
        match self {
            Self::ConfigureSurface { transaction, .. }
            | Self::FocusSurface { transaction, .. }
            | Self::CloseSurface { transaction, .. } => transaction,
        }
    }

    pub const fn surface(self) -> SurfaceId {
        match self {
            Self::ConfigureSurface { surface, .. }
            | Self::FocusSurface { surface, .. }
            | Self::CloseSurface { surface, .. } => surface,
        }
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XAuthorityControlOutcome {
    Delivered,
    UnknownSurface,
    InvalidSize,
    AuthorityRejected,
    UnsupportedProtocol,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityControlAck {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub outcome: XAuthorityControlOutcome,
}

/// A control acknowledgement bound to the connection that applied it.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientControlAck {
    pub client: XServerFrontendClientId,
    pub acknowledgement: XAuthorityControlAck,
}

/// Service-supervision command for a long-running routed X11 frontend.
#[cfg(unix)]
#[derive(Clone, Debug)]
pub enum XServerFrontendServiceCommand {
    /// Stop accepting new clients and drain workers that are already connected.
    StopAccepting,
    /// Disconnect the worker holding this session-issued admission. Its normal
    /// teardown releases routes and resources before revoking the lease.
    RevokeAdmission { admission: ClientAdmissionId },
    /// Apply a newer Engine output snapshot and notify subscribed clients.
    UpdateOutputTopology {
        snapshot: sophia_protocol::OutputTopologySnapshot,
        acknowledgement: SyncSender<XAuthorityOutputUpdateOutcome>,
    },
}

/// Routing failure between the Engine-facing ingress queues and a live X11
/// client worker.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendRouteError {
    UnknownClient { client: XServerFrontendClientId },
    UnknownSurface { surface: SurfaceId },
    ClientQueueFull { client: XServerFrontendClientId },
    DuplicatePresentation { transaction: TransactionId },
    ClientQueueDisconnected { client: XServerFrontendClientId },
    DuplicateClient { client: XServerFrontendClientId },
    InputDeliveryQueueFull,
    RegistryPoisoned,
}

#[cfg(unix)]
impl core::fmt::Display for XServerFrontendRouteError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownClient { client } => {
                write!(
                    formatter,
                    "X11 route targets unknown client {}",
                    client.raw()
                )
            }
            Self::UnknownSurface { surface } => write!(
                formatter,
                "X11 route targets unknown Sophia surface {}:{}",
                surface.index(),
                surface.generation()
            ),
            Self::ClientQueueFull { client } => {
                write!(
                    formatter,
                    "X11 route queue is full for client {}",
                    client.raw()
                )
            }
            Self::DuplicatePresentation { transaction } => write!(
                formatter,
                "X11 Present transaction {} is already pending",
                transaction.raw()
            ),
            Self::ClientQueueDisconnected { client } => write!(
                formatter,
                "X11 route queue disconnected for client {}",
                client.raw()
            ),
            Self::DuplicateClient { client } => {
                write!(
                    formatter,
                    "X11 route client {} is already registered",
                    client.raw()
                )
            }
            Self::InputDeliveryQueueFull => {
                formatter.write_str("X11 input delivery acknowledgement queue is full")
            }
            Self::RegistryPoisoned => formatter.write_str("X11 route registry lock poisoned"),
        }
    }
}

#[cfg(unix)]
impl std::error::Error for XServerFrontendRouteError {}

/// Engine-facing ingress and per-client queue registry for a routed X11
/// session.
///
/// Engine code sends client-addressed input and control values to the bounded
/// ingress queues, then its session loop calls [`Self::route_pending`] to move
/// them into the registered worker's private queues. The broker never
/// broadcasts a route and fails closed when its target has disappeared or is
/// backpressured.
#[cfg(unix)]
pub struct XServerFrontendRouteBroker {
    registry: XServerFrontendRouteRegistry,
    input_sender: SyncSender<XAuthorityClientInputEvent>,
    input_receiver: Receiver<XAuthorityClientInputEvent>,
    routed_input_sender: SyncSender<XAuthorityRoutedInput>,
    routed_input_receiver: Receiver<XAuthorityRoutedInput>,
    control_sender: SyncSender<XAuthorityClientControlCommand>,
    control_receiver: Receiver<XAuthorityClientControlCommand>,
    acknowledgement_receiver: Option<Receiver<XAuthorityClientControlAck>>,
    source_payload_receiver: Receiver<crate::ClipboardSourcePayload>,
}

/// Cloneable protocol-feedback handle for Engine/backend presentation code.
///
/// This handle can outlive the broker value moved into the X11 service loop,
/// but it exposes only frontend protocol completion. It cannot route input,
/// mutate scene state, submit scanout, or access native renderer resources.
#[cfg(unix)]
#[derive(Clone)]
pub struct XServerFrontendProtocolRouter {
    registry: XServerFrontendRouteRegistry,
}

#[cfg(unix)]
impl XServerFrontendProtocolRouter {
    pub fn route_present_complete(
        &self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: XPresentCompletionMode,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry
            .route_present_complete(transaction, ust, msc, mode)
    }

    pub fn route_present_idle(
        &self,
        transaction: TransactionId,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry.route_present_idle(transaction)
    }
}

#[cfg(unix)]
impl XServerFrontendRouteBroker {
    pub fn new(queue_capacity: NonZeroUsize) -> Self {
        let capacity = queue_capacity.get();
        let (acknowledgement_sender, acknowledgement_receiver) = sync_channel(capacity);
        Self::with_transports(
            queue_capacity,
            acknowledgement_sender,
            Some(acknowledgement_receiver),
            None,
        )
    }

    /// Creates a broker whose control acknowledgements return to the supplied
    /// Engine-owned bounded queue.
    pub fn with_control_ack_sender(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
    ) -> Self {
        Self::with_transports(queue_capacity, acknowledgement_sender, None, None)
    }

    /// Creates a broker whose focus/configure and input-flush acknowledgements
    /// return through Engine-owned bounded queues.
    pub fn with_control_and_input_delivery_senders(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        input_delivery_sender: SyncSender<XAuthorityClientInputDelivery>,
    ) -> Self {
        Self::with_transports(
            queue_capacity,
            acknowledgement_sender,
            None,
            Some(input_delivery_sender),
        )
    }

    pub fn with_control_and_input_delivery_senders_and_xkb_config(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        input_delivery_sender: SyncSender<XAuthorityClientInputDelivery>,
        xkb_config: crate::XkbRmlvoConfig,
    ) -> Result<Self, crate::XkbKeyboardError> {
        crate::XkbKeyboardState::new(&xkb_config)?;
        let mut broker = Self::with_transports(
            queue_capacity,
            acknowledgement_sender,
            None,
            Some(input_delivery_sender),
        );
        broker.registry.xkb_config = xkb_config.clone();
        broker.registry.xkb_worker = XkbKeyboardWorker::spawn(xkb_config);
        Ok(broker)
    }

    fn with_transports(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        acknowledgement_receiver: Option<Receiver<XAuthorityClientControlAck>>,
        input_delivery_sender: Option<SyncSender<XAuthorityClientInputDelivery>>,
    ) -> Self {
        let capacity = queue_capacity.get();
        let (input_sender, input_receiver) = sync_channel(capacity);
        let (routed_input_sender, routed_input_receiver) = sync_channel(capacity);
        let (control_sender, control_receiver) = sync_channel(capacity);
        let (source_payload_sender, source_payload_receiver) = sync_channel(capacity);
        Self {
            registry: XServerFrontendRouteRegistry {
                clients: Arc::new(Mutex::new(BTreeMap::new())),
                surfaces: Arc::new(Mutex::new(BTreeMap::new())),
                randr_subscriptions: Arc::new(Mutex::new(BTreeMap::new())),
                present_subscriptions: Arc::new(Mutex::new(BTreeMap::new())),
                pending_presentations: Arc::new(XPendingPresentRegistry::default()),
                pointer_state: Arc::new(Mutex::new(BTreeMap::new())),
                input_authority: Arc::new(Mutex::new(crate::XInputAuthorityState::default())),
                frozen_input: Arc::new(Mutex::new(VecDeque::new())),
                xkb_config: crate::XkbRmlvoConfig::default(),
                xkb_worker: XkbKeyboardWorker::spawn(crate::XkbRmlvoConfig::default()),
                acknowledgement_sender,
                input_delivery_sender,
                per_client_queue_capacity: queue_capacity,
                source_payload_sender,
            },
            input_sender,
            input_receiver,
            routed_input_sender,
            routed_input_receiver,
            control_sender,
            control_receiver,
            acknowledgement_receiver,
            source_payload_receiver,
        }
    }

    pub fn input_sender(&self) -> SyncSender<XAuthorityClientInputEvent> {
        self.input_sender.clone()
    }

    pub fn routed_input_sender(&self) -> SyncSender<XAuthorityRoutedInput> {
        self.routed_input_sender.clone()
    }

    pub fn control_sender(&self) -> SyncSender<XAuthorityClientControlCommand> {
        self.control_sender.clone()
    }

    pub fn recv_control_ack_timeout(
        &self,
        timeout: Duration,
    ) -> Result<XAuthorityClientControlAck, RecvTimeoutError> {
        self.acknowledgement_receiver
            .as_ref()
            .ok_or(RecvTimeoutError::Disconnected)?
            .recv_timeout(timeout)
    }

    pub fn recv_clipboard_source_payload_timeout(
        &self,
        timeout: Duration,
    ) -> Result<crate::ClipboardSourcePayload, RecvTimeoutError> {
        self.source_payload_receiver.recv_timeout(timeout)
    }

    /// Routes every value currently available at the bounded ingress.
    pub fn route_pending(&mut self) -> Result<usize, XServerFrontendRouteError> {
        let mut routed = 0usize;
        loop {
            let mut progressed = false;
            match self.routed_input_receiver.try_recv() {
                Ok(route) => {
                    match self.registry.route_engine_input(route) {
                        Ok(()) => routed = routed.saturating_add(1),
                        Err(
                            XServerFrontendRouteError::UnknownSurface { .. }
                            | XServerFrontendRouteError::ClientQueueDisconnected { .. }
                            | XServerFrontendRouteError::UnknownClient { .. },
                        ) => {}
                        Err(error) => return Err(error),
                    }
                    progressed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
            match self.input_receiver.try_recv() {
                Ok(route) => {
                    if let Err(error) = self.registry.route_input(route) {
                        self.registry.send_input_delivery(
                            route.client,
                            route.delivery,
                            XAuthorityInputDeliveryOutcome::RouteRejected,
                        )?;
                        return Err(error);
                    }
                    routed = routed.saturating_add(1);
                    progressed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
            match self.control_receiver.try_recv() {
                Ok(route) => {
                    self.registry.route_control(route)?;
                    routed = routed.saturating_add(1);
                    progressed = true;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
            }
            let thawed = self.registry.drain_thawed_input()?;
            if thawed != 0 {
                routed = routed.saturating_add(thawed);
                progressed = true;
            }
            if !progressed {
                return Ok(routed);
            }
        }
    }

    pub fn registered_client_count(&self) -> usize {
        self.registry.registered_client_count()
    }

    pub fn protocol_router(&self) -> XServerFrontendProtocolRouter {
        XServerFrontendProtocolRouter {
            registry: self.registry.clone(),
        }
    }

    pub fn route_present_complete(
        &self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: XPresentCompletionMode,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry
            .route_present_complete(transaction, ust, msc, mode)
    }

    pub fn route_present_idle(
        &self,
        transaction: TransactionId,
    ) -> Result<bool, XServerFrontendRouteError> {
        self.registry.route_present_idle(transaction)
    }
}

#[cfg(unix)]
#[derive(Clone)]
struct XServerFrontendRouteRegistry {
    clients: Arc<Mutex<BTreeMap<XServerFrontendClientId, XServerFrontendClientRouteSenders>>>,
    surfaces: Arc<Mutex<BTreeMap<SurfaceId, XServerFrontendSurfaceRoute>>>,
    randr_subscriptions: Arc<Mutex<BTreeMap<XServerFrontendClientId, (XResourceId, u16)>>>,
    present_subscriptions:
        Arc<Mutex<BTreeMap<(XServerFrontendClientId, XResourceId), XPresentSubscription>>>,
    pending_presentations: Arc<XPendingPresentRegistry>,
    pointer_state: Arc<Mutex<BTreeMap<SeatId, crate::XCorePointerMapper>>>,
    input_authority: Arc<Mutex<crate::XInputAuthorityState>>,
    frozen_input: Arc<Mutex<VecDeque<XDeferredRoutedInput>>>,
    xkb_config: crate::XkbRmlvoConfig,
    xkb_worker: XkbKeyboardWorker,
    acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
    input_delivery_sender: Option<SyncSender<XAuthorityClientInputDelivery>>,
    per_client_queue_capacity: NonZeroUsize,
    source_payload_sender: SyncSender<crate::ClipboardSourcePayload>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct XServerFrontendSurfaceRoute {
    client: XServerFrontendClientId,
    namespace: NamespaceId,
    window: XResourceId,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct XPresentSubscription {
    event_id: XResourceId,
    window: XResourceId,
    mask: u32,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct XPendingPresent {
    client: XServerFrontendClientId,
    window: XResourceId,
    pixmap: XResourceId,
    serial: u32,
    idle_fence: Option<XResourceId>,
    completed: bool,
}

#[cfg(unix)]
#[derive(Default)]
struct XPendingPresentRegistry {
    entries: Mutex<BTreeMap<TransactionId, XPendingPresent>>,
    capacity_changed: Condvar,
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct XDeferredRoutedInput {
    client: XServerFrontendClientId,
    route: XAuthorityRoutedInput,
}

#[cfg(unix)]
#[derive(Clone)]
struct XServerFrontendClientRouteSenders {
    input: SyncSender<XAuthorityClientInputEvent>,
    control: SyncSender<XAuthorityControlCommand>,
    protocol: SyncSender<XClientEvent>,
}

#[cfg(unix)]
struct XServerFrontendClientRouteChannels {
    input: Receiver<XAuthorityClientInputEvent>,
    control: Receiver<XAuthorityControlCommand>,
    protocol: Receiver<XClientEvent>,
}

#[cfg(unix)]
struct XServerFrontendClientRouteRegistration {
    client: XServerFrontendClientId,
    clients: Arc<Mutex<BTreeMap<XServerFrontendClientId, XServerFrontendClientRouteSenders>>>,
    surfaces: Arc<Mutex<BTreeMap<SurfaceId, XServerFrontendSurfaceRoute>>>,
    randr_subscriptions: Arc<Mutex<BTreeMap<XServerFrontendClientId, (XResourceId, u16)>>>,
    present_subscriptions:
        Arc<Mutex<BTreeMap<(XServerFrontendClientId, XResourceId), XPresentSubscription>>>,
    pending_presentations: Arc<XPendingPresentRegistry>,
    frozen_input: Arc<Mutex<VecDeque<XDeferredRoutedInput>>>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
enum XkbWorkerCommand {
    Key {
        seat: SeatId,
        keycode: u32,
        pressed: bool,
    },
    Modifiers {
        seat: SeatId,
    },
}

#[cfg(unix)]
#[derive(Clone)]
struct XkbKeyboardWorker {
    commands: SyncSender<XkbWorkerCommand>,
    replies: Arc<Mutex<Receiver<Option<(u8, u16)>>>>,
}

#[cfg(unix)]
impl XkbKeyboardWorker {
    fn spawn(config: crate::XkbRmlvoConfig) -> Self {
        let (commands, command_receiver) = sync_channel(64);
        let (reply_sender, replies) = sync_channel(64);
        std::thread::Builder::new()
            .name("sophia-xkb-authority".to_owned())
            .spawn(move || {
                let mut seats = BTreeMap::<SeatId, crate::XkbKeyboardState>::new();
                while let Ok(command) = command_receiver.recv() {
                    let seat_id = match command {
                        XkbWorkerCommand::Key { seat, .. }
                        | XkbWorkerCommand::Modifiers { seat } => seat,
                    };
                    let state = seats.entry(seat_id).or_insert_with(|| {
                        crate::XkbKeyboardState::new(&config)
                            .expect("validated XKB configuration must remain compilable")
                    });
                    let reply = match command {
                        XkbWorkerCommand::Key {
                            keycode, pressed, ..
                        } => state.map_evdev_key(keycode, pressed),
                        XkbWorkerCommand::Modifiers { .. } => Some((0, state.modifier_mask())),
                    };
                    if reply_sender.send(reply).is_err() {
                        break;
                    }
                }
            })
            .expect("Sophia XKB authority worker must start");
        Self {
            commands,
            replies: Arc::new(Mutex::new(replies)),
        }
    }

    fn request(
        &self,
        command: XkbWorkerCommand,
    ) -> Result<Option<(u8, u16)>, XServerFrontendRouteError> {
        self.commands
            .try_send(command)
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        self.replies
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .recv()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)
    }
}

#[cfg(unix)]
impl XServerFrontendRouteRegistry {
    fn register_client(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<
        (
            XServerFrontendClientRouteRegistration,
            XServerFrontendClientRouteChannels,
        ),
        XServerFrontendRouteError,
    > {
        let capacity = self.per_client_queue_capacity.get();
        let (input_sender, input) = sync_channel(capacity);
        let (control_sender, control) = sync_channel(capacity);
        let (protocol_sender, protocol) = sync_channel(capacity);
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        if clients.contains_key(&client) {
            return Err(XServerFrontendRouteError::DuplicateClient { client });
        }
        clients.insert(
            client,
            XServerFrontendClientRouteSenders {
                input: input_sender,
                control: control_sender,
                protocol: protocol_sender,
            },
        );
        Ok((
            XServerFrontendClientRouteRegistration {
                client,
                clients: self.clients.clone(),
                surfaces: self.surfaces.clone(),
                randr_subscriptions: self.randr_subscriptions.clone(),
                present_subscriptions: self.present_subscriptions.clone(),
                pending_presentations: self.pending_presentations.clone(),
                frozen_input: self.frozen_input.clone(),
            },
            XServerFrontendClientRouteChannels {
                input,
                control,
                protocol,
            },
        ))
    }

    fn route_input(
        &self,
        route: XAuthorityClientInputEvent,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(route.client)?.input;
        self.route_to_client(route.client, sender, route)
    }

    fn register_surface(
        &self,
        client: XServerFrontendClientId,
        namespace: NamespaceId,
        surface: SurfaceId,
        window: XResourceId,
    ) -> Result<(), XServerFrontendRouteError> {
        self.surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .insert(
                surface,
                XServerFrontendSurfaceRoute {
                    client,
                    namespace,
                    window,
                },
            );
        Ok(())
    }

    fn select_randr_input(
        &self,
        client: XServerFrontendClientId,
        window: XResourceId,
        mask: u16,
    ) -> Result<(), XServerFrontendRouteError> {
        let mut subscriptions = self
            .randr_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        if mask == 0 {
            subscriptions.remove(&client);
        } else {
            subscriptions.insert(client, (window, mask));
        }
        Ok(())
    }

    fn select_present_input(
        &self,
        client: XServerFrontendClientId,
        event_id: XResourceId,
        window: XResourceId,
        mask: u32,
    ) -> Result<(), XServerFrontendRouteError> {
        let mut subscriptions = self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let key = (client, event_id);
        if mask == 0 {
            subscriptions.remove(&key);
        } else {
            subscriptions.insert(
                key,
                XPresentSubscription {
                    event_id,
                    window,
                    mask,
                },
            );
        }
        Ok(())
    }

    fn queue_present(
        &self,
        transaction: TransactionId,
        client: XServerFrontendClientId,
        window: XResourceId,
        pixmap: XResourceId,
        serial: u32,
        idle_fence: Option<XResourceId>,
    ) -> Result<(), XServerFrontendRouteError> {
        self.surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .find_map(|(surface, route)| {
                (route.client == client && route.window == window).then_some(*surface)
            })
            .ok_or(XServerFrontendRouteError::UnknownClient { client })?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut pending = self
            .pending_presentations
            .entries
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        if pending.contains_key(&transaction) {
            return Err(XServerFrontendRouteError::DuplicatePresentation { transaction });
        }
        while pending
            .values()
            .filter(|presentation| presentation.client == client)
            .count()
            >= self.per_client_queue_capacity.get()
        {
            let now = Instant::now();
            if now >= deadline {
                return Err(XServerFrontendRouteError::ClientQueueFull { client });
            }
            let (next, wait) = self
                .pending_presentations
                .capacity_changed
                .wait_timeout(pending, deadline.saturating_duration_since(now))
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            pending = next;
            if wait.timed_out()
                && pending
                    .values()
                    .filter(|presentation| presentation.client == client)
                    .count()
                    >= self.per_client_queue_capacity.get()
            {
                return Err(XServerFrontendRouteError::ClientQueueFull { client });
            }
        }
        pending.insert(
            transaction,
            XPendingPresent {
                client,
                window,
                pixmap,
                serial,
                idle_fence,
                completed: false,
            },
        );
        Ok(())
    }

    fn route_present_complete(
        &self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: XPresentCompletionMode,
    ) -> Result<bool, XServerFrontendRouteError> {
        let presentation = {
            let mut pending = self
                .pending_presentations
                .entries
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            let Some(presentation) = pending.get_mut(&transaction) else {
                return Ok(false);
            };
            if presentation.completed {
                return Ok(false);
            }
            presentation.completed = true;
            *presentation
        };
        let subscriptions = self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .filter_map(|((client, _), subscription)| {
                (*client == presentation.client
                    && subscription.window == presentation.window
                    && subscription.mask & (1 << 1) != 0)
                    .then_some(*subscription)
            })
            .collect::<Vec<_>>();
        if subscriptions.is_empty() {
            return Ok(false);
        }
        for subscription in subscriptions {
            self.route_protocol(
                presentation.client,
                XClientEvent::PresentCompleteNotify {
                    sequence: 0,
                    event_id: subscription.event_id,
                    window: presentation.window,
                    serial: presentation.serial,
                    ust,
                    msc,
                    mode: mode as u8,
                },
            )?;
        }
        Ok(true)
    }

    fn cancel_present(&self, transaction: TransactionId) -> Result<(), XServerFrontendRouteError> {
        self.pending_presentations
            .entries
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .remove(&transaction);
        self.pending_presentations.capacity_changed.notify_all();
        Ok(())
    }

    fn route_present_idle(
        &self,
        transaction: TransactionId,
    ) -> Result<bool, XServerFrontendRouteError> {
        let presentation = {
            let mut pending = self
                .pending_presentations
                .entries
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            let Some(front) = pending.get(&transaction).copied() else {
                return Ok(false);
            };
            if !front.completed {
                return Ok(false);
            }
            pending.remove(&transaction);
            self.pending_presentations.capacity_changed.notify_all();
            front
        };
        let subscriptions = self
            .present_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .filter_map(|((client, _), subscription)| {
                (*client == presentation.client
                    && subscription.window == presentation.window
                    && subscription.mask & (1 << 2) != 0)
                    .then_some(*subscription)
            })
            .collect::<Vec<_>>();
        if subscriptions.is_empty() {
            return Ok(false);
        }
        for subscription in subscriptions {
            self.route_protocol(
                presentation.client,
                XClientEvent::PresentIdleNotify {
                    sequence: 0,
                    event_id: subscription.event_id,
                    window: presentation.window,
                    serial: presentation.serial,
                    pixmap: presentation.pixmap,
                    idle_fence: presentation.idle_fence,
                },
            )?;
        }
        Ok(true)
    }

    fn broadcast_randr_update(
        &self,
        snapshot: &sophia_protocol::OutputTopologySnapshot,
    ) -> Result<usize, XServerFrontendRouteError> {
        let size = snapshot
            .root_size()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let width =
            u16::try_from(size.width).map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let height =
            u16::try_from(size.height).map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let mm_width = u16::try_from((i64::from(size.width) * 254 + 480) / 960)
            .unwrap_or(u16::MAX)
            .max(1);
        let mm_height = u16::try_from((i64::from(size.height) * 254 + 480) / 960)
            .unwrap_or(u16::MAX)
            .max(1);
        let timestamp = u32::try_from(snapshot.generation)
            .unwrap_or(u32::MAX)
            .max(1);
        let subscriptions = self
            .randr_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .clone();
        let mut delivered = 0usize;
        for (client, (window, mask)) in subscriptions {
            if mask & 1 != 0 {
                self.route_protocol(
                    client,
                    XClientEvent::RandrScreenChange {
                        sequence: 0,
                        timestamp,
                        config_timestamp: timestamp,
                        root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                        request_window: window,
                        width,
                        height,
                        mm_width,
                        mm_height,
                    },
                )?;
                delivered = delivered.saturating_add(1);
            }
            for output in &snapshot.outputs {
                let identity = crate::dispatch::stable_randr_identity(output.output.raw());
                let crtc = 0x1000_0000 | identity;
                let output_id = 0x2000_0000 | identity;
                let mode = crate::dispatch::stable_randr_mode_id(
                    output.logical.width,
                    output.logical.height,
                    output.refresh_millihz,
                );
                if mask & (1 << 1) != 0 {
                    self.route_protocol(
                        client,
                        XClientEvent::RandrCrtcChange {
                            sequence: 0,
                            timestamp,
                            window,
                            crtc,
                            mode,
                            x: i16::try_from(output.logical.x).unwrap_or(i16::MAX),
                            y: i16::try_from(output.logical.y).unwrap_or(i16::MAX),
                            width: u16::try_from(output.logical.width).unwrap_or(u16::MAX),
                            height: u16::try_from(output.logical.height).unwrap_or(u16::MAX),
                        },
                    )?;
                    delivered = delivered.saturating_add(1);
                }
                if mask & (1 << 2) != 0 {
                    self.route_protocol(
                        client,
                        XClientEvent::RandrOutputChange {
                            sequence: 0,
                            timestamp,
                            window,
                            output: output_id,
                            crtc,
                            mode,
                        },
                    )?;
                    delivered = delivered.saturating_add(1);
                }
            }
            if mask & (1 << 6) != 0 {
                self.route_protocol(
                    client,
                    XClientEvent::RandrResourceChange {
                        sequence: 0,
                        timestamp,
                        window,
                    },
                )?;
                delivered = delivered.saturating_add(1);
            }
        }
        Ok(delivered)
    }

    fn route_engine_input(
        &self,
        route: XAuthorityRoutedInput,
    ) -> Result<(), XServerFrontendRouteError> {
        let surface_route = self
            .surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .get(&route.request.target_surface)
            .copied()
            .ok_or(XServerFrontendRouteError::UnknownSurface {
                surface: route.request.target_surface,
            })?;
        if self.route_is_frozen(&route, surface_route.namespace)? {
            let mut frozen = self
                .frozen_input
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
            if frozen.len() >= self.per_client_queue_capacity.get() {
                drop(frozen);
                self.send_input_delivery(
                    surface_route.client,
                    route.delivery,
                    XAuthorityInputDeliveryOutcome::RouteRejected,
                )?;
                return Err(XServerFrontendRouteError::ClientQueueFull {
                    client: surface_route.client,
                });
            }
            frozen.push_back(XDeferredRoutedInput {
                client: surface_route.client,
                route,
            });
            return Ok(());
        }
        let mut client = surface_route.client;
        // Engine already selected the committed target surface. Preserve its
        // owning window as the start of core propagation; X grabs may replace
        // it below, but event-mask update order must never choose the target.
        let mut target_window = Some(surface_route.window);
        let mut pointers = self
            .pointer_state
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let pointer = pointers
            .entry(route.request.seat)
            .or_insert_with(crate::XCorePointerMapper::new);
        let time_msec = u32::try_from(route.request.time_msec).unwrap_or(u32::MAX);
        let event = match route.request.kind {
            InputEventKind::Key { keycode, pressed } => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .keyboard_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                let Some((keycode, state)) = self.xkb_worker.request(XkbWorkerCommand::Key {
                    seat: route.request.seat,
                    keycode,
                    pressed,
                })?
                else {
                    return self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    );
                };
                let passive = if pressed {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .activate_key(surface_route.namespace, keycode, state & 0xff)
                } else {
                    None
                };
                if let Some(grab) = passive {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                if !pressed {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .release_key(surface_route.namespace, keycode);
                }
                XAuthorityInputEvent::Key(XAuthorityKeyEvent {
                    keycode,
                    pressed,
                    state: state | pointer.state(),
                    time_msec,
                })
            }
            InputEventKind::PointerMotion => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .pointer_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                    kind: XAuthorityPointerEventKind::Motion,
                    surface: route.request.target_surface,
                    root_x: clamp_input_coordinate(route.request.global_position.x),
                    root_y: clamp_input_coordinate(route.request.global_position.y),
                    event_x: clamp_input_coordinate(route.request.local_position.x),
                    event_y: clamp_input_coordinate(route.request.local_position.y),
                    state: self
                        .xkb_worker
                        .request(XkbWorkerCommand::Modifiers {
                            seat: route.request.seat,
                        })?
                        .map_or(0, |(_, state)| state)
                        | pointer.state(),
                    time_msec,
                })
            }
            InputEventKind::PointerButton { button, pressed } => {
                if let Some(grab) = self
                    .input_authority
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .pointer_grab(surface_route.namespace)
                {
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                }
                let Some((button, state)) = pointer.map_evdev_button(button, pressed) else {
                    return self.send_input_delivery(
                        client,
                        route.delivery,
                        XAuthorityInputDeliveryOutcome::RouteRejected,
                    );
                };
                if pressed {
                    let grab = self
                        .input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .activate_button(
                            surface_route.namespace,
                            button,
                            state & 0xff,
                            crate::XActiveInputGrab {
                                owner: surface_route.client.raw(),
                                window: surface_route.window,
                                owner_events: true,
                                pointer_mode: 1,
                                keyboard_mode: 1,
                                event_mask: u16::MAX,
                            },
                        );
                    client = XServerFrontendClientId(grab.owner);
                    target_window = Some(if grab.owner_events && client == surface_route.client {
                        surface_route.window
                    } else {
                        grab.window
                    });
                } else {
                    self.input_authority
                        .lock()
                        .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                        .release_button(surface_route.namespace, button);
                }
                XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                    kind: XAuthorityPointerEventKind::Button { button, pressed },
                    surface: route.request.target_surface,
                    root_x: clamp_input_coordinate(route.request.global_position.x),
                    root_y: clamp_input_coordinate(route.request.global_position.y),
                    event_x: clamp_input_coordinate(route.request.local_position.x),
                    event_y: clamp_input_coordinate(route.request.local_position.y),
                    state: self
                        .xkb_worker
                        .request(XkbWorkerCommand::Modifiers {
                            seat: route.request.seat,
                        })?
                        .map_or(0, |(_, state)| state)
                        | state,
                    time_msec,
                })
            }
        };
        drop(pointers);
        let (xi_device, selected_type) = match event {
            XAuthorityInputEvent::Key(key) => (3, if key.pressed { 2 } else { 3 }),
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind: XAuthorityPointerEventKind::Button { pressed, .. },
                ..
            }) => (2, if pressed { 4 } else { 5 }),
            XAuthorityInputEvent::Pointer(_) => (2, 6),
        };
        let event_window = target_window.unwrap_or(surface_route.window);
        let xi_event_type = self
            .input_authority
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .xi_event_selected(
                surface_route.namespace,
                client.raw(),
                event_window,
                xi_device,
                selected_type,
            )
            .then_some(selected_type);
        let transition_types: &[u16] = if xi_device == 3 { &[9, 10] } else { &[7, 8] };
        let authority = self
            .input_authority
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let xi_transition_mask = transition_types.iter().fold(0u16, |mask, event_type| {
            if authority.xi_event_selected(
                surface_route.namespace,
                client.raw(),
                event_window,
                xi_device,
                *event_type,
            ) {
                mask | (1 << event_type)
            } else {
                mask
            }
        });
        self.route_input(XAuthorityClientInputEvent {
            client,
            event,
            target_window,
            xi_event_type,
            xi_transition_mask,
            delivery: route.delivery,
        })
    }

    fn route_is_frozen(
        &self,
        route: &XAuthorityRoutedInput,
        namespace: NamespaceId,
    ) -> Result<bool, XServerFrontendRouteError> {
        let authority = self
            .input_authority
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        Ok(match route.request.kind {
            InputEventKind::Key { .. } => authority.keyboard_frozen(namespace),
            InputEventKind::PointerMotion | InputEventKind::PointerButton { .. } => {
                authority.pointer_frozen(namespace)
            }
        })
    }

    fn drain_thawed_input(&self) -> Result<usize, XServerFrontendRouteError> {
        let queued = self
            .frozen_input
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .len();
        let mut routed = 0usize;
        for _ in 0..queued {
            let deferred = self
                .frozen_input
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                .pop_front();
            let Some(deferred) = deferred else { break };
            let route = deferred.route;
            let surface_route = self
                .surfaces
                .lock()
                .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                .get(&route.request.target_surface)
                .copied();
            let Some(surface_route) = surface_route else {
                self.send_input_delivery(
                    deferred.client,
                    route.delivery,
                    XAuthorityInputDeliveryOutcome::RouteRejected,
                )?;
                continue;
            };
            if self.route_is_frozen(&route, surface_route.namespace)? {
                self.frozen_input
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .push_back(XDeferredRoutedInput {
                        client: deferred.client,
                        route,
                    });
            } else {
                self.route_engine_input(route)?;
                routed = routed.saturating_add(1);
            }
        }
        Ok(routed)
    }

    fn route_control(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(route.client)?.control;
        self.route_to_client(route.client, sender, route.command)
    }

    fn route_protocol(
        &self,
        client: XServerFrontendClientId,
        event: XClientEvent,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(client)?.protocol;
        match self.route_to_client(client, sender, event) {
            Err(XServerFrontendRouteError::ClientQueueDisconnected { .. }) => Ok(()),
            result => result,
        }
    }

    fn client_senders(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<XServerFrontendClientRouteSenders, XServerFrontendRouteError> {
        self.clients
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .get(&client)
            .cloned()
            .ok_or(XServerFrontendRouteError::UnknownClient { client })
    }

    fn route_to_client<T>(
        &self,
        client: XServerFrontendClientId,
        sender: SyncSender<T>,
        value: T,
    ) -> Result<(), XServerFrontendRouteError> {
        match sender.try_send(value) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                Err(XServerFrontendRouteError::ClientQueueFull { client })
            }
            Err(TrySendError::Disconnected(_)) => {
                self.clients
                    .lock()
                    .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
                    .remove(&client);
                Err(XServerFrontendRouteError::ClientQueueDisconnected { client })
            }
        }
    }

    fn registered_client_count(&self) -> usize {
        self.clients
            .lock()
            .map(|clients| clients.len())
            .unwrap_or(0)
    }

    fn send_input_delivery(
        &self,
        client: XServerFrontendClientId,
        delivery: Option<XAuthorityInputDeliveryId>,
        outcome: XAuthorityInputDeliveryOutcome,
    ) -> Result<(), XServerFrontendRouteError> {
        let Some(delivery) = delivery else {
            return Ok(());
        };
        let Some(sender) = self.input_delivery_sender.as_ref() else {
            return Ok(());
        };
        match sender.try_send(XAuthorityClientInputDelivery {
            client,
            delivery,
            outcome,
        }) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
            Err(TrySendError::Full(_)) => Err(XServerFrontendRouteError::InputDeliveryQueueFull),
        }
    }
}

#[cfg(unix)]
impl Drop for XServerFrontendClientRouteRegistration {
    fn drop(&mut self) {
        if let Ok(mut clients) = self.clients.lock() {
            clients.remove(&self.client);
        }
        if let Ok(mut surfaces) = self.surfaces.lock() {
            surfaces.retain(|_, route| route.client != self.client);
        }
        if let Ok(mut subscriptions) = self.randr_subscriptions.lock() {
            subscriptions.remove(&self.client);
        }
        if let Ok(mut subscriptions) = self.present_subscriptions.lock() {
            subscriptions.retain(|(client, _), _| *client != self.client);
        }
        if let Ok(mut pending) = self.pending_presentations.entries.lock() {
            pending.retain(|_, presentation| presentation.client != self.client);
            self.pending_presentations.capacity_changed.notify_all();
        }
        if let Ok(mut frozen) = self.frozen_input.lock() {
            frozen.retain(|route| route.client != self.client);
        }
    }
}

#[cfg(unix)]
fn clamp_input_coordinate(value: f64) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    value
        .floor()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

#[cfg(unix)]
fn encode_xi_device_event(
    byte_order: XByteOrder,
    sequence: u16,
    event_type: u16,
    event: XAuthorityInputEvent,
    event_window: XResourceId,
) -> Vec<u8> {
    let (device, time, detail, root_x, root_y, event_x, event_y, state) = match event {
        XAuthorityInputEvent::Key(key) => (
            3,
            key.time_msec,
            u32::from(key.keycode),
            0,
            0,
            0,
            0,
            key.state,
        ),
        XAuthorityInputEvent::Pointer(pointer) => (
            2,
            pointer.time_msec,
            match pointer.kind {
                XAuthorityPointerEventKind::Button { button, .. } => u32::from(button),
                XAuthorityPointerEventKind::Motion => 0,
            },
            pointer.root_x,
            pointer.root_y,
            pointer.event_x,
            pointer.event_y,
            pointer.state,
        ),
    };
    let mut out = vec![0; 80];
    out[0] = 35;
    out[1] = crate::X_INPUT_MAJOR_OPCODE;
    write_xi_u16(byte_order, &mut out[2..4], sequence);
    write_xi_u32(byte_order, &mut out[4..8], 12);
    write_xi_u16(byte_order, &mut out[8..10], event_type);
    write_xi_u16(byte_order, &mut out[10..12], device);
    write_xi_u32(byte_order, &mut out[12..16], time);
    write_xi_u32(byte_order, &mut out[16..20], detail);
    write_xi_u32(byte_order, &mut out[20..24], X_SETUP_DEFAULT_ROOT);
    write_xi_u32(
        byte_order,
        &mut out[24..28],
        u32::try_from(event_window.local.raw()).unwrap_or(0),
    );
    write_xi_u32(
        byte_order,
        &mut out[32..36],
        (i32::from(root_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[36..40],
        (i32::from(root_y) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[40..44],
        (i32::from(event_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[44..48],
        (i32::from(event_y) << 16) as u32,
    );
    write_xi_u16(byte_order, &mut out[52..54], device);
    write_xi_u32(byte_order, &mut out[72..76], u32::from(state & 0xff));
    out
}

#[cfg(unix)]
fn encode_xi_crossing_event(
    byte_order: XByteOrder,
    sequence: u16,
    event_type: u16,
    event: XAuthorityInputEvent,
    event_window: XResourceId,
) -> Vec<u8> {
    let (device, time, root_x, root_y, event_x, event_y, state) = match event {
        XAuthorityInputEvent::Key(key) => (3, key.time_msec, 0, 0, 0, 0, key.state),
        XAuthorityInputEvent::Pointer(pointer) => (
            2,
            pointer.time_msec,
            pointer.root_x,
            pointer.root_y,
            pointer.event_x,
            pointer.event_y,
            pointer.state,
        ),
    };
    let mut out = vec![0; 72];
    out[0] = 35;
    out[1] = crate::X_INPUT_MAJOR_OPCODE;
    write_xi_u16(byte_order, &mut out[2..4], sequence);
    write_xi_u32(byte_order, &mut out[4..8], 10);
    write_xi_u16(byte_order, &mut out[8..10], event_type);
    write_xi_u16(byte_order, &mut out[10..12], device);
    write_xi_u32(byte_order, &mut out[12..16], time);
    write_xi_u16(byte_order, &mut out[16..18], device);
    out[18] = 0;
    out[19] = 3;
    write_xi_u32(byte_order, &mut out[20..24], X_SETUP_DEFAULT_ROOT);
    write_xi_u32(
        byte_order,
        &mut out[24..28],
        u32::try_from(event_window.local.raw()).unwrap_or(0),
    );
    write_xi_u32(
        byte_order,
        &mut out[32..36],
        (i32::from(root_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[36..40],
        (i32::from(root_y) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[40..44],
        (i32::from(event_x) << 16) as u32,
    );
    write_xi_u32(
        byte_order,
        &mut out[44..48],
        (i32::from(event_y) << 16) as u32,
    );
    out[48] = 1;
    out[49] = 1;
    write_xi_u32(byte_order, &mut out[64..68], u32::from(state & 0xff));
    out
}

#[cfg(unix)]
fn write_xi_u16(byte_order: XByteOrder, out: &mut [u8], value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

#[cfg(unix)]
fn write_xi_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

#[cfg(unix)]
enum X11InputEventReceiver {
    Plain(Receiver<XAuthorityInputEvent>),
    Routed {
        receiver: Receiver<XAuthorityClientInputEvent>,
        deliveries: Option<SyncSender<XAuthorityClientInputDelivery>>,
    },
}

#[cfg(unix)]
impl X11InputEventReceiver {
    fn recv_timeout(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<
        (
            XAuthorityInputEvent,
            Option<XResourceId>,
            Option<u16>,
            u16,
            Option<XAuthorityInputDeliveryId>,
        ),
        RecvTimeoutError,
    > {
        loop {
            match self {
                Self::Plain(receiver) => {
                    return receiver
                        .recv_timeout(Duration::from_millis(10))
                        .map(|event| (event, None, None, 0, None));
                }
                Self::Routed { receiver, .. } => {
                    match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(route) if route.client == client => {
                            return Ok((
                                route.event,
                                route.target_window,
                                route.xi_event_type,
                                route.xi_transition_mask,
                                route.delivery,
                            ));
                        }
                        // Drop one misaddressed route, then let the writer loop
                        // observe its stop flag before it receives again.
                        Ok(_) => return Err(RecvTimeoutError::Timeout),
                        Err(error) => return Err(error),
                    }
                }
            }
        }
    }

    fn send_delivery(
        &self,
        client: XServerFrontendClientId,
        delivery: Option<XAuthorityInputDeliveryId>,
        outcome: XAuthorityInputDeliveryOutcome,
    ) -> Result<(), X11SetupSocketError> {
        let Some(delivery) = delivery else {
            return Ok(());
        };
        let Self::Routed {
            deliveries: Some(sender),
            ..
        } = self
        else {
            return Ok(());
        };
        match sender.try_send(XAuthorityClientInputDelivery {
            client,
            delivery,
            outcome,
        }) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
            Err(TrySendError::Full(_)) => Err(X11SetupSocketError::new(
                "X11 input delivery acknowledgement channel is full",
            )),
        }
    }
}

#[cfg(unix)]
enum X11ControlChannels {
    Routed {
        receiver: Receiver<XAuthorityClientControlCommand>,
        acknowledgements: SyncSender<XAuthorityClientControlAck>,
    },
    ClientBound {
        receiver: Receiver<XAuthorityControlCommand>,
        acknowledgements: SyncSender<XAuthorityClientControlAck>,
    },
}

#[cfg(unix)]
impl X11ControlChannels {
    fn recv_timeout(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<XAuthorityControlCommand, RecvTimeoutError> {
        loop {
            match self {
                Self::Routed { receiver, .. } => {
                    match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(route) if route.client == client => return Ok(route.command),
                        // Drop one misaddressed route, then let the writer
                        // loop observe its stop flag before it receives again.
                        Ok(_) => return Err(RecvTimeoutError::Timeout),
                        Err(error) => return Err(error),
                    }
                }
                Self::ClientBound { receiver, .. } => {
                    return receiver.recv_timeout(Duration::from_millis(10));
                }
            }
        }
    }

    fn send_ack(
        &self,
        client: XServerFrontendClientId,
        acknowledgement: XAuthorityControlAck,
    ) -> Result<(), X11SetupSocketError> {
        match self {
            Self::Routed {
                acknowledgements, ..
            }
            | Self::ClientBound {
                acknowledgements, ..
            } => match acknowledgements.try_send(XAuthorityClientControlAck {
                client,
                acknowledgement,
            }) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => Ok(()),
                Err(TrySendError::Full(_)) => Err(X11SetupSocketError::new(
                    "X11 control acknowledgement channel is full",
                )),
            },
        }
    }
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
#[derive(Clone)]
pub struct X11CoreSocketServerState {
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    atoms: Arc<Mutex<XAtomTable>>,
    properties: Arc<Mutex<XPropertyTable>>,
    clients: Arc<Mutex<X11CoreClientLeaseState>>,
    next_transaction_id: Arc<AtomicU64>,
    render_device_provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
}

#[cfg(unix)]
impl core::fmt::Debug for X11CoreSocketServerState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("X11CoreSocketServerState")
            .field("runtime", &self.runtime)
            .field("atoms", &self.atoms)
            .field("properties", &self.properties)
            .field("clients", &self.clients)
            .field(
                "next_transaction_id",
                &self.next_transaction_id.load(Ordering::Relaxed),
            )
            .field(
                "has_render_device_provider",
                &self.render_device_provider.is_some(),
            )
            .finish()
    }
}

/// The small part of socket state that must be serialized across connection
/// setup and teardown. Protocol dispatch itself uses the independent runtime,
/// atom, and property locks above.
#[cfg(unix)]
#[derive(Debug)]
struct X11CoreClientLeaseState {
    next_client_resource_range: u16,
    next_client_id: u64,
    client_leases: BTreeMap<XServerFrontendClientId, XServerFrontendClientLease>,
}

#[cfg(unix)]
impl Default for X11CoreSocketServerState {
    fn default() -> Self {
        Self {
            runtime: Default::default(),
            atoms: Default::default(),
            properties: Default::default(),
            clients: Arc::new(Mutex::new(X11CoreClientLeaseState {
                next_client_resource_range: 1,
                next_client_id: 1,
                client_leases: Default::default(),
            })),
            next_transaction_id: Arc::new(AtomicU64::new(1)),
            render_device_provider: None,
        }
    }
}

#[cfg(unix)]
impl X11CoreSocketServerState {
    pub fn new() -> Self {
        Self::default()
    }

    fn allocate_transaction(&self) -> Result<TransactionId, X11SetupSocketError> {
        let raw = self
            .next_transaction_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| X11SetupSocketError::new("X11 transaction identity space exhausted"))?;
        Ok(TransactionId::from_raw(raw))
    }

    pub fn with_render_device_provider(
        mut self,
        provider: Arc<dyn XServerFrontendRenderDeviceProvider>,
    ) -> Self {
        self.render_device_provider = Some(provider);
        self
    }

    fn with_optional_render_device_provider(
        mut self,
        provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
    ) -> Self {
        self.render_device_provider = provider;
        self
    }

    fn open_render_device_fd(&self) -> Result<OwnedFd, XServerFrontendRenderDeviceError> {
        self.render_device_provider
            .as_ref()
            .ok_or(XServerFrontendRenderDeviceError::Unavailable)?
            .open_render_device_fd()
    }

    fn has_render_device_provider(&self) -> bool {
        self.render_device_provider.is_some()
    }

    pub fn with_output_topology(
        output_topology: sophia_protocol::OutputTopologySnapshot,
    ) -> Result<Self, X11SetupSocketError> {
        let runtime =
            XAuthorityRuntime::with_output_topology(output_topology).map_err(|error| {
                X11SetupSocketError::new(format!("invalid Engine output topology: {error:?}"))
            })?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            ..Self::default()
        })
    }

    pub fn with_output_topology_and_xkb_config(
        output_topology: sophia_protocol::OutputTopologySnapshot,
        xkb_config: &crate::XkbRmlvoConfig,
    ) -> Result<Self, X11SetupSocketError> {
        let runtime =
            XAuthorityRuntime::with_output_topology_and_xkb_config(output_topology, xkb_config)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            ..Self::default()
        })
    }

    fn next_client_setup_success(
        &self,
    ) -> Result<(XServerFrontendClientLease, XSetupSuccess), X11SetupSocketError> {
        let root_size = self
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
            .output_topology()
            .root_size()
            .map_err(|error| {
                X11SetupSocketError::new(format!(
                    "invalid Engine output topology during setup: {error:?}"
                ))
            })?;
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?;
        if clients.next_client_resource_range > X11_MAX_CLIENT_RESOURCE_RANGES {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend exhausted X11 client resource ranges",
            ));
        }
        let resource_id_base =
            u32::from(clients.next_client_resource_range) * X11_CLIENT_RESOURCE_RANGE_SIZE;
        clients.next_client_resource_range = clients.next_client_resource_range.saturating_add(1);
        let client = XServerFrontendClientId(clients.next_client_id);
        clients.next_client_id = clients.next_client_id.checked_add(1).ok_or_else(|| {
            X11SetupSocketError::new("Sophia X Server Frontend exhausted client identities")
        })?;
        let resource_id_range = crate::XWireClientResourceRange {
            base: resource_id_base,
            mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
        };
        Ok((
            XServerFrontendClientLease {
                client,
                resource_id_range,
            },
            XSetupSuccess {
                resource_id_base,
                resource_id_mask: X_SETUP_DEFAULT_RESOURCE_ID_MASK,
                root_size,
                ..XSetupSuccess::client_compatible()
            },
        ))
    }

    fn register_client(
        &self,
        lease: XServerFrontendClientLease,
    ) -> Result<(), X11SetupSocketError> {
        if self
            .clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?
            .client_leases
            .insert(lease.client, lease)
            .is_some()
        {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend assigned a duplicate client identity",
            ));
        }
        Ok(())
    }

    fn release_client(
        &self,
        client: XServerFrontendClientId,
    ) -> Result<XServerFrontendClientLease, X11SetupSocketError> {
        self.clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?
            .client_leases
            .remove(&client)
            .ok_or_else(|| {
                X11SetupSocketError::new("Sophia X Server Frontend lost a client connection lease")
            })
    }

    fn active_client_count(&self) -> usize {
        self.clients
            .lock()
            .map(|clients| clients.client_leases.len())
            .unwrap_or(0)
    }

    fn client_for_resource(
        &self,
        resource: XResourceId,
    ) -> Result<Option<XServerFrontendClientId>, X11SetupSocketError> {
        let raw = u32::try_from(resource.local.raw()).ok();
        let clients = self
            .clients
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 client lease lock poisoned"))?;
        Ok(raw.and_then(|raw| {
            clients.client_leases.iter().find_map(|(client, lease)| {
                lease
                    .resource_id_range
                    .owns_new_resource(raw)
                    .then_some(*client)
            })
        }))
    }
}

#[cfg(unix)]
fn release_x11_client_lease(
    state: &X11CoreSocketServerState,
    namespace: NamespaceId,
    lease: XServerFrontendClientLease,
) -> Result<crate::XAuthorityClientResourceRelease, X11SetupSocketError> {
    // Keep authority resource destruction and property removal together. X11
    // request dispatch acquires the runtime lock before the property lock, so
    // this prevents another client observing a destroyed window with stale
    // properties between the two cleanup steps.
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
    let release = runtime
        .release_client_resource_range(namespace, lease.resource_id_range)
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to release X11 client resources: {error:?}"))
        })?;
    let mut properties = state
        .properties
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 property table lock poisoned"))?;
    for window in &release.destroyed_windows {
        properties.remove_window(namespace, *window);
    }
    Ok(release)
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

/// Runs one bounded client against an explicitly assembled frontend config.
///
/// This retains the external-probe idle timeout while allowing a caller to
/// inject backend-owned capabilities such as the DRI3 render-device provider.
#[cfg(unix)]
pub fn run_x11_core_socket_server_once_config_traced_with_idle_timeout(
    config: XServerFrontendConfig,
    idle_timeout: Duration,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let listener = bind_x11_core_socket_server(config.socket_path())?;
    let state = X11CoreSocketServerState::with_output_topology_and_xkb_config(
        config.output_topology().clone(),
        config.xkb_config(),
    )?
    .with_optional_render_device_provider(config.render_device_provider());
    serve_x11_core_socket_listener_once_with_setup_authorization(
        &listener,
        config.namespace(),
        &state,
        config.setup_authorization(),
        config.admission_policy(),
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
    run_x11_core_socket_server_once_with_trace_observer(path, namespace, None, move |trace| {
        try_emit_x_authority_trace(&sender, &trace)
            .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        Ok(())
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
        Some(X11InputEventReceiver::Plain(input_receiver)),
        None,
        None,
        &XServerFrontendSetupAuthorization::default(),
        None,
        None,
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
    input_receiver: Receiver<XAuthorityClientInputEvent>,
    control_receiver: Receiver<XAuthorityClientControlCommand>,
    control_ack_sender: SyncSender<XAuthorityClientControlAck>,
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
        Some(X11InputEventReceiver::Routed {
            receiver: input_receiver,
            deliveries: None,
        }),
        Some(X11ControlChannels::Routed {
            receiver: control_receiver,
            acknowledgements: control_ack_sender,
        }),
        None,
        &XServerFrontendSetupAuthorization::default(),
        None,
        None,
        move |trace| {
            try_emit_x_authority_trace(&transaction_sender, &trace)
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            Ok(())
        },
    )
}

/// Runs one routed concurrent X11 client until it disconnects.
///
/// The caller owns the broker's input/control senders and must stop producing
/// routes before joining this helper. This is the migration bridge from the
/// single-client live-session transport to the general bounded concurrent
/// frontend service: the connection uses the same private worker queues as a
/// multi-client frontend, while this helper intentionally accepts only one
/// client.
#[cfg(unix)]
pub fn run_x11_core_socket_server_once_routed(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    mut broker: XServerFrontendRouteBroker,
) -> Result<(), X11SetupSocketError> {
    let config = XServerFrontendConfig::new(path.as_ref().to_path_buf(), namespace)?;
    let mut frontend = XServerFrontend::bind(config)?;
    frontend
        .state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .set_input_authority(broker.registry.input_authority.clone());
    let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
        try_emit_x_authority_trace(&transaction_sender, &trace)
            .map(|_| ())
            .map_err(|error| X11SetupSocketError::new(error.to_string()))
    });
    frontend.serve_next_concurrently_routed_traced(&broker, observer)?;
    while frontend.active_client_worker_count() != 0 {
        let routed = broker
            .route_pending()
            .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
        frontend.poll_client_workers()?;
        if routed == 0 && frontend.active_client_worker_count() != 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}

/// Runs a bounded routed X11 frontend until supervision stops accepting.
///
/// While accepting, the service starts every ready local connection up to the
/// configured worker limit, routes all pending Engine input/control into the
/// owning worker's private queues, and reaps completed workers. A
/// [`XServerFrontendServiceCommand::StopAccepting`] command closes admission
/// without closing client streams; the service then drains the workers that
/// already exist. The caller remains responsible for its session process
/// policy and should stop producing Engine routes before sending that command.
#[cfg(unix)]
pub fn run_x_server_frontend_routed_until_stopped(
    config: XServerFrontendConfig,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    mut broker: XServerFrontendRouteBroker,
    service_commands: Receiver<XServerFrontendServiceCommand>,
) -> Result<(), X11SetupSocketError> {
    let mut frontend = XServerFrontend::bind(config)?;
    frontend
        .state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .set_input_authority(broker.registry.input_authority.clone());
    let observer: Arc<X11CoreTraceObserver> = Arc::new(move |trace| {
        try_emit_x_authority_trace(&transaction_sender, &trace)
            .map(|_| ())
            .map_err(|error| X11SetupSocketError::new(error.to_string()))
    });
    let mut accepting = true;
    loop {
        let mut progressed = false;
        match service_commands.try_recv() {
            Ok(XServerFrontendServiceCommand::StopAccepting) | Err(TryRecvError::Disconnected) => {
                accepting = false
            }
            Ok(XServerFrontendServiceCommand::RevokeAdmission { admission }) => {
                progressed |= frontend.revoke_admission(admission)?;
            }
            Ok(XServerFrontendServiceCommand::UpdateOutputTopology {
                snapshot,
                acknowledgement,
            }) => {
                let mut outcome = frontend.update_output_topology(snapshot.clone())?;
                if matches!(outcome, XAuthorityOutputUpdateOutcome::Applied { .. }) {
                    let notifications = broker
                        .registry
                        .broadcast_randr_update(&snapshot)
                        .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
                    if let XAuthorityOutputUpdateOutcome::Applied {
                        notifications: delivered,
                        ..
                    } = &mut outcome
                    {
                        *delivered = notifications;
                    }
                }
                acknowledgement.try_send(outcome).map_err(|error| {
                    X11SetupSocketError::new(format!(
                        "failed to return Engine output topology acknowledgement: {error}"
                    ))
                })?;
                progressed = true;
            }
            Err(TryRecvError::Empty) => {}
        }

        if accepting {
            while frontend.active_client_worker_count()
                < frontend.config().max_concurrent_clients().get()
            {
                if !frontend.try_serve_next_concurrently_routed_traced(&broker, observer.clone())? {
                    break;
                }
                progressed = true;
            }
            let routed = broker
                .route_pending()
                .map_err(|error| X11SetupSocketError::new(error.to_string()))?;
            progressed |= routed != 0;
        }
        let workers_before_reap = frontend.active_client_worker_count();
        frontend.poll_client_workers()?;
        progressed |= workers_before_reap != frontend.active_client_worker_count();

        if !accepting && frontend.active_client_worker_count() == 0 {
            return Ok(());
        }
        if !progressed {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Convenience form of [`run_x_server_frontend_routed_until_stopped`] for an
/// unauthenticated local socket using the default frontend configuration.
#[cfg(unix)]
pub fn run_x11_core_socket_server_routed_until_stopped(
    path: impl AsRef<Path>,
    namespace: NamespaceId,
    transaction_sender: SyncSender<XAuthorityObservedTransactionBatch>,
    broker: XServerFrontendRouteBroker,
    service_commands: Receiver<XServerFrontendServiceCommand>,
) -> Result<(), X11SetupSocketError> {
    run_x_server_frontend_routed_until_stopped(
        XServerFrontendConfig::new(path.as_ref().to_path_buf(), namespace)?,
        transaction_sender,
        broker,
        service_commands,
    )
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
        None,
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
    state: &X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_once_traced(listener, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_once_traced(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_once_with_setup_authorization(
        listener,
        namespace,
        state,
        &authorization,
        None,
        None,
        observer,
    )
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_listener_traced(listener, namespace, state, |_| Ok(()))
}

#[cfg(unix)]
pub fn serve_x11_core_socket_listener_traced(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_listener_with_setup_authorization(
        listener,
        namespace,
        state,
        &authorization,
        None,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_with_setup_authorization(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    mut observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    loop {
        serve_x11_core_socket_listener_once_with_setup_authorization(
            listener,
            namespace,
            state,
            authorization,
            admission_policy.clone(),
            None,
            &mut observer,
        )?;
    }
}

#[cfg(unix)]
fn serve_x11_core_socket_listener_once_with_setup_authorization(
    listener: &UnixListener,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
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
        admission_policy,
        observer,
    )
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    serve_x11_setup_socket_client_with_root_size(
        stream,
        Size {
            width: i32::from(crate::X_SETUP_ROOT_WIDTH),
            height: i32::from(crate::X_SETUP_ROOT_HEIGHT),
        },
    )
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client_with_root_size(
    stream: &mut UnixStream,
    root_size: Size,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_setup_socket_client_with_setup_authorization(stream, &authorization, |_| {
        let mut success = XSetupSuccess::client_compatible();
        success.root_size = root_size;
        Ok(Some(success))
    })?
    .map(|(request, _)| request)
    .ok_or_else(|| {
        X11SetupSocketError::new("default X11 setup authorization unexpectedly rejected")
    })
}

#[cfg(unix)]
fn serve_x11_setup_socket_client_with_setup_authorization(
    stream: &mut UnixStream,
    authorization: &XServerFrontendSetupAuthorization,
    setup_success: impl FnOnce(&XSetupRequest) -> Result<Option<XSetupSuccess>, X11SetupSocketError>,
) -> Result<Option<(XSetupRequest, XSetupSuccess)>, X11SetupSocketError> {
    let request = read_x11_setup_request(stream)?;
    if !authorization.permits(&request) {
        write_x11_setup_failure(
            stream,
            request.byte_order,
            b"Sophia X11 authorization failed",
        )?;
        return Ok(None);
    }
    let Some(setup_success) = setup_success(&request)? else {
        write_x11_setup_failure(stream, request.byte_order, b"Sophia X11 admission failed")?;
        return Ok(None);
    };
    let response =
        encode_x11_setup_success(request.byte_order, &setup_success).map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup success: {error}"))
        })?;
    stream
        .write_all(&response)
        .map_err(|error| X11SetupSocketError::new(format!("failed to write X11 setup: {error}")))?;
    stream
        .flush()
        .map_err(|error| X11SetupSocketError::new(format!("failed to flush X11 setup: {error}")))?;
    Ok(Some((request, setup_success)))
}

#[cfg(unix)]
fn write_x11_setup_failure(
    stream: &mut UnixStream,
    byte_order: XByteOrder,
    reason: &[u8],
) -> Result<(), X11SetupSocketError> {
    let response =
        encode_x11_setup_failure(byte_order, &XSetupFailure::new(reason)).map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup failure: {error}"))
        })?;
    stream.write_all(&response).map_err(|error| {
        X11SetupSocketError::new(format!("failed to write X11 setup failure: {error}"))
    })?;
    stream.flush().map_err(|error| {
        X11SetupSocketError::new(format!("failed to flush X11 setup failure: {error}"))
    })
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
    state: &X11CoreSocketServerState,
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
    state: &X11CoreSocketServerState,
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
    state: &X11CoreSocketServerState,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
        stream,
        namespace,
        state,
        &authorization,
        None,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_setup_authorization(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer_and_setup_authorization_and_routing(
        stream,
        namespace,
        state,
        authorization,
        admission_policy,
        None,
        None,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_setup_authorization_and_routing(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    client_routing: Option<XServerFrontendRouteRegistry>,
    worker_admission: Option<(u64, Sender<X11CoreClientWorkerAdmission>)>,
    observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    serve_x11_core_socket_client_with_trace_observer_and_input(
        stream,
        namespace,
        state,
        None,
        None,
        client_routing,
        authorization,
        admission_policy,
        worker_admission,
        observer,
    )
}

#[cfg(unix)]
fn serve_x11_core_socket_client_with_trace_observer_and_input(
    stream: &mut UnixStream,
    namespace: NamespaceId,
    state: &X11CoreSocketServerState,
    input_receiver: Option<X11InputEventReceiver>,
    control_channels: Option<X11ControlChannels>,
    client_routing: Option<XServerFrontendRouteRegistry>,
    authorization: &XServerFrontendSetupAuthorization,
    admission_policy: Option<Arc<dyn XServerFrontendAdmissionPolicy>>,
    worker_admission: Option<(u64, Sender<X11CoreClientWorkerAdmission>)>,
    mut observer: impl FnMut(X11CoreDispatchTrace<'_>) -> Result<(), X11SetupSocketError>,
) -> Result<(), X11SetupSocketError> {
    let peer_credentials = if admission_policy.is_some() {
        x11_peer_credentials(stream)?
    } else {
        None
    };
    let mut setup_lease = None;
    let mut admission_lease = None;
    let mut admission_failure = None;
    let Some((setup, _setup_success)) = serve_x11_setup_socket_client_with_setup_authorization(
        stream,
        authorization,
        |setup_request| {
            if let Some(policy) = admission_policy.as_ref() {
                let request = XServerFrontendAdmissionRequest {
                    setup_authentication: authorization.authentication_method(),
                    peer_credentials,
                };
                match policy.admit(request) {
                    Ok(context) if context.is_valid() => {
                        admission_lease =
                            Some(XServerFrontendAdmissionLease::new(policy.clone(), context));
                    }
                    Ok(_) => {
                        admission_failure = Some(XServerFrontendAdmissionError::Unavailable);
                        return Ok(None);
                    }
                    Err(error) => {
                        admission_failure = Some(error);
                        return Ok(None);
                    }
                }
            }
            debug_assert!(authorization.permits(setup_request));
            let (lease, setup_success) = state.next_client_setup_success()?;
            setup_lease = Some(lease);
            Ok(Some(setup_success))
        },
    )?
    else {
        if admission_failure == Some(XServerFrontendAdmissionError::Unavailable) {
            return Err(X11SetupSocketError::new(
                "Sophia X Server Frontend admission policy unavailable",
            ));
        }
        return Ok(());
    };
    let namespace = admission_lease
        .as_ref()
        .map(|lease| lease.context().namespace.id)
        .unwrap_or(namespace);
    let client_lease = setup_lease.ok_or_else(|| {
        X11SetupSocketError::new("Sophia X Server Frontend did not retain a setup client lease")
    })?;
    let client = client_lease.client;
    if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
        eprintln!(
            "sophia_x11_client_route schema=1 stage=accepted client={}",
            client.raw()
        );
    }
    let resource_id_range = client_lease.resource_id_range;
    let mut sequence = 0u16;
    let event_sequence = Arc::new(AtomicU16::new(0));
    let focused_surface_window = Arc::new(AtomicU64::new(u64::from(X_SETUP_DEFAULT_ROOT)));
    let core_event_selections = Arc::new(Mutex::new(XCoreEventSelectionState::default()));
    let xkb_state_details = Arc::new(AtomicU16::new(0));
    let xkb_modifiers = Arc::new(AtomicU16::new(0));
    let surface_windows = Arc::new(Mutex::new(BTreeMap::new()));
    let output_stream = Arc::new(Mutex::new(stream.try_clone().map_err(|error| {
        X11SetupSocketError::new(format!("failed to clone X11 output socket: {error}"))
    })?));
    let protocol_routing = client_routing.clone();
    let (route_registration, input_receiver, control_channels, protocol_receiver) =
        if let Some(routing) = client_routing {
            let (registration, channels) = match routing.register_client(client) {
                Ok(registration) => registration,
                Err(error) => {
                    let _ = state.release_client(client);
                    return Err(X11SetupSocketError::new(format!(
                        "failed to register X11 client route: {error}"
                    )));
                }
            };
            (
                Some(registration),
                Some(X11InputEventReceiver::Routed {
                    receiver: channels.input,
                    deliveries: routing.input_delivery_sender.clone(),
                }),
                Some(X11ControlChannels::ClientBound {
                    receiver: channels.control,
                    acknowledgements: routing.acknowledgement_sender.clone(),
                }),
                Some(channels.protocol),
            )
        } else {
            (None, input_receiver, control_channels, None)
        };
    let input_writer = input_receiver
        .map(|receiver| {
            spawn_x11_input_event_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                focused_surface_window.clone(),
                core_event_selections.clone(),
                xkb_state_details.clone(),
                xkb_modifiers.clone(),
                surface_windows.clone(),
                client,
                receiver,
            )
        })
        .transpose()?;
    let control_writer = control_channels
        .map(|channels| {
            spawn_x11_control_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                focused_surface_window.clone(),
                surface_windows.clone(),
                core_event_selections.clone(),
                state.atoms.clone(),
                state.properties.clone(),
                state.runtime.clone(),
                resource_id_range,
                namespace,
                client,
                channels,
            )
        })
        .transpose()?;
    let protocol_writer = protocol_receiver
        .map(|receiver| {
            spawn_x11_protocol_event_writer(
                output_stream.clone(),
                setup.byte_order,
                event_sequence.clone(),
                receiver,
            )
        })
        .transpose()?;
    state.register_client(client_lease)?;
    if let Some((worker_id, sender)) = worker_admission
        && let Some(lease) = admission_lease.as_ref()
    {
        let _ = sender.send(X11CoreClientWorkerAdmission {
            worker_id,
            admission: lease.context().client_id,
        });
    }

    let result = (|| {
        // SCM_RIGHTS on a Unix stream is an in-band barrier, but recvmsg can
        // return the descriptors alongside bytes that precede the request
        // which consumes them. Retain those descriptors until the decoded X11
        // request declares its FD arity instead of binding them to the first
        // header returned by recvmsg.
        let mut pending_request_fds = Vec::new();
        while let Some(received) = read_x11_core_request(stream, setup.byte_order)? {
            let major_opcode = received.major_opcode;
            let request = received.bytes;
            let request_minor_code = if major_opcode >= 128 {
                u16::from(request[1])
            } else {
                0
            };
            let ancillary_fds = received.fds;
            let mut received_fds = Vec::new();
            loop {
                let server_owner = state
                    .runtime
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
                    .input_authority_mut()
                    .server_owner(namespace);
                if server_owner.is_none_or(|owner| owner == client.raw()) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            sequence = sequence.wrapping_add(1);
            let transaction = state.allocate_transaction()?;
            let dispatch_context = XDispatchContext {
                byte_order: setup.byte_order,
                namespace,
                sequence,
                major_opcode,
                client_id: client.raw(),
            };
            let mut parse_error = None;
            let mut request_detail = None;
            let (
                mut output,
                cpu_buffer_update,
                dri3_pixmap_import,
                dri3_fence_import,
                present_submission,
                released_dma_bufs,
                released_fences,
                mut server_reply_fds,
            ) = match decode_x11_core_request(
                XWireClientContext {
                    byte_order: setup.byte_order,
                    namespace,
                    transaction,
                    resource_id_range: Some(resource_id_range),
                },
                &request,
            ) {
                Ok(request) => {
                    let required_fd_count = request.required_fd_count();
                    pending_request_fds.extend(ancillary_fds);
                    const MAX_PENDING_REQUEST_FDS: usize = sophia_protocol::DMA_BUF_MAX_PLANES * 16;
                    if pending_request_fds.len() > MAX_PENDING_REQUEST_FDS {
                        return Err(X11SetupSocketError::new(
                            "X11 request stream carried too many pending file descriptors",
                        ));
                    }
                    if required_fd_count != 0 {
                        let take = required_fd_count.min(pending_request_fds.len());
                        received_fds.extend(pending_request_fds.drain(..take));
                    }
                    if required_fd_count != received_fds.len() {
                        return Err(X11SetupSocketError::new(format!(
                            "X11 request opcode {major_opcode} required {} file descriptors but received {}",
                            required_fd_count,
                            received_fds.len()
                        )));
                    }
                    let event_selection = x11_core_event_selection_update(&request);
                    let dri3_open = matches!(&request, crate::XWireRequest::Dri3Open { .. });
                    let dri3_query = matches!(
                        &request,
                        crate::XWireRequest::QueryExtension { name }
                            if name == crate::X_DRI3_EXTENSION_NAME
                    );
                    let dri3_pixmap = match &request {
                        crate::XWireRequest::Dri3PixmapFromBuffer { pixmap, .. }
                        | crate::XWireRequest::Dri3PixmapFromBuffers { pixmap, .. } => {
                            Some(*pixmap)
                        }
                        _ => None,
                    };
                    let dri3_fence_request = match &request {
                        crate::XWireRequest::Dri3FenceFromFd {
                            fence,
                            initially_triggered,
                            ..
                        } => Some((*fence, *initially_triggered)),
                        _ => None,
                    };
                    let freed_pixmap = match &request {
                        crate::XWireRequest::FreePixmap { pixmap } => Some(*pixmap),
                        _ => None,
                    };
                    let destroyed_fence = match &request {
                        crate::XWireRequest::SyncDestroyFence { fence } => Some(*fence),
                        _ => None,
                    };
                    let hierarchy_create = match &request {
                        crate::XWireRequest::CreateWindow { packet, parent, .. } => {
                            match &packet.kind {
                                crate::XAuthorityRequestKind::CreateWindow { window, .. } => {
                                    Some((*window, *parent))
                                }
                                _ => None,
                            }
                        }
                        crate::XWireRequest::ReparentWindow { window, parent, .. } => {
                            Some((*window, *parent))
                        }
                        _ => None,
                    };
                    let hierarchy_restack = match &request {
                        crate::XWireRequest::ConfigureWindow {
                            window,
                            sibling,
                            stack_mode,
                            ..
                        } => Some((*window, *sibling, *stack_mode)),
                        _ => None,
                    };
                    let randr_selection = match &request {
                        crate::XWireRequest::RandrSelectInput { window, enable } => {
                            Some((*window, *enable))
                        }
                        _ => None,
                    };
                    let present_selection = match &request {
                        crate::XWireRequest::PresentSelectInput {
                            event_id,
                            window,
                            event_mask,
                        } => Some((*event_id, *window, *event_mask)),
                        _ => None,
                    };
                    let pending_present = match &request {
                        crate::XWireRequest::PresentPixmap {
                            window,
                            pixmap,
                            serial,
                            idle_fence,
                            ..
                        } => Some((*window, *pixmap, *serial, *idle_fence)),
                        _ => None,
                    };
                    let present_request = match &request {
                        crate::XWireRequest::PresentPixmap {
                            wait_fence,
                            idle_fence,
                            ..
                        } => Some((*wait_fence, *idle_fence)),
                        _ => None,
                    };
                    let xkb_selection = match &request {
                        crate::XWireRequest::XkbSelectEvents {
                            affect_which,
                            clear,
                            select_all,
                            state_details,
                        } => Some((*affect_which, *clear, *select_all, *state_details)),
                        _ => None,
                    };
                    let xkb_get_state = matches!(request, crate::XWireRequest::XkbGetState);
                    let mapped_window = match &request {
                        crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
                            kind: crate::XAuthorityRequestKind::MapWindow { window, .. },
                            ..
                        }) => Some(*window),
                        _ => None,
                    };
                    let destroyed_window = match &request {
                        crate::XWireRequest::DestroyWindow { window } => Some(*window),
                        _ => None,
                    };
                    let unmapped_window = match &request {
                        crate::XWireRequest::UnmapWindow { window } => Some(*window),
                        _ => None,
                    };
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
                        if let Some(routing) = protocol_routing.as_ref() {
                            routing
                                .register_surface(client, namespace, *surface, *window)
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to register X11 surface route: {error}"
                                    ))
                                })?;
                        }
                    }
                    request_detail = x11_core_request_trace_detail(&request);
                    let queued_present = if let Some((window, pixmap, serial, idle_fence)) =
                        pending_present
                        && let Some(routing) = protocol_routing.as_ref()
                    {
                        routing
                            .queue_present(transaction, client, window, pixmap, serial, idle_fence)
                            .map_err(|error| {
                                X11SetupSocketError::client_failure(format!(
                                    "failed to queue Present feedback: {error}"
                                ))
                            })?;
                        true
                    } else {
                        false
                    };
                    let mut runtime = state.runtime.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 authority runtime lock poisoned")
                    })?;
                    let mut atoms = state
                        .atoms
                        .lock()
                        .map_err(|_| X11SetupSocketError::new("X11 atom table lock poisoned"))?;
                    let mut properties = state.properties.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 property table lock poisoned")
                    })?;
                    let released_dma_buf = freed_pixmap.and_then(|pixmap| {
                        runtime
                            .dri3_pixmap_descriptor(namespace, pixmap)
                            .ok()
                            .map(|descriptor| descriptor.handle)
                    });
                    let released_fence = destroyed_fence
                        .and_then(|fence| runtime.dri3_fence_handle(namespace, fence).ok());
                    let mut output = dispatch_x11_wire_request(
                        dispatch_context,
                        request,
                        &mut runtime,
                        &mut atoms,
                        &mut properties,
                    );
                    if dri3_query && !state.has_render_device_provider() {
                        for client_output in &mut output.outputs {
                            if let crate::XClientOutput::Reply(
                                crate::XClientReply::QueryExtension {
                                    present,
                                    major_opcode,
                                    first_event,
                                    first_error,
                                    ..
                                },
                            ) = client_output
                            {
                                *present = false;
                                *major_opcode = 0;
                                *first_event = 0;
                                *first_error = 0;
                            }
                        }
                    }
                    if let Some(crate::XClientOutput::Reply(crate::XClientReply::GetGeometry {
                        geometry,
                        ..
                    })) = output.outputs.iter().find(|output| {
                        matches!(
                            output,
                            crate::XClientOutput::Reply(crate::XClientReply::GetGeometry { .. })
                        )
                    }) {
                        request_detail = Some(format!(
                            "{}:reply={}x{}+{}+{}",
                            request_detail.as_deref().unwrap_or("GetGeometry"),
                            geometry.width,
                            geometry.height,
                            geometry.x,
                            geometry.y
                        ));
                    }
                    if xkb_get_state {
                        for client_output in &mut output.outputs {
                            if let crate::XClientOutput::Reply(crate::XClientReply::XkbGetState {
                                modifiers,
                                ..
                            }) = client_output
                            {
                                *modifiers = xkb_modifiers.load(Ordering::Acquire) as u8;
                            }
                        }
                    }
                    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some()
                        && let Some(detail) = request_detail.as_deref()
                        && detail.starts_with("GetKeyboardMapping:")
                    {
                        eprintln!("sophia_x11_keyboard_map schema=1 {detail}");
                    }
                    let dispatch_succeeded = !output
                        .outputs
                        .iter()
                        .any(|output| matches!(output, crate::XClientOutput::Error(_)));
                    if dispatch_succeeded {
                        let mut selections = core_event_selections.lock().map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?;
                        if let Some((window, event_mask, do_not_propagate_mask)) = event_selection {
                            selections.update(window, event_mask, do_not_propagate_mask);
                        }
                        if let Some((window, parent)) = hierarchy_create {
                            selections.register(window, parent);
                        }
                        if let Some((window, sibling, mode)) = hierarchy_restack {
                            selections.restack(window, sibling, mode);
                        }
                        if let Some(window) = mapped_window {
                            selections.observe_mapped(window);
                        }
                        if let Some(window) = unmapped_window {
                            selections.observe_unmapped(window);
                        }
                        if let Some(window) = destroyed_window {
                            selections.remove(window);
                        }
                        if let Some((window, mask)) = randr_selection
                            && let Some(routing) = protocol_routing.as_ref()
                        {
                            routing
                                .select_randr_input(client, window, mask)
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to update RandR subscription: {error}"
                                    ))
                                })?;
                        }
                        if let Some((event_id, window, mask)) = present_selection
                            && let Some(routing) = protocol_routing.as_ref()
                        {
                            routing
                                .select_present_input(client, event_id, window, mask)
                                .map_err(|error| {
                                    X11SetupSocketError::new(format!(
                                        "failed to update Present subscription: {error}"
                                    ))
                                })?;
                        }
                        if let Some((affect_which, clear, select_all, state)) = xkb_selection {
                            let mut details = xkb_state_details.load(Ordering::Acquire);
                            if clear & 4 != 0 {
                                details = 0;
                            }
                            if select_all & 4 != 0 {
                                details = u16::MAX;
                            }
                            if affect_which & 4 != 0
                                && let Some((affect, selected)) = state
                            {
                                details = (details & !affect) | (selected & affect);
                            }
                            xkb_state_details.store(details, Ordering::Release);
                        }
                    }
                    if queued_present
                        && !dispatch_succeeded
                        && let Some(routing) = protocol_routing.as_ref()
                    {
                        routing.cancel_present(transaction).map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to cancel rejected Present feedback: {error}"
                            ))
                        })?;
                    }
                    // The CPU update belongs to this dispatch. Keep it under
                    // the runtime lock so a simultaneous client cannot take
                    // an update generated by this request.
                    let cpu_buffer_update = runtime.take_cpu_buffer_update();
                    let dri3_pixmap_import = dri3_pixmap.and_then(|pixmap| {
                        runtime
                            .dri3_pixmap_descriptor(namespace, pixmap)
                            .ok()
                            .map(|descriptor| XAuthorityDri3PixmapImport { pixmap, descriptor })
                    });
                    let dri3_fence_import = dispatch_succeeded
                        .then_some(dri3_fence_request)
                        .flatten()
                        .and_then(|(fence, initially_triggered)| {
                            runtime
                                .dri3_fence_handle(namespace, fence)
                                .ok()
                                .map(|handle| XAuthorityDri3FenceImport {
                                    fence,
                                    handle,
                                    initially_triggered,
                                })
                        });
                    let present_submission = dispatch_succeeded
                        .then_some(present_request)
                        .flatten()
                        .and_then(|(wait_fence, idle_fence)| {
                            let response = output.response.as_ref()?;
                            let transaction = response.transactions.first()?;
                            let sophia_protocol::BufferSource::DmaBuf { handle } =
                                transaction.target_buffer
                            else {
                                return None;
                            };
                            Some(XAuthorityPresentSubmission {
                                transaction: response.transaction,
                                surface: transaction.surface,
                                buffer: sophia_protocol::BufferHandle::from_raw(handle),
                                acquire_fence: wait_fence.and_then(|fence| {
                                    runtime.dri3_fence_handle(namespace, fence).ok()
                                }),
                                idle_fence: idle_fence.and_then(|fence| {
                                    runtime.dri3_fence_handle(namespace, fence).ok()
                                }),
                            })
                        });
                    let mut server_reply_fds = Vec::new();
                    if dispatch_succeeded && dri3_open {
                        match state.open_render_device_fd() {
                            Ok(fd) => server_reply_fds.push(fd),
                            Err(_) => {
                                output.outputs =
                                    vec![crate::XClientOutput::Error(crate::XClientError {
                                        code: crate::XErrorCode::BadImplementation,
                                        sequence,
                                        resource_id: 0,
                                        minor_code: u16::from(crate::X_DRI3_OPEN_MINOR_OPCODE),
                                        major_code: crate::X_DRI3_MAJOR_OPCODE,
                                    })];
                            }
                        }
                    }
                    (
                        output,
                        cpu_buffer_update,
                        dri3_pixmap_import,
                        dri3_fence_import,
                        present_submission,
                        released_dma_buf.into_iter().collect::<Vec<_>>(),
                        released_fence.into_iter().collect::<Vec<_>>(),
                        server_reply_fds,
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
                    (
                        dispatch_x11_parse_error(dispatch_context, request_minor_code, error),
                        None,
                        None,
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                }
            };
            if let Some(routing) = protocol_routing.as_ref()
                && let Some((index, requestor, property)) = output
                    .outputs
                    .iter()
                    .enumerate()
                    .find_map(|(index, output)| match output {
                        crate::XClientOutput::Event(XClientEvent::SelectionNotify {
                            requestor,
                            property,
                            ..
                        }) => Some((index, *requestor, *property)),
                        _ => None,
                    })
            {
                let mut runtime = state
                    .runtime
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?;
                if runtime.is_clipboard_proxy(namespace, requestor) {
                    let mut properties = state.properties.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 property table lock poisoned")
                    })?;
                    let payload = runtime
                        .capture_clipboard_source_payload(requestor, property, &mut properties)
                        .map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to capture clipboard source payload: {error:?}"
                            ))
                        })?;
                    routing.source_payload_sender.try_send(payload).map_err(
                        |error| match error {
                            TrySendError::Full(_) => {
                                X11SetupSocketError::new("clipboard source payload queue is full")
                            }
                            TrySendError::Disconnected(_) => X11SetupSocketError::new(
                                "clipboard source payload queue is disconnected",
                            ),
                        },
                    )?;
                    output.outputs.remove(index);
                }
            }
            if let Some(routing) = protocol_routing.as_ref()
                && let Some((index, destination, event)) = output
                    .outputs
                    .iter()
                    .enumerate()
                    .find_map(|(index, output)| match output {
                        crate::XClientOutput::Event(
                            event @ XClientEvent::SelectionNotify { requestor, .. },
                        ) => Some((index, *requestor, *event)),
                        crate::XClientOutput::Event(
                            event @ XClientEvent::SelectionRequest { owner, .. },
                        ) => Some((index, *owner, *event)),
                        crate::XClientOutput::Event(
                            event @ XClientEvent::SelectionClear { owner, .. },
                        ) => Some((index, *owner, *event)),
                        _ => None,
                    })
                && let Some(target) = state.client_for_resource(destination)?
                && target != client
            {
                routing.route_protocol(target, event).map_err(|error| {
                    X11SetupSocketError::new(format!("failed to route X11 protocol event: {error}"))
                })?;
                output.outputs.remove(index);
            }
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                let request_head = request
                    .iter()
                    .take(24)
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let replies = output
                    .outputs
                    .iter()
                    .filter(|item| matches!(item, crate::XClientOutput::Reply(_)))
                    .count();
                let errors = output
                    .outputs
                    .iter()
                    .filter(|item| matches!(item, crate::XClientOutput::Error(_)))
                    .count();
                let events = output
                    .outputs
                    .iter()
                    .filter(|item| matches!(item, crate::XClientOutput::Event(_)))
                    .count();
                let first_error = output.outputs.iter().find_map(|item| match item {
                    crate::XClientOutput::Error(error) => {
                        Some(format!("{:?}:minor={}", error.code, error.minor_code))
                    }
                    _ => None,
                });
                eprintln!(
                    "sophia_x11_dispatch schema=1 sequence={} major={} minor={} request_len={} request_head={} parse={} detail={} replies={} errors={} events={} first_error={} response={}",
                    sequence,
                    major_opcode,
                    request_minor_code,
                    request.len(),
                    request_head,
                    x11_trace_token(parse_error.as_deref()),
                    x11_trace_token(request_detail.as_deref()),
                    replies,
                    errors,
                    events,
                    first_error.as_deref().unwrap_or("none"),
                    output.response.is_some(),
                );
            }
            observer(X11CoreDispatchTrace {
                client,
                resource_id_range,
                sequence,
                major_opcode,
                request_detail,
                parse_error,
                result: &output,
                cpu_buffer_update: cpu_buffer_update.as_ref(),
                received_fd_count: received_fds.len(),
                received_fds: &received_fds,
                dri3_pixmap_import,
                dri3_fence_import,
                present_submission,
                released_dma_bufs: &released_dma_bufs,
                released_fences: &released_fences,
                server_reply_fd_count: server_reply_fds.len(),
            })?;
            let encoded_outputs = output.encoded_outputs(setup.byte_order);
            {
                let mut output_stream = output_stream
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
                if !encoded_outputs.is_empty() || !server_reply_fds.is_empty() {
                    for (index, bytes) in encoded_outputs.into_iter().enumerate() {
                        let fds = if index == 0 {
                            core::mem::take(&mut server_reply_fds)
                        } else {
                            Vec::new()
                        };
                        let record = X11SocketOutputRecord::new(bytes, fds)?;
                        if let Err(error) =
                            write_x11_socket_output_record(&mut output_stream, record)
                        {
                            if is_x11_client_disconnect(&error) {
                                return Ok(());
                            }
                            return Err(X11SetupSocketError::new(format!(
                                "failed to write X11 output: {error}"
                            )));
                        }
                    }
                    debug_assert!(server_reply_fds.is_empty());
                    if let Err(error) = output_stream.flush() {
                        if matches!(
                            error.kind(),
                            ErrorKind::BrokenPipe
                                | ErrorKind::ConnectionReset
                                | ErrorKind::UnexpectedEof
                        ) {
                            return Ok(());
                        }
                        return Err(X11SetupSocketError::new(format!(
                            "failed to flush X11 output: {error}"
                        )));
                    }
                }
                // Publish the request sequence while holding the same lock
                // used by every asynchronous event writer. Otherwise a
                // writer can snapshot the old value, wait behind this reply,
                // and emit a backwards sequence after it.
                event_sequence.store(sequence, Ordering::Release);
            }
        }
        Ok(())
    })();

    let writer_result: Result<(), X11SetupSocketError> = (|| {
        if let Some(writer) = input_writer {
            writer.stop.store(true, Ordering::Release);
            writer.thread.join().map_err(|_| {
                X11SetupSocketError::new("X11 input event writer thread panicked")
            })??;
        }
        if let Some(writer) = control_writer {
            writer.stop.store(true, Ordering::Release);
            writer
                .thread
                .join()
                .map_err(|_| X11SetupSocketError::new("X11 control writer thread panicked"))??;
        }
        if let Some(writer) = protocol_writer {
            writer.stop.store(true, Ordering::Release);
            writer.thread.join().map_err(|_| {
                X11SetupSocketError::new("X11 protocol event writer thread panicked")
            })??;
        }
        Ok(())
    })();
    state
        .runtime
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
        .input_authority_mut()
        .cleanup_owner(client.raw());
    let client_lease = state.release_client(client)?;
    debug_assert_eq!(client_lease.resource_id_range, resource_id_range);
    let release = release_x11_client_lease(state, namespace, client_lease)?;
    drop(route_registration);
    let cleanup_observer_result = if release.removed_surfaces.is_empty()
        && release.released_dma_bufs.is_empty()
        && release.released_fences.is_empty()
    {
        Ok(())
    } else {
        sequence = sequence.wrapping_add(1);
        let transaction = state.allocate_transaction()?;
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        response.removed_surfaces = release.removed_surfaces;
        let cleanup = XDispatchResult {
            response: Some(response),
            outputs: Vec::new(),
            metadata_candidates: Vec::new(),
        };
        observer(X11CoreDispatchTrace {
            client,
            resource_id_range,
            sequence,
            major_opcode: 0,
            request_detail: Some("DisconnectCleanup".to_owned()),
            parse_error: None,
            result: &cleanup,
            cpu_buffer_update: None,
            received_fd_count: 0,
            received_fds: &[],
            dri3_pixmap_import: None,
            dri3_fence_import: None,
            present_submission: None,
            released_dma_bufs: &release.released_dma_bufs,
            released_fences: &release.released_fences,
            server_reply_fd_count: 0,
        })
    };
    let admission_result = admission_lease.as_mut().map_or(Ok(()), |lease| {
        lease.revoke().map_err(|error| {
            X11SetupSocketError::new(format!("failed to revoke X11 client admission: {error}"))
        })
    });
    result?;
    writer_result?;
    cleanup_observer_result?;
    admission_result
}

#[cfg(unix)]
fn x11_core_event_selection_update(
    request: &crate::XWireRequest,
) -> Option<(XResourceId, Option<u32>, Option<u32>)> {
    match request {
        crate::XWireRequest::CreateWindow {
            packet:
                crate::XAuthorityRequestPacket {
                    kind: crate::XAuthorityRequestKind::CreateWindow { window, .. },
                    ..
                },
            event_mask,
            do_not_propagate_mask,
            ..
        }
        | crate::XWireRequest::ChangeWindowAttributes {
            window,
            event_mask,
            do_not_propagate_mask,
        } => Some((*window, *event_mask, *do_not_propagate_mask)),
        _ => None,
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Default)]
struct XCoreWindowEventSelection {
    mask: u32,
    do_not_propagate_mask: u32,
}

#[cfg(unix)]
#[derive(Debug)]
struct XCoreEventSelectionState {
    windows: BTreeMap<XResourceId, XCoreWindowEventSelection>,
    parents: BTreeMap<XResourceId, XResourceId>,
    stacking: Vec<XResourceId>,
    mapped: BTreeSet<XResourceId>,
    fallback_mapped_window: XResourceId,
}

#[cfg(unix)]
impl Default for XCoreEventSelectionState {
    fn default() -> Self {
        Self {
            windows: BTreeMap::new(),
            parents: BTreeMap::new(),
            stacking: Vec::new(),
            mapped: BTreeSet::new(),
            fallback_mapped_window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    }
}

#[cfg(unix)]
impl XCoreEventSelectionState {
    const KEY_MASKS: u32 = (1 << 0) | (1 << 1);

    fn update(
        &mut self,
        window: XResourceId,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    ) {
        if event_mask.is_none() && do_not_propagate_mask.is_none() {
            return;
        }
        let selection = self.windows.entry(window).or_default();
        if let Some(mask) = event_mask {
            selection.mask = mask;
        }
        if let Some(mask) = do_not_propagate_mask {
            selection.do_not_propagate_mask = mask;
        }
    }

    fn register(&mut self, window: XResourceId, parent: XResourceId) {
        self.parents.insert(window, parent);
        self.stacking.retain(|candidate| *candidate != window);
        self.stacking.push(window);
    }

    fn restack(&mut self, window: XResourceId, sibling: Option<XResourceId>, mode: Option<u8>) {
        self.stacking.retain(|candidate| *candidate != window);
        let sibling_index = sibling.and_then(|sibling| {
            self.stacking
                .iter()
                .position(|candidate| *candidate == sibling)
        });
        let index = match (mode, sibling_index) {
            (Some(1 | 3), Some(index)) => index,
            (Some(1 | 3), None) => 0,
            (Some(0 | 2 | 4), Some(index)) => index.saturating_add(1),
            _ => self.stacking.len(),
        };
        self.stacking.insert(index.min(self.stacking.len()), window);
    }

    fn observe_mapped(&mut self, window: XResourceId) {
        self.mapped.insert(window);
        self.fallback_mapped_window = window;
    }

    fn observe_unmapped(&mut self, window: XResourceId) {
        self.mapped.remove(&window);
        if self.fallback_mapped_window == window {
            self.fallback_mapped_window = self
                .stacking
                .iter()
                .rev()
                .copied()
                .find(|candidate| self.mapped.contains(candidate))
                .unwrap_or_else(|| XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1));
        }
    }

    fn remove(&mut self, window: XResourceId) {
        self.windows.remove(&window);
        self.parents.remove(&window);
        self.stacking.retain(|candidate| *candidate != window);
        self.mapped.remove(&window);
        if self.fallback_mapped_window == window {
            self.fallback_mapped_window = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        }
    }

    fn keyboard_target(&self, focused: XResourceId) -> XResourceId {
        self.selected_keyboard_target(focused)
            .unwrap_or_else(|| self.keyboard_fallback(focused))
    }

    fn selected_keyboard_target(&self, focused: XResourceId) -> Option<XResourceId> {
        let mut candidate = self.keyboard_fallback(focused);
        for _ in 0..64 {
            if self
                .windows
                .get(&candidate)
                .is_some_and(|selection| selection.mask & Self::KEY_MASKS != 0)
            {
                return Some(candidate);
            }
            candidate = self.parents.get(&candidate).copied()?;
        }
        None
    }

    fn keyboard_fallback(&self, focused: XResourceId) -> XResourceId {
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        if focused == root {
            self.stacking
                .iter()
                .rev()
                .copied()
                .find(|window| self.mapped.contains(window))
                .unwrap_or(self.fallback_mapped_window)
        } else {
            focused
        }
    }

    fn ancestors(&self, window: XResourceId) -> Vec<XResourceId> {
        let mut ancestors = Vec::new();
        let mut candidate = window;
        for _ in 0..64 {
            let Some(parent) = self.parents.get(&candidate).copied() else {
                break;
            };
            ancestors.push(parent);
            candidate = parent;
        }
        ancestors
    }
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
struct X11ProtocolEventWriter {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<Result<(), X11SetupSocketError>>,
}

#[cfg(unix)]
fn spawn_x11_protocol_event_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    receiver: Receiver<XClientEvent>,
) -> Result<X11ProtocolEventWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Acquire) {
            let mut event = match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let mut stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            set_x11_protocol_event_sequence(&mut event, sequence.load(Ordering::Acquire));
            let record = encode_x_client_event(byte_order, event);
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                use std::os::fd::AsRawFd;
                eprintln!(
                    "sophia_x11_socket_write schema=1 writer=protocol fd={} bytes={} head={:02x}{:02x}{:02x}{:02x}",
                    stream.as_raw_fd(),
                    record.len(),
                    record[0],
                    record[1],
                    record[2],
                    record[3],
                );
            }
            if let Err(error) = stream.write_all(&record) {
                if is_x11_client_disconnect(&error) {
                    return Ok(());
                }
                return Err(X11SetupSocketError::new(format!(
                    "failed to write X11 protocol event: {error}"
                )));
            }
            stream.flush().map_err(|error| {
                X11SetupSocketError::new(format!("failed to flush X11 protocol event: {error}"))
            })?;
        }
        Ok(())
    });
    Ok(X11ProtocolEventWriter { stop, thread })
}

#[cfg(unix)]
fn set_x11_protocol_event_sequence(event: &mut XClientEvent, value: u16) {
    match event {
        XClientEvent::SelectionClear { sequence, .. }
        | XClientEvent::SelectionRequest { sequence, .. }
        | XClientEvent::SelectionNotify { sequence, .. }
        | XClientEvent::RandrScreenChange { sequence, .. }
        | XClientEvent::RandrCrtcChange { sequence, .. }
        | XClientEvent::RandrOutputChange { sequence, .. }
        | XClientEvent::RandrResourceChange { sequence, .. }
        | XClientEvent::PresentCompleteNotify { sequence, .. }
        | XClientEvent::PresentIdleNotify { sequence, .. } => *sequence = value,
        _ => unreachable!("protocol routing received a non-routable event"),
    }
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn spawn_x11_control_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_surface_window: Arc<AtomicU64>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    core_event_selections: Arc<Mutex<XCoreEventSelectionState>>,
    atoms: Arc<Mutex<XAtomTable>>,
    properties: Arc<Mutex<XPropertyTable>>,
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    resource_id_range: crate::XWireClientResourceRange,
    namespace: NamespaceId,
    client: XServerFrontendClientId,
    channels: X11ControlChannels,
) -> Result<X11ControlWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    macro_rules! terminate_client {
        ($transaction:expr, $surface:expr) => {{
            let stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
            stream.shutdown(Shutdown::Both).map_err(|error| {
                X11SetupSocketError::new(format!(
                    "failed to terminate non-cooperating X11 client: {error}"
                ))
            })?;
            drop(stream);
            channels.send_ack(
                client,
                XAuthorityControlAck {
                    transaction: $transaction,
                    surface: $surface,
                    outcome: XAuthorityControlOutcome::Delivered,
                },
            )?;
            return Ok(());
        }};
    }
    let thread = std::thread::spawn(move || {
        while !writer_stop.load(Ordering::Acquire) {
            let command = match channels.recv_timeout(client) {
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
                channels.send_ack(
                    client,
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
                        channels.send_ack(
                            client,
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
                            channels.send_ack(
                                client,
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
                XAuthorityControlCommand::CloseSurface { .. } => {
                    let atoms = atoms
                        .lock()
                        .map_err(|_| X11SetupSocketError::new("X11 atom table lock poisoned"))?;
                    let Some(protocols) = atoms.atom(X_ATOM_NAME_WM_PROTOCOLS) else {
                        terminate_client!(transaction, surface);
                    };
                    let Some(delete) = atoms.atom(X_ATOM_NAME_WM_DELETE_WINDOW) else {
                        terminate_client!(transaction, surface);
                    };
                    drop(atoms);
                    let properties = properties.lock().map_err(|_| {
                        X11SetupSocketError::new("X11 property table lock poisoned")
                    })?;
                    let protocol_windows = properties.windows_with_property(namespace, protocols);
                    let advertises_delete = |candidate: &XResourceId| {
                        u32::try_from(candidate.local.raw())
                            .is_ok_and(|raw| resource_id_range.owns_new_resource(raw))
                            && properties
                                .get(namespace, *candidate, protocols)
                                .is_some_and(|record| {
                                    record.format == 32
                                        && record
                                            .bytes
                                            .chunks_exact(4)
                                            .any(|bytes| byte_order.u32(bytes) == delete)
                                })
                    };
                    let candidates: Vec<_> = protocol_windows
                        .iter()
                        .map(|candidate| (*candidate, advertises_delete(candidate)))
                        .collect();
                    let ancestors = core_event_selections
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 core event selection lock poisoned")
                        })?
                        .ancestors(window);
                    let decision = crate::select_x_close_target(window, &ancestors, &candidates);
                    if decision.protocol_window_count == 0 {
                        drop(properties);
                        terminate_client!(transaction, surface);
                    }
                    eprintln!(
                        "sophia_x11_close_target schema=1 surface_map_hit=true exact_delete={} fallback_used={} protocol_windows={}",
                        decision.exact_advertises_delete,
                        decision.fallback_used,
                        decision.protocol_window_count,
                    );
                    let window = decision.window;
                    let mut bytes = [0_u8; 32];
                    // ICCCM WM_DELETE_WINDOW is delivered via SendEvent, so
                    // the synthetic-event bit must be set on ClientMessage.
                    bytes[0] = 33 | 0x80;
                    bytes[1] = 32;
                    write_control_u32(byte_order, &mut bytes[4..8], window.local.raw() as u32);
                    write_control_u32(byte_order, &mut bytes[8..12], protocols);
                    write_control_u32(byte_order, &mut bytes[12..16], delete);
                    vec![encode_x_client_event(
                        byte_order,
                        XClientEvent::ClientMessage {
                            sequence: event_sequence,
                            bytes,
                        },
                    )]
                }
                XAuthorityControlCommand::FocusSurface { .. } => {
                    if runtime
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 authority runtime lock poisoned")
                        })?
                        .set_input_focus(namespace, window, 1)
                        .is_err()
                    {
                        channels.send_ack(
                            client,
                            XAuthorityControlAck {
                                transaction,
                                surface,
                                outcome: XAuthorityControlOutcome::AuthorityRejected,
                            },
                        )?;
                        continue;
                    }
                    let previous = XResourceId::new(
                        focused_surface_window.swap(window.local.raw(), Ordering::AcqRel),
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
            let event_sequence = sequence.load(Ordering::Acquire);
            for mut record in records {
                write_xi_u16(byte_order, &mut record[2..4], event_sequence);
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                    use std::os::fd::AsRawFd;
                    eprintln!(
                        "sophia_x11_socket_write schema=1 writer=control fd={} bytes={} head={:02x}{:02x}{:02x}{:02x}",
                        stream.as_raw_fd(),
                        record.len(),
                        record[0],
                        record[1],
                        record[2],
                        record[3],
                    );
                }
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
            channels.send_ack(
                client,
                XAuthorityControlAck {
                    transaction,
                    surface,
                    outcome: XAuthorityControlOutcome::Delivered,
                },
            )?;
        }
        Ok(())
    });
    Ok(X11ControlWriter { stop, thread })
}

#[cfg(unix)]
fn write_control_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    let bytes = match byte_order {
        XByteOrder::LittleEndian => value.to_le_bytes(),
        XByteOrder::BigEndian => value.to_be_bytes(),
    };
    out.copy_from_slice(&bytes);
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
    focused_surface_window: Arc<AtomicU64>,
    core_event_selections: Arc<Mutex<XCoreEventSelectionState>>,
    xkb_state_details: Arc<AtomicU16>,
    xkb_modifiers: Arc<AtomicU16>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    client: XServerFrontendClientId,
    receiver: X11InputEventReceiver,
) -> Result<X11InputEventWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut focus_sent_to = None;
        let mut pointer_sent_to = None;
        while !writer_stop.load(Ordering::Acquire) {
            let (event, target_window, xi_event_type, xi_transition_mask, delivery) =
                match receiver.recv_timeout(client) {
                    Ok(event) => event,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                };
            // A mapped GL client can expose its first frame before its event
            // loop installs KeyPress/KeyReleaseMask. Keep physical keys
            // boundedly pending across that startup race instead of writing
            // core events which the client has not selected and will ignore.
            let keyboard_wait_started = std::time::Instant::now();
            let keyboard_deadline = keyboard_wait_started + Duration::from_secs(5);
            let (focused_window, routed_keyboard_window, keyboard_selected) = loop {
                let selections = core_event_selections.lock().map_err(|_| {
                    X11SetupSocketError::new("X11 core event selection lock poisoned")
                })?;
                let focused = XResourceId::new(focused_surface_window.load(Ordering::Acquire), 1);
                let focused_selected = selections.selected_keyboard_target(focused);
                let routed_selected =
                    target_window.and_then(|window| selections.selected_keyboard_target(window));
                let focused_fallback = selections.keyboard_target(focused);
                let routed_fallback =
                    target_window.map(|window| selections.keyboard_target(window));
                drop(selections);
                if !matches!(event, XAuthorityInputEvent::Key(_))
                    || focused_selected.is_some()
                    || routed_selected.is_some()
                    || std::time::Instant::now() >= keyboard_deadline
                {
                    break (
                        focused_selected.unwrap_or(focused_fallback),
                        routed_selected.or(routed_fallback),
                        focused_selected.is_some() || routed_selected.is_some(),
                    );
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some()
                && let XAuthorityInputEvent::Key(key) = event
            {
                eprintln!(
                    "sophia_x11_key_delivery schema=2 stage=target_resolved client={} keycode={} pressed={} focus_window={:#x} routed_window={} keyboard_selected={} xi_event_type={} xi_transition_mask={:#x} wait_msec={}",
                    client.raw(),
                    key.keycode,
                    key.pressed,
                    focused_window.local.raw(),
                    routed_keyboard_window
                        .map(|window| format!("{:#x}", window.local.raw()))
                        .unwrap_or_else(|| "none".to_owned()),
                    keyboard_selected,
                    xi_event_type
                        .map(|event_type| event_type.to_string())
                        .unwrap_or_else(|| "none".to_owned()),
                    xi_transition_mask,
                    keyboard_wait_started.elapsed().as_millis(),
                );
            }
            if let XAuthorityInputEvent::Key(_) = event
                && routed_keyboard_window.is_some_and(|window| window != focused_window)
            {
                eprintln!(
                    "sophia_x11_key_delivery schema=1 target_matches_focus=false explicit_target=true",
                );
            }
            let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
            let delivered_window = match event {
                XAuthorityInputEvent::Key(_) => routed_keyboard_window.unwrap_or(focused_window),
                XAuthorityInputEvent::Pointer(pointer) => target_window.unwrap_or(
                    *surface_windows
                        .lock()
                        .map_err(|_| {
                            X11SetupSocketError::new("X11 surface/window map lock poisoned")
                        })?
                        .get(&pointer.surface)
                        .ok_or_else(|| {
                            X11SetupSocketError::new("X11 pointer target surface is unknown")
                        })?,
                ),
            };
            let delivered_focus = delivered_window;
            let mut record = encode_x_client_event(
                byte_order,
                match event {
                    XAuthorityInputEvent::Key(event) => XClientEvent::Key {
                        sequence: 0,
                        pressed: event.pressed,
                        keycode: event.keycode,
                        time: event.time_msec,
                        root,
                        event: delivered_window,
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
                        sequence: 0,
                        time: time_msec,
                        root,
                        event: target_window.unwrap_or(
                            *surface_windows
                                .lock()
                                .map_err(|_| {
                                    X11SetupSocketError::new("X11 surface/window map lock poisoned")
                                })?
                                .get(&surface)
                                .ok_or_else(|| {
                                    X11SetupSocketError::new(
                                        "X11 pointer target surface is unknown",
                                    )
                                })?,
                        ),
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
                        sequence: 0,
                        pressed,
                        button,
                        time: time_msec,
                        root,
                        event: target_window.unwrap_or(
                            *surface_windows
                                .lock()
                                .map_err(|_| {
                                    X11SetupSocketError::new("X11 surface/window map lock poisoned")
                                })?
                                .get(&surface)
                                .ok_or_else(|| {
                                    X11SetupSocketError::new(
                                        "X11 pointer target surface is unknown",
                                    )
                                })?,
                        ),
                        root_x,
                        root_y,
                        event_x,
                        event_y,
                        state,
                    },
                },
            );
            let write_result = (|| -> Result<(), X11SetupSocketError> {
                let mut stream = stream
                    .lock()
                    .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
                let sequence = sequence.load(Ordering::Acquire);
                write_xi_u16(byte_order, &mut record[2..4], sequence);
                let transition = match event {
                    XAuthorityInputEvent::Key(_) if focus_sent_to != Some(delivered_window) => {
                        Some((focus_sent_to, 10, 9))
                    }
                    XAuthorityInputEvent::Pointer(_)
                        if pointer_sent_to != Some(delivered_window) =>
                    {
                        Some((pointer_sent_to, 8, 7))
                    }
                    _ => None,
                };
                if let Some((previous, out_type, in_type)) = transition {
                    if let Some(previous) = previous
                        && xi_transition_mask & (1 << out_type) != 0
                    {
                        stream
                            .write_all(&encode_xi_crossing_event(
                                byte_order, sequence, out_type, event, previous,
                            ))
                            .map_err(|error| {
                                X11SetupSocketError::new(format!(
                                    "failed to write XI2 leave/focus-out event: {error}"
                                ))
                            })?;
                    }
                    if xi_transition_mask & (1 << in_type) != 0 {
                        stream
                            .write_all(&encode_xi_crossing_event(
                                byte_order,
                                sequence,
                                in_type,
                                event,
                                delivered_window,
                            ))
                            .map_err(|error| {
                                X11SetupSocketError::new(format!(
                                    "failed to write XI2 enter/focus-in event: {error}"
                                ))
                            })?;
                    }
                    if matches!(event, XAuthorityInputEvent::Pointer(_)) {
                        pointer_sent_to = Some(delivered_window);
                    }
                }
                if matches!(event, XAuthorityInputEvent::Key(_))
                    && focus_sent_to != Some(delivered_focus)
                {
                    let focus = encode_x_client_event(
                        byte_order,
                        XClientEvent::Focus {
                            sequence,
                            focused: true,
                            detail: 3,
                            event: delivered_focus,
                            mode: 0,
                        },
                    );
                    stream.write_all(&focus).map_err(|error| {
                        X11SetupSocketError::new(format!(
                            "failed to write X11 focus event: {error}"
                        ))
                    })?;
                    focus_sent_to = Some(delivered_focus);
                }
                stream.write_all(&record).map_err(|error| {
                    if is_x11_client_disconnect(&error) {
                        X11SetupSocketError::client_disconnect(format!(
                            "X11 client disconnected while writing input: {error}"
                        ))
                    } else {
                        X11SetupSocketError::new(format!(
                            "failed to write X11 input event: {error}"
                        ))
                    }
                })?;
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                    use std::os::fd::AsRawFd;
                    eprintln!(
                        "sophia_x11_socket_write schema=1 writer=input fd={} bytes={} head={:02x}{:02x}{:02x}{:02x}",
                        stream.as_raw_fd(),
                        record.len(),
                        record[0],
                        record[1],
                        record[2],
                        record[3],
                    );
                }
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some()
                    && let XAuthorityInputEvent::Key(key) = event
                {
                    eprintln!(
                        "sophia_x11_key_delivery schema=2 stage=wire_flushed keycode={} pressed={} window={:#x} sequence={sequence}",
                        key.keycode,
                        key.pressed,
                        delivered_window.local.raw(),
                    );
                }
                if let XAuthorityInputEvent::Key(key) = event {
                    let previous = xkb_modifiers.swap(key.state, Ordering::AcqRel);
                    let changed = previous ^ key.state;
                    let selected = xkb_state_details.load(Ordering::Acquire);
                    if changed != 0 && selected & 1 != 0 {
                        let state_notify = encode_x_client_event(
                            byte_order,
                            XClientEvent::XkbStateNotify {
                                sequence,
                                time: key.time_msec,
                                modifiers: key.state as u8,
                                changed: 1,
                                keycode: key.keycode,
                                event_type: if key.pressed { 2 } else { 3 },
                            },
                        );
                        stream.write_all(&state_notify).map_err(|error| {
                            X11SetupSocketError::new(format!(
                                "failed to write XKB state notification: {error}"
                            ))
                        })?;
                    }
                }
                if let Some(event_type) = xi_event_type {
                    let generic = encode_xi_device_event(
                        byte_order,
                        sequence,
                        event_type,
                        event,
                        delivered_window,
                    );
                    stream.write_all(&generic).map_err(|error| {
                        X11SetupSocketError::new(format!(
                            "failed to write XI2 generic event: {error}"
                        ))
                    })?;
                }
                stream.flush().map_err(|error| {
                    X11SetupSocketError::new(format!("failed to flush X11 input event: {error}"))
                })
            })();
            match write_result {
                Ok(()) => receiver.send_delivery(
                    client,
                    delivery,
                    XAuthorityInputDeliveryOutcome::Flushed,
                )?,
                Err(error) => {
                    if error.client_disconnect {
                        return Ok(());
                    }
                    let _ = receiver.send_delivery(
                        client,
                        delivery,
                        XAuthorityInputDeliveryOutcome::WriteFailed,
                    );
                    return Err(error);
                }
            }
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

#[cfg(all(test, unix))]
mod routing_tests {
    use super::*;
    use sophia_protocol::{DeviceId, Point};
    use std::sync::mpsc::sync_channel;

    #[test]
    fn listener_transaction_ids_are_global_across_client_workers() {
        let state = X11CoreSocketServerState::new();
        let first_worker = state.clone();
        let second_worker = state.clone();

        let first = first_worker.allocate_transaction().unwrap();
        let second = second_worker.allocate_transaction().unwrap();
        let third = first_worker.allocate_transaction().unwrap();

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_eq!(first.raw() + 1, second.raw());
        assert_eq!(second.raw() + 1, third.raw());
    }

    #[test]
    fn routed_input_discards_another_clients_event() {
        let first = XServerFrontendClientId(1);
        let second = XServerFrontendClientId(2);
        let (sender, receiver) = sync_channel(2);
        sender
            .send(XAuthorityClientInputEvent {
                client: second,
                event: XAuthorityKeyEvent {
                    keycode: 24,
                    pressed: true,
                    state: 0,
                    time_msec: 1,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        sender
            .send(XAuthorityClientInputEvent {
                client: first,
                event: XAuthorityKeyEvent {
                    keycode: 25,
                    pressed: true,
                    state: 0,
                    time_msec: 2,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();

        let receiver = X11InputEventReceiver::Routed {
            receiver,
            deliveries: None,
        };
        assert_eq!(receiver.recv_timeout(first), Err(RecvTimeoutError::Timeout));
        assert_eq!(
            receiver.recv_timeout(first).unwrap(),
            (
                XAuthorityInputEvent::Key(XAuthorityKeyEvent {
                    keycode: 25,
                    pressed: true,
                    state: 0,
                    time_msec: 2,
                }),
                None,
                None,
                0,
                None,
            )
        );
    }

    #[test]
    fn routed_control_discards_another_clients_command_and_labels_its_ack() {
        let first = XServerFrontendClientId(1);
        let second = XServerFrontendClientId(2);
        let surface = SurfaceId::new(44, 1);
        let (command_sender, command_receiver) = sync_channel(2);
        let (ack_sender, ack_receiver) = sync_channel(1);
        let command = XAuthorityControlCommand::FocusSurface {
            transaction: TransactionId::from_raw(7),
            surface,
        };
        command_sender
            .send(XAuthorityClientControlCommand {
                client: second,
                command,
            })
            .unwrap();
        command_sender
            .send(XAuthorityClientControlCommand {
                client: first,
                command,
            })
            .unwrap();

        let channels = X11ControlChannels::Routed {
            receiver: command_receiver,
            acknowledgements: ack_sender,
        };
        assert_eq!(channels.recv_timeout(first), Err(RecvTimeoutError::Timeout));
        assert_eq!(channels.recv_timeout(first).unwrap(), command);
        let acknowledgement = XAuthorityControlAck {
            transaction: command.transaction(),
            surface: command.surface(),
            outcome: XAuthorityControlOutcome::Delivered,
        };
        channels.send_ack(first, acknowledgement).unwrap();
        assert_eq!(
            ack_receiver.recv().unwrap(),
            XAuthorityClientControlAck {
                client: first,
                acknowledgement,
            }
        );
    }

    #[test]
    fn route_broker_delivers_to_the_registered_client_only() {
        let client = XServerFrontendClientId(9);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
        let (registration, channels) = broker.registry.register_client(client).unwrap();
        let input = XAuthorityInputEvent::Key(XAuthorityKeyEvent {
            keycode: 38,
            pressed: true,
            state: 0,
            time_msec: 3,
        });
        let command = XAuthorityControlCommand::FocusSurface {
            transaction: TransactionId::from_raw(8),
            surface: SurfaceId::new(45, 1),
        };

        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: input,
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        broker
            .control_sender()
            .send(XAuthorityClientControlCommand { client, command })
            .unwrap();

        assert_eq!(broker.route_pending(), Ok(2));
        assert_eq!(
            channels.input.recv().unwrap(),
            XAuthorityClientInputEvent {
                client,
                event: input,
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            }
        );
        assert_eq!(channels.control.recv().unwrap(), command);
        let acknowledgement = XAuthorityControlAck {
            transaction: command.transaction(),
            surface: command.surface(),
            outcome: XAuthorityControlOutcome::Delivered,
        };
        let channels = X11ControlChannels::ClientBound {
            receiver: channels.control,
            acknowledgements: broker.registry.acknowledgement_sender.clone(),
        };
        channels.send_ack(client, acknowledgement).unwrap();
        assert_eq!(
            broker
                .recv_control_ack_timeout(Duration::from_millis(1))
                .unwrap(),
            XAuthorityClientControlAck {
                client,
                acknowledgement,
            }
        );
        assert_eq!(broker.registered_client_count(), 1);

        drop(registration);
        assert_eq!(broker.registered_client_count(), 0);
        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: input,
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: None,
            })
            .unwrap();
        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::UnknownClient { client })
        );
    }

    #[test]
    fn clearing_old_present_selection_preserves_active_window_feedback() {
        let namespace = NamespaceId::from_raw(10);
        let client = XServerFrontendClientId(9);
        let surface = SurfaceId::new(11, 1);
        let bootstrap_window = XResourceId::new(0x200009, 1);
        let bootstrap_event = XResourceId::new(0x20000d, 1);
        let main_window = XResourceId::new(0x200010, 1);
        let main_event = XResourceId::new(0x200014, 1);
        let pixmap = XResourceId::new(0x200015, 1);
        let idle_fence = XResourceId::new(0x200016, 1);
        let transaction = TransactionId::from_raw(202);
        let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(8).unwrap());
        let (_registration, channels) = broker.registry.register_client(client).unwrap();
        broker
            .registry
            .register_surface(client, namespace, surface, main_window)
            .unwrap();

        broker
            .registry
            .select_present_input(client, bootstrap_event, bootstrap_window, 7)
            .unwrap();
        broker
            .registry
            .select_present_input(client, main_event, main_window, 7)
            .unwrap();
        broker
            .registry
            .select_present_input(client, bootstrap_event, bootstrap_window, 0)
            .unwrap();
        broker
            .registry
            .queue_present(
                transaction,
                client,
                main_window,
                pixmap,
                1,
                Some(idle_fence),
            )
            .unwrap();

        assert_eq!(
            broker.route_present_complete(
                transaction,
                1_188_203,
                7_668_086,
                XPresentCompletionMode::Flip,
            ),
            Ok(true)
        );
        assert_eq!(
            channels.protocol.recv().unwrap(),
            XClientEvent::PresentCompleteNotify {
                sequence: 0,
                event_id: main_event,
                window: main_window,
                serial: 1,
                ust: 1_188_203,
                msc: 7_668_086,
                mode: XPresentCompletionMode::Flip as u8,
            }
        );
        assert_eq!(broker.route_present_idle(transaction), Ok(true));
        assert_eq!(
            channels.protocol.recv().unwrap(),
            XClientEvent::PresentIdleNotify {
                sequence: 0,
                event_id: main_event,
                window: main_window,
                serial: 1,
                pixmap,
                idle_fence: Some(idle_fence),
            }
        );
    }

    #[test]
    fn present_feedback_reaches_every_matching_event_selection() {
        let namespace = NamespaceId::from_raw(10);
        let client = XServerFrontendClientId(10);
        let surface = SurfaceId::new(12, 1);
        let window = XResourceId::new(0x300010, 1);
        let first_event = XResourceId::new(0x300014, 1);
        let second_event = XResourceId::new(0x300015, 1);
        let pixmap = XResourceId::new(0x300016, 1);
        let transaction = TransactionId::from_raw(203);
        let broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(8).unwrap());
        let (registration, channels) = broker.registry.register_client(client).unwrap();
        broker
            .registry
            .register_surface(client, namespace, surface, window)
            .unwrap();
        for event_id in [first_event, second_event] {
            broker
                .registry
                .select_present_input(client, event_id, window, 7)
                .unwrap();
        }
        broker
            .registry
            .queue_present(transaction, client, window, pixmap, 2, None)
            .unwrap();

        assert_eq!(
            broker.route_present_complete(transaction, 10, 20, XPresentCompletionMode::Flip),
            Ok(true)
        );
        for event_id in [first_event, second_event] {
            assert!(matches!(
                channels.protocol.recv().unwrap(),
                XClientEvent::PresentCompleteNotify {
                    event_id: routed_event,
                    ..
                } if routed_event == event_id
            ));
        }
        assert_eq!(broker.route_present_idle(transaction), Ok(true));
        for event_id in [first_event, second_event] {
            assert!(matches!(
                channels.protocol.recv().unwrap(),
                XClientEvent::PresentIdleNotify {
                    event_id: routed_event,
                    ..
                } if routed_event == event_id
            ));
        }

        let disconnected = TransactionId::from_raw(204);
        broker
            .registry
            .queue_present(disconnected, client, window, pixmap, 3, None)
            .unwrap();
        drop(registration);
        assert_eq!(
            broker.route_present_complete(disconnected, 30, 40, XPresentCompletionMode::Flip,),
            Ok(false)
        );
    }

    #[test]
    fn route_broker_fails_closed_when_a_client_queue_is_backpressured() {
        let client = XServerFrontendClientId(10);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(1).unwrap());
        let (_registration, _channels) = broker.registry.register_client(client).unwrap();
        for time_msec in [4, 5] {
            broker
                .input_sender()
                .send(XAuthorityClientInputEvent {
                    client,
                    event: XAuthorityKeyEvent {
                        keycode: 39,
                        pressed: true,
                        state: 0,
                        time_msec,
                    }
                    .into(),
                    target_window: None,
                    xi_event_type: None,
                    xi_transition_mask: 0,
                    delivery: None,
                })
                .unwrap();
            if time_msec == 4 {
                assert_eq!(broker.route_pending(), Ok(1));
            }
        }

        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::ClientQueueFull { client })
        );
    }

    #[test]
    fn route_broker_reports_rejected_delivery_for_an_unknown_client() {
        let client = XServerFrontendClientId(12);
        let (control_ack_sender, _control_ack_receiver) = sync_channel(1);
        let (delivery_sender, delivery_receiver) = sync_channel(1);
        let mut broker = XServerFrontendRouteBroker::with_control_and_input_delivery_senders(
            NonZeroUsize::new(1).unwrap(),
            control_ack_sender,
            delivery_sender,
        );
        let delivery = XAuthorityInputDeliveryId::from_raw(7);
        broker
            .input_sender()
            .send(XAuthorityClientInputEvent {
                client,
                event: XAuthorityKeyEvent {
                    keycode: 38,
                    pressed: true,
                    state: 0,
                    time_msec: 1,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: Some(delivery),
            })
            .unwrap();

        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::UnknownClient { client })
        );
        assert_eq!(
            delivery_receiver.recv().unwrap(),
            XAuthorityClientInputDelivery {
                client,
                delivery,
                outcome: XAuthorityInputDeliveryOutcome::RouteRejected,
            }
        );
    }

    #[test]
    fn active_keyboard_grab_redirects_engine_routed_input_and_window() {
        let namespace = NamespaceId::from_raw(9);
        let focused = XServerFrontendClientId(1);
        let grabber = XServerFrontendClientId(2);
        let surface = SurfaceId::new(10, 1);
        let grab_window = XResourceId::new(0x400001, 1);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(4).unwrap());
        let (_focused_registration, focused_channels) =
            broker.registry.register_client(focused).unwrap();
        let (_grab_registration, grab_channels) = broker.registry.register_client(grabber).unwrap();
        broker
            .registry
            .register_surface(focused, namespace, surface, XResourceId::new(0x200001, 1))
            .unwrap();
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .grab_keyboard(
                namespace,
                crate::XActiveInputGrab {
                    owner: grabber.raw(),
                    window: grab_window,
                    owner_events: false,
                    pointer_mode: 1,
                    keyboard_mode: 1,
                    event_mask: 0,
                },
            )
            .unwrap();
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .select_xi_events(namespace, grabber.raw(), grab_window, &[(1, vec![1 << 2])]);
        broker
            .routed_input_sender()
            .send(XAuthorityRoutedInput {
                request: RoutedInputRequest {
                    serial: 1,
                    seat: SeatId::from_raw(1),
                    device: DeviceId::from_raw(1),
                    time_msec: 1,
                    target_surface: surface,
                    global_position: Point::default(),
                    local_position: Point::default(),
                    kind: InputEventKind::Key {
                        keycode: 30,
                        pressed: true,
                    },
                },
                delivery: None,
            })
            .unwrap();
        assert_eq!(broker.route_pending(), Ok(1));
        assert!(matches!(
            focused_channels.input.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        let routed = grab_channels.input.recv().unwrap();
        assert_eq!(routed.client, grabber);
        assert_eq!(routed.target_window, Some(grab_window));
        assert_eq!(routed.xi_event_type, Some(2));
    }

    #[test]
    fn synchronous_keyboard_grab_queues_until_allow_events() {
        let namespace = NamespaceId::from_raw(10);
        let client = XServerFrontendClientId(3);
        let surface = SurfaceId::new(11, 1);
        let mut broker = XServerFrontendRouteBroker::new(NonZeroUsize::new(2).unwrap());
        let (_registration, channels) = broker.registry.register_client(client).unwrap();
        broker
            .registry
            .register_surface(client, namespace, surface, XResourceId::new(0x200001, 1))
            .unwrap();
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .grab_keyboard(
                namespace,
                crate::XActiveInputGrab {
                    owner: client.raw(),
                    window: XResourceId::new(0x200001, 1),
                    owner_events: false,
                    pointer_mode: 1,
                    keyboard_mode: 0,
                    event_mask: 0,
                },
            )
            .unwrap();
        broker
            .routed_input_sender()
            .send(XAuthorityRoutedInput {
                request: RoutedInputRequest {
                    serial: 2,
                    seat: SeatId::from_raw(1),
                    device: DeviceId::from_raw(1),
                    time_msec: 2,
                    target_surface: surface,
                    global_position: Point::default(),
                    local_position: Point::default(),
                    kind: InputEventKind::Key {
                        keycode: 30,
                        pressed: true,
                    },
                },
                delivery: None,
            })
            .unwrap();
        assert_eq!(broker.route_pending(), Ok(1));
        assert!(matches!(
            channels.input.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        broker
            .registry
            .input_authority
            .lock()
            .unwrap()
            .allow_events(namespace, client.raw(), 3)
            .unwrap();
        assert_eq!(broker.route_pending(), Ok(1));
        assert_eq!(channels.input.recv().unwrap().client, client);
    }

    #[test]
    fn xi2_device_event_uses_xge_header_and_fp1616_local_coordinates() {
        let bytes = encode_xi_device_event(
            XByteOrder::LittleEndian,
            7,
            6,
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind: XAuthorityPointerEventKind::Motion,
                surface: SurfaceId::new(1, 1),
                root_x: 11,
                root_y: 12,
                event_x: 3,
                event_y: -4,
                state: 5,
                time_msec: 9,
            }),
            XResourceId::new(0x200001, 1),
        );
        assert_eq!(bytes.len(), 80);
        assert_eq!(bytes[0], 35);
        assert_eq!(bytes[1], crate::X_INPUT_MAJOR_OPCODE);
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 6);
        assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 2);
        assert_eq!(
            i32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            3 << 16
        );
        assert_eq!(
            i32::from_le_bytes(bytes[44..48].try_into().unwrap()),
            -4 << 16
        );
        let crossing = encode_xi_crossing_event(
            XByteOrder::LittleEndian,
            8,
            7,
            XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                kind: XAuthorityPointerEventKind::Motion,
                surface: SurfaceId::new(1, 1),
                root_x: 11,
                root_y: 12,
                event_x: 3,
                event_y: -4,
                state: 5,
                time_msec: 9,
            }),
            XResourceId::new(0x200001, 1),
        );
        assert_eq!(crossing.len(), 72);
        assert_eq!(u16::from_le_bytes([crossing[8], crossing[9]]), 7);
        assert_eq!(crossing[48], 1);
    }

    #[test]
    fn keyboard_focus_propagates_only_through_its_ancestor_chain() {
        let mut selections = XCoreEventSelectionState::default();
        let parent = XResourceId::new(0x200007, 1);
        let child = XResourceId::new(0x200001, 1);
        selections.register(child, parent);
        assert_eq!(selections.selected_keyboard_target(child), None);
        selections.update(parent, Some(1), None);

        assert_eq!(selections.keyboard_target(child), parent);
        assert_eq!(selections.selected_keyboard_target(child), Some(parent));

        assert_eq!(
            selections.keyboard_target(XResourceId::new(0x200009, 1)),
            XResourceId::new(0x200009, 1)
        );
    }

    #[test]
    fn keyboard_delivery_falls_back_to_engine_focused_surface() {
        let selections = XCoreEventSelectionState::default();

        assert_eq!(
            selections.keyboard_target(XResourceId::new(0x200001, 1)),
            XResourceId::new(0x200001, 1)
        );
    }

    #[test]
    fn root_focus_uses_mapped_stacking_order_and_restacking() {
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        let lower = XResourceId::new(0x200001, 1);
        let upper = XResourceId::new(0x200002, 1);
        let mut selections = XCoreEventSelectionState::default();
        for window in [lower, upper] {
            selections.register(window, root);
            selections.update(window, Some(1), None);
            selections.observe_mapped(window);
        }
        assert_eq!(selections.keyboard_target(root), upper);

        selections.restack(lower, Some(upper), Some(0));
        assert_eq!(selections.keyboard_target(root), lower);

        selections.observe_unmapped(lower);
        assert_eq!(selections.keyboard_target(root), upper);
    }
}

#[cfg(unix)]
fn x11_trace_token(value: Option<&str>) -> String {
    value
        .unwrap_or("none")
        .chars()
        .take(512)
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(
                    character,
                    '-' | '_' | '.' | ':' | '=' | ',' | '{' | '}' | '[' | ']'
                )
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

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
        crate::XWireRequest::GetGeometry { drawable } => {
            Some(format!("GetGeometry:drawable={:#x}", drawable.local.raw()))
        }
        crate::XWireRequest::PresentPixmap {
            window,
            pixmap,
            valid_region,
            update_region,
            target_crtc,
            wait_fence,
            idle_fence,
            options,
            divisor,
            remainder,
            ..
        } => Some(format!(
            "PRESENT:Pixmap:window={:#x}:pixmap={:#x}:valid={valid_region:#x}:update={update_region:#x}:crtc={target_crtc:#x}:wait={:#x}:idle={:#x}:options={options:#x}:divisor={divisor}:remainder={remainder}",
            window.local.raw(),
            pixmap.local.raw(),
            wait_fence.map_or(0, |fence| fence.local.raw()),
            idle_fence.map_or(0, |fence| fence.local.raw()),
        )),
        crate::XWireRequest::XfixesQueryVersion { .. } => Some("XFIXES:QueryVersion".to_owned()),
        crate::XWireRequest::XfixesCreateRegion { region, rectangles } => Some(format!(
            "XFIXES:CreateRegion:region={:#x}:rectangles={}",
            region.local.raw(),
            rectangles.len()
        )),
        crate::XWireRequest::XfixesDestroyRegion { region } => Some(format!(
            "XFIXES:DestroyRegion:region={:#x}",
            region.local.raw()
        )),
        crate::XWireRequest::XfixesSetRegion { region, rectangles } => Some(format!(
            "XFIXES:SetRegion:region={:#x}:rectangles={}",
            region.local.raw(),
            rectangles.len()
        )),
        crate::XWireRequest::Dri3PixmapFromBuffers {
            pixmap,
            window,
            num_buffers,
            width,
            height,
            modifier,
            ..
        } => Some(format!(
            "DRI3:PixmapFromBuffers:pixmap={:#x}:window={:#x}:buffers={}:{}x{}:modifier={modifier:#x}",
            pixmap.local.raw(),
            window.local.raw(),
            num_buffers,
            width,
            height
        )),
        crate::XWireRequest::GlxQueryServerString { name } => {
            Some(format!("GLX:QueryServerString:name={name:#x}"))
        }
        crate::XWireRequest::GlxGetVisualConfigs { screen } => {
            Some(format!("GLX:GetVisualConfigs:screen={screen}"))
        }
        crate::XWireRequest::GlxGetFbConfigs { screen } => {
            Some(format!("GLX:GetFBConfigs:screen={screen}"))
        }
        crate::XWireRequest::GlxCreateContext {
            context, fbconfig, ..
        } => Some(format!(
            "GLX:CreateContext:context={:#x}:fbconfig={fbconfig}",
            context.local.raw()
        )),
        crate::XWireRequest::GlxCreateWindow {
            glx_window,
            fbconfig,
            ..
        } => Some(format!(
            "GLX:CreateWindow:window={:#x}:fbconfig={fbconfig}",
            glx_window.local.raw()
        )),
        crate::XWireRequest::InternAtom { name, .. } => Some(format!("InternAtom:{name}")),
        crate::XWireRequest::ChangeWindowAttributes { window, .. } => Some(format!(
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
            ..
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
            ..
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
        crate::XWireRequest::ReparentWindow {
            window,
            parent,
            x,
            y,
        } => Some(format!(
            "ReparentWindow:window={:#x}:parent={:#x}:x={}:y={}",
            window.local.raw(),
            parent.local.raw(),
            x,
            y
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
        crate::XWireRequest::GetImage {
            drawable,
            width,
            height,
            ..
        } => Some(format!(
            "GetImage:drawable={:#x}:{}x{}",
            drawable.local.raw(),
            width,
            height
        )),
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
        crate::XWireRequest::ShmCreatePixmap {
            pixmap,
            drawable,
            segment,
            width,
            height,
            ..
        } => Some(format!(
            "MIT-SHM:CreatePixmap:pixmap={:#x}:drawable={:#x}:segment={:#x}:{}x{}",
            pixmap.local.raw(),
            drawable.local.raw(),
            segment.local.raw(),
            width,
            height
        )),
        crate::XWireRequest::Dri3Open { drawable, provider } => Some(format!(
            "DRI3:Open:drawable={:#x}:provider={provider:#x}",
            drawable.local.raw()
        )),
        crate::XWireRequest::Dri3GetSupportedModifiers {
            window,
            depth,
            bits_per_pixel,
        } => Some(format!(
            "DRI3:GetSupportedModifiers:window={:#x}:depth={depth}:bpp={bits_per_pixel}",
            window.local.raw()
        )),
        crate::XWireRequest::RandrQueryVersion { .. } => Some("RANDR:QueryVersion".to_string()),
        crate::XWireRequest::RandrSelectInput { window, .. } => {
            Some(format!("RANDR:SelectInput:{:#x}", window.local.raw()))
        }
        crate::XWireRequest::RandrGetScreenResources { current, .. } => {
            Some(format!("RANDR:GetScreenResources:current={current}"))
        }
        crate::XWireRequest::RandrGetOutputInfo { output, .. } => {
            Some(format!("RANDR:GetOutputInfo:{output:#x}"))
        }
        crate::XWireRequest::RandrGetCrtcInfo { crtc, .. } => {
            Some(format!("RANDR:GetCrtcInfo:{crtc:#x}"))
        }
        crate::XWireRequest::RandrGetOutputPrimary { window } => {
            Some(format!("RANDR:GetOutputPrimary:{:#x}", window.local.raw()))
        }
        crate::XWireRequest::RandrGetMonitors { window, .. } => {
            Some(format!("RANDR:GetMonitors:{:#x}", window.local.raw()))
        }
        crate::XWireRequest::XkbUseExtension { .. } => Some("XKEYBOARD:UseExtension".to_string()),
        crate::XWireRequest::XkbGetControls => Some("XKEYBOARD:GetControls".to_string()),
        crate::XWireRequest::XkbGetMap { full, partial } => Some(format!(
            "XKEYBOARD:GetMap:full={full:#x}:partial={partial:#x}"
        )),
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

/// Send one X11 output record while attaching its descriptors exactly once.
///
/// `SCM_RIGHTS` accompanies the first successful byte range. If the stream
/// accepts only part of the byte payload, the remainder is written without
/// ancillary data so the receiver cannot observe duplicate descriptors.
#[cfg(unix)]
pub fn write_x11_socket_output_record(
    stream: &mut UnixStream,
    record: X11SocketOutputRecord,
) -> std::io::Result<()> {
    let X11SocketOutputRecord { bytes, fds } = record;
    if fds.is_empty() {
        return stream.write_all(&bytes);
    }

    let borrowed = fds.iter().map(AsFd::as_fd).collect::<Vec<_>>();
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut ancillary_space);
    if !ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)) {
        return Err(std::io::Error::other(
            "failed to encode X11 output file descriptors",
        ));
    }

    let sent = loop {
        match rustix::net::sendmsg(
            &*stream,
            &[IoSlice::new(&bytes)],
            &mut ancillary,
            rustix::net::SendFlags::empty(),
        ) {
            Ok(sent) => break sent,
            Err(error) => {
                let error = std::io::Error::from(error);
                if error.kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
        }
    };
    if sent == 0 {
        return Err(std::io::Error::new(
            ErrorKind::WriteZero,
            "failed to write X11 output record",
        ));
    }
    stream.write_all(&bytes[sent..])
}

#[cfg(unix)]
#[derive(Debug)]
pub struct X11ReceivedCoreRequest {
    pub major_opcode: u8,
    pub bytes: Vec<u8>,
    pub fds: Vec<OwnedFd>,
}

pub fn read_x11_core_request(
    stream: &mut UnixStream,
    byte_order: crate::XByteOrder,
) -> Result<Option<X11ReceivedCoreRequest>, X11SetupSocketError> {
    let mut header = [0; 4];
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut ancillary_space);
    let mut iov = [IoSliceMut::new(&mut header)];
    let received = match rustix::net::recvmsg(
        &*stream,
        &mut iov,
        &mut ancillary,
        rustix::net::RecvFlags::CMSG_CLOEXEC,
    ) {
        Ok(received) => received,
        Err(error) => {
            let error = std::io::Error::from(error);
            if matches!(
                error.kind(),
                ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionReset
                    | ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
            ) {
                return Ok(None);
            }
            return Err(X11SetupSocketError::new(format!(
                "failed to read X11 request header: {error}"
            )));
        }
    };
    if received.bytes == 0 {
        return Ok(None);
    }
    if received.flags.contains(rustix::net::ReturnFlags::CTRUNC) {
        return Err(X11SetupSocketError::new(
            "X11 request carried too many ancillary file descriptors",
        ));
    }
    let mut fds = Vec::new();
    for message in ancillary.drain() {
        if let rustix::net::RecvAncillaryMessage::ScmRights(rights) = message {
            fds.extend(rights);
        }
    }
    if fds.len() > sophia_protocol::DMA_BUF_MAX_PLANES {
        return Err(X11SetupSocketError::new(
            "X11 request carried too many file descriptors",
        ));
    }
    if received.bytes < header.len() {
        match stream.read_exact(&mut header[received.bytes..]) {
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
    }

    let length = usize::from(byte_order.u16(&header[2..4])) * 4;
    if length < 4 {
        return Ok(Some(X11ReceivedCoreRequest {
            major_opcode: header[0],
            bytes: header.to_vec(),
            fds,
        }));
    }
    // The setup reply advertises the full core u16 request-length range. Keep
    // the socket reader consistent with that wire contract: Firefox emits
    // large, but still ordinary, requests just below the 65,535-unit limit.
    // BIG-REQUESTS extended (zero u16 plus u32 length) frames remain outside
    // this bounded reader until a captured client requires them.
    let max_len = usize::from(crate::X_SETUP_DEFAULT_MAX_REQUEST_UNITS) * 4;
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

    Ok(Some(X11ReceivedCoreRequest {
        major_opcode: header[0],
        bytes: request,
        fds,
    }))
}

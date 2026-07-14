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
    collections::{BTreeMap, BTreeSet},
    io::{ErrorKind, Read, Write},
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
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
    XAuthorityObservedTransactionBatch, XAuthorityResponsePacket, XAuthorityRuntime, XByteOrder,
    XClientEvent, XDispatchContext, XDispatchResult, XPropertyTable, XResourceId, XSetupFailure,
    XSetupRequest, XSetupSuccess, XWireClientContext, decode_x11_core_request,
    dispatch_x11_parse_error, dispatch_x11_wire_request, encode_x_client_event,
    encode_x11_setup_failure, encode_x11_setup_success, parse_x11_setup_request,
    try_emit_x_authority_trace, x11_setup_request_total_len,
};
#[cfg(unix)]
use sophia_protocol::{
    ClientAdmissionContext, ClientAdmissionId, ClientAuthenticationMethod, NamespaceCapabilities,
    NamespaceContext, NamespaceId, NamespaceProfile, Size, SurfaceId, TransactionId,
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
    max_concurrent_clients: NonZeroUsize,
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
            .field("max_concurrent_clients", &self.max_concurrent_clients)
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
            max_concurrent_clients: X_SERVER_FRONTEND_DEFAULT_MAX_CONCURRENT_CLIENTS,
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

    /// Sets the upper bound for simultaneously dispatched X11 clients.
    ///
    /// The default allows sixteen connections. This bound applies only to the
    /// opt-in concurrent dispatcher; the existing sequential APIs still serve
    /// one connection at a time.
    pub fn with_max_concurrent_clients(mut self, max_concurrent_clients: NonZeroUsize) -> Self {
        self.max_concurrent_clients = max_concurrent_clients;
        self
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
        let (worker_completion_sender, worker_completions) = std::sync::mpsc::channel();
        let (worker_admission_event_sender, worker_admission_events) = std::sync::mpsc::channel();
        Ok(Self {
            config,
            listener,
            state: X11CoreSocketServerState::new(),
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
        completion.result
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
        let outcome = self
            .state
            .runtime
            .lock()
            .map_err(|_| X11SetupSocketError::new("X11 authority runtime lock poisoned"))?
            .fail_clipboard_transfer(transfer, error)
            .map_err(|error| {
                X11SetupSocketError::new(format!("clipboard failure rejected: {error:?}"))
            })?;
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
    /// Opaque Engine-assigned token used to prove that the owning X11 worker
    /// flushed this event to its client socket. It deliberately carries no
    /// X11 resource, key, or text identity.
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

/// A control acknowledgement bound to the connection that applied it.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAuthorityClientControlAck {
    pub client: XServerFrontendClientId,
    pub acknowledgement: XAuthorityControlAck,
}

/// Service-supervision command for a long-running routed X11 frontend.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendServiceCommand {
    /// Stop accepting new clients and drain workers that are already connected.
    StopAccepting,
    /// Disconnect the worker holding this session-issued admission. Its normal
    /// teardown releases routes and resources before revoking the lease.
    RevokeAdmission { admission: ClientAdmissionId },
}

/// Routing failure between the Engine-facing ingress queues and a live X11
/// client worker.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XServerFrontendRouteError {
    UnknownClient { client: XServerFrontendClientId },
    ClientQueueFull { client: XServerFrontendClientId },
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
            Self::ClientQueueFull { client } => {
                write!(
                    formatter,
                    "X11 route queue is full for client {}",
                    client.raw()
                )
            }
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
    control_sender: SyncSender<XAuthorityClientControlCommand>,
    control_receiver: Receiver<XAuthorityClientControlCommand>,
    acknowledgement_receiver: Option<Receiver<XAuthorityClientControlAck>>,
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

    fn with_transports(
        queue_capacity: NonZeroUsize,
        acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
        acknowledgement_receiver: Option<Receiver<XAuthorityClientControlAck>>,
        input_delivery_sender: Option<SyncSender<XAuthorityClientInputDelivery>>,
    ) -> Self {
        let capacity = queue_capacity.get();
        let (input_sender, input_receiver) = sync_channel(capacity);
        let (control_sender, control_receiver) = sync_channel(capacity);
        Self {
            registry: XServerFrontendRouteRegistry {
                clients: Arc::new(Mutex::new(BTreeMap::new())),
                acknowledgement_sender,
                input_delivery_sender,
                per_client_queue_capacity: queue_capacity,
            },
            input_sender,
            input_receiver,
            control_sender,
            control_receiver,
            acknowledgement_receiver,
        }
    }

    pub fn input_sender(&self) -> SyncSender<XAuthorityClientInputEvent> {
        self.input_sender.clone()
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

    /// Routes every value currently available at the bounded ingress.
    pub fn route_pending(&mut self) -> Result<usize, XServerFrontendRouteError> {
        let mut routed = 0usize;
        loop {
            let mut progressed = false;
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
            if !progressed {
                return Ok(routed);
            }
        }
    }

    pub fn registered_client_count(&self) -> usize {
        self.registry.registered_client_count()
    }
}

#[cfg(unix)]
#[derive(Clone)]
struct XServerFrontendRouteRegistry {
    clients: Arc<Mutex<BTreeMap<XServerFrontendClientId, XServerFrontendClientRouteSenders>>>,
    acknowledgement_sender: SyncSender<XAuthorityClientControlAck>,
    input_delivery_sender: Option<SyncSender<XAuthorityClientInputDelivery>>,
    per_client_queue_capacity: NonZeroUsize,
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
        self.route_to_client(client, sender, event)
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
    ) -> Result<(XAuthorityInputEvent, Option<XAuthorityInputDeliveryId>), RecvTimeoutError> {
        loop {
            match self {
                Self::Plain(receiver) => {
                    return receiver
                        .recv_timeout(Duration::from_millis(10))
                        .map(|event| (event, None));
                }
                Self::Routed { receiver, .. } => {
                    match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(route) if route.client == client => {
                            return Ok((route.event, route.delivery));
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
#[derive(Clone, Debug)]
pub struct X11CoreSocketServerState {
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    atoms: Arc<Mutex<XAtomTable>>,
    properties: Arc<Mutex<XPropertyTable>>,
    clients: Arc<Mutex<X11CoreClientLeaseState>>,
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
        }
    }
}

#[cfg(unix)]
impl X11CoreSocketServerState {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_client_setup_success(
        &self,
    ) -> Result<(XServerFrontendClientLease, XSetupSuccess), X11SetupSocketError> {
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
    let authorization = XServerFrontendSetupAuthorization::default();
    serve_x11_setup_socket_client_with_setup_authorization(stream, &authorization, |_| {
        Ok(Some(XSetupSuccess::client_compatible()))
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
    let resource_id_range = client_lease.resource_id_range;
    let mut sequence = 0u16;
    let event_sequence = Arc::new(AtomicU16::new(0));
    let focused_surface_window = Arc::new(AtomicU64::new(u64::from(X_SETUP_DEFAULT_ROOT)));
    let keyboard_event_window = Arc::new(AtomicU64::new(u64::from(X_SETUP_DEFAULT_ROOT)));
    let keyboard_target_selected = Arc::new(AtomicBool::new(false));
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
                keyboard_event_window.clone(),
                keyboard_target_selected.clone(),
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
                state.runtime.clone(),
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
        while let Some((major_opcode, request)) = read_x11_core_request(stream, setup.byte_order)? {
            sequence = sequence.wrapping_add(1);
            event_sequence.store(sequence, Ordering::Release);
            let keyboard_event_target = x11_keyboard_event_target(&request, setup.byte_order);
            let dispatch_context = XDispatchContext {
                byte_order: setup.byte_order,
                namespace,
                sequence,
                major_opcode,
            };
            let mut parse_error = None;
            let mut request_detail = None;
            let (mut output, cpu_buffer_update) = match decode_x11_core_request(
                XWireClientContext {
                    byte_order: setup.byte_order,
                    namespace,
                    transaction: TransactionId::from_raw(u64::from(sequence)),
                    resource_id_range: Some(resource_id_range),
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
                    let default_map_target = if !keyboard_target_selected.load(Ordering::Acquire)
                        && let crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
                            kind: crate::XAuthorityRequestKind::MapWindow { window, .. },
                            ..
                        }) = &request
                    {
                        Some(*window)
                    } else {
                        None
                    };
                    request_detail = x11_core_request_trace_detail(&request);
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
                    let output = dispatch_x11_wire_request(
                        dispatch_context,
                        request,
                        &mut runtime,
                        &mut atoms,
                        &mut properties,
                    );
                    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some()
                        && let Some(detail) = request_detail.as_deref()
                        && detail.starts_with("GetKeyboardMapping:")
                    {
                        eprintln!("sophia_x11_keyboard_map schema=1 {detail}");
                    }
                    if let Some(window) = keyboard_event_target.or(default_map_target)
                        && (window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT)
                            || runtime.validate_window_access(namespace, window).is_ok())
                    {
                        keyboard_event_window.store(window.local.raw(), Ordering::Release);
                        if keyboard_event_target.is_some() {
                            keyboard_target_selected.store(true, Ordering::Release);
                        }
                    }
                    // The CPU update belongs to this dispatch. Keep it under
                    // the runtime lock so a simultaneous client cannot take
                    // an update generated by this request.
                    let cpu_buffer_update = runtime.take_cpu_buffer_update();
                    (output, cpu_buffer_update)
                }
                Err(error) => {
                    let head = request
                        .iter()
                        .take(24)
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join("");
                    parse_error = Some(format!("{error:?}:len={}:head={head}", request.len()));
                    (dispatch_x11_parse_error(dispatch_context, error), None)
                }
            };
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
                eprintln!(
                    "sophia-x-authority: seq={} opcode={}",
                    sequence, major_opcode
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
    let client_lease = state.release_client(client)?;
    debug_assert_eq!(client_lease.resource_id_range, resource_id_range);
    let release = release_x11_client_lease(state, namespace, client_lease)?;
    drop(route_registration);
    let cleanup_observer_result = if release.removed_surfaces.is_empty() {
        Ok(())
    } else {
        sequence = sequence.wrapping_add(1);
        let transaction = TransactionId::from_raw(u64::from(sequence));
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
            set_x11_selection_event_sequence(&mut event, sequence.load(Ordering::Acquire));
            let record = encode_x_client_event(byte_order, event);
            let mut stream = stream
                .lock()
                .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
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
fn set_x11_selection_event_sequence(event: &mut XClientEvent, value: u16) {
    match event {
        XClientEvent::SelectionClear { sequence, .. }
        | XClientEvent::SelectionRequest { sequence, .. }
        | XClientEvent::SelectionNotify { sequence, .. } => *sequence = value,
        _ => unreachable!("protocol routing accepts only selection events"),
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
    runtime: Arc<Mutex<XAuthorityRuntime>>,
    namespace: NamespaceId,
    client: XServerFrontendClientId,
    channels: X11ControlChannels,
) -> Result<X11ControlWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
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
                XAuthorityControlCommand::FocusSurface { .. } => {
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
            channels.send_ack(
                client,
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
fn clamp_engine_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(unix)]
fn spawn_x11_input_event_writer(
    stream: Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: Arc<AtomicU16>,
    focused_surface_window: Arc<AtomicU64>,
    keyboard_event_window: Arc<AtomicU64>,
    keyboard_target_selected: Arc<AtomicBool>,
    surface_windows: Arc<Mutex<BTreeMap<SurfaceId, XResourceId>>>,
    client: XServerFrontendClientId,
    receiver: X11InputEventReceiver,
) -> Result<X11InputEventWriter, X11SetupSocketError> {
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut focus_sent_to = None;
        while !writer_stop.load(Ordering::Acquire) {
            let (event, delivery) = match receiver.recv_timeout(client) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };
            let focused_window = x11_keyboard_delivery_target(
                &focused_surface_window,
                &keyboard_event_window,
                &keyboard_target_selected,
            );
            if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some()
                && let XAuthorityInputEvent::Key(key) = event
            {
                eprintln!(
                    "sophia_x11_key_delivery schema=1 keycode={} pressed={} state={} target_selected={}",
                    key.keycode,
                    key.pressed,
                    key.state,
                    keyboard_target_selected.load(Ordering::Acquire),
                );
            }
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
            let write_result = (|| -> Result<(), X11SetupSocketError> {
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
                        X11SetupSocketError::new(format!(
                            "failed to write X11 focus event: {error}"
                        ))
                    })?;
                    focus_sent_to = Some(focused_window);
                }
                stream.write_all(&record).map_err(|error| {
                    X11SetupSocketError::new(format!("failed to write X11 input event: {error}"))
                })?;
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
fn x11_keyboard_delivery_target(
    focused_surface_window: &AtomicU64,
    keyboard_event_window: &AtomicU64,
    keyboard_target_selected: &AtomicBool,
) -> XResourceId {
    XResourceId::new(
        if keyboard_target_selected.load(Ordering::Acquire) {
            keyboard_event_window.load(Ordering::Acquire)
        } else {
            focused_surface_window.load(Ordering::Acquire)
        },
        1,
    )
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
    use std::sync::mpsc::sync_channel;

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
            outcome: XAuthorityControlOutcome::Applied,
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
                delivery: None,
            }
        );
        assert_eq!(channels.control.recv().unwrap(), command);
        let acknowledgement = XAuthorityControlAck {
            transaction: command.transaction(),
            surface: command.surface(),
            outcome: XAuthorityControlOutcome::Applied,
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
                delivery: None,
            })
            .unwrap();
        assert_eq!(
            broker.route_pending(),
            Err(XServerFrontendRouteError::UnknownClient { client })
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
    fn engine_focus_does_not_replace_selected_keyboard_child() {
        let focused_surface = AtomicU64::new(0x200001);
        let keyboard_child = AtomicU64::new(0x200007);
        let selected = AtomicBool::new(true);

        assert_eq!(
            x11_keyboard_delivery_target(&focused_surface, &keyboard_child, &selected),
            XResourceId::new(0x200007, 1)
        );

        focused_surface.store(0x200009, Ordering::Release);
        assert_eq!(
            x11_keyboard_delivery_target(&focused_surface, &keyboard_child, &selected),
            XResourceId::new(0x200007, 1)
        );
    }

    #[test]
    fn keyboard_delivery_falls_back_to_engine_focused_surface() {
        let focused_surface = AtomicU64::new(0x200001);
        let keyboard_child = AtomicU64::new(u64::from(X_SETUP_DEFAULT_ROOT));
        let selected = AtomicBool::new(false);

        assert_eq!(
            x11_keyboard_delivery_target(&focused_surface, &keyboard_child, &selected),
            XResourceId::new(0x200001, 1)
        );
    }
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

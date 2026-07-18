use crate::WmTransactionUpdate;
use crate::prelude::*;
use sophia_protocol::{
    WM_API_VERSION, WM_MAX_BINDINGS, WmActionId, WmCapabilities, WmHello, WmModifierMask,
    WmSessionDescriptor, decode_wm_hello_frame, encode_wm_session_descriptor_frame,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmIpcError {
    Codec(IpcCodecError),
    Io(String),
    TransactionMismatch {
        expected: TransactionId,
        actual: TransactionId,
    },
    Negotiation(&'static str),
}

impl fmt::Display for WmIpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Negotiation(message) => write!(f, "WM negotiation failed: {message}"),
            Self::Codec(error) => write!(f, "codec error: {error:?}"),
            Self::Io(error) => f.write_str(error),
            Self::TransactionMismatch { expected, actual } => write!(
                f,
                "transaction mismatch, expected {}, got {}",
                expected.raw(),
                actual.raw()
            ),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmShortcutRegistry {
    bindings: BTreeMap<(u32, u32), WmActionId>,
    held: BTreeMap<u32, WmActionId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmShortcutDecision {
    pub action: Option<WmActionId>,
    pub consumed: bool,
}

impl WmShortcutRegistry {
    pub fn from_hello(hello: &WmHello) -> Result<Self, WmIpcError> {
        if hello.api_version != WM_API_VERSION {
            return Err(WmIpcError::Negotiation("unsupported WM API version"));
        }
        if hello.capabilities.bits & !WmCapabilities::SUPPORTED != 0 {
            return Err(WmIpcError::Negotiation("unsupported WM capability"));
        }
        if hello.bindings.len() > WM_MAX_BINDINGS {
            return Err(WmIpcError::Negotiation("too many WM bindings"));
        }

        let mut bindings = BTreeMap::new();
        let mut actions = BTreeSet::new();
        for binding in &hello.bindings {
            if !binding.action.is_valid() || binding.keycode == 0 || binding.keycode > 0x2ff {
                return Err(WmIpcError::Negotiation("invalid WM binding"));
            }
            if binding.modifiers.bits & !WmModifierMask::SUPPORTED != 0 {
                return Err(WmIpcError::Negotiation("unsupported WM modifier"));
            }
            if binding.keycode == 14
                && binding.modifiers.bits & (WmModifierMask::CONTROL | WmModifierMask::ALT)
                    == WmModifierMask::CONTROL | WmModifierMask::ALT
            {
                return Err(WmIpcError::Negotiation("reserved emergency chord"));
            }
            if !actions.insert(binding.action) {
                return Err(WmIpcError::Negotiation("duplicate WM action"));
            }
            if bindings
                .insert((binding.keycode, binding.modifiers.bits), binding.action)
                .is_some()
            {
                return Err(WmIpcError::Negotiation("duplicate WM chord"));
            }
        }

        Ok(Self {
            bindings,
            held: BTreeMap::new(),
        })
    }

    pub fn handle_key(
        &mut self,
        keycode: u32,
        modifiers: WmModifierMask,
        pressed: bool,
    ) -> WmShortcutDecision {
        if !pressed {
            return WmShortcutDecision {
                action: None,
                consumed: self.held.remove(&keycode).is_some(),
            };
        }
        let Some(action) = self.bindings.get(&(keycode, modifiers.bits)).copied() else {
            return WmShortcutDecision {
                action: None,
                consumed: false,
            };
        };
        let first_press = self.held.insert(keycode, action).is_none();
        WmShortcutDecision {
            action: first_press.then_some(action),
            consumed: true,
        }
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
}

impl std::error::Error for WmIpcError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRuntimeAction {
    KeepRunning,
    RestartWm { reason: WmRestartReason },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmRestartReason {
    IpcFailure(WmIpcError),
}

impl WmTransactionUpdate {
    pub fn runtime_action(&self) -> WmRuntimeAction {
        match &self.ipc_error {
            Some(error) => WmRuntimeAction::RestartWm {
                reason: WmRestartReason::IpcFailure(error.clone()),
            },
            None => WmRuntimeAction::KeepRunning,
        }
    }
}

pub fn update_wm_supervisor_from_runtime_action(
    state: SupervisorState,
    action: WmRuntimeAction,
    policy: RestartPolicy,
) -> (SupervisorState, SupervisorCommand) {
    debug_assert_eq!(state.process, SupervisedProcessKind::WindowManager);

    match action {
        WmRuntimeAction::KeepRunning => {
            debug!(
                process = ?state.process,
                running = state.running,
                restart_attempts = state.restart_attempts,
                "WM runtime action keeps supervisor state"
            );
            (state, SupervisorCommand::None)
        }
        WmRuntimeAction::RestartWm { .. } => {
            warn!(
                process = ?state.process,
                running = state.running,
                restart_attempts = state.restart_attempts,
                "WM runtime action requests supervisor restart"
            );
            update_supervisor(state, SupervisorEvent::RestartRequested, policy)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmSocketTransportConfig {
    pub response_timeout: Duration,
}

impl Default for WmSocketTransportConfig {
    fn default() -> Self {
        Self {
            response_timeout: Duration::from_millis(250),
        }
    }
}

#[cfg(unix)]
pub struct WmSocketTransport {
    stream: UnixStream,
    config: WmSocketTransportConfig,
}

#[cfg(unix)]
impl WmSocketTransport {
    pub fn new(stream: UnixStream, config: WmSocketTransportConfig) -> Self {
        Self { stream, config }
    }

    pub fn negotiate(
        &mut self,
        descriptor: &WmSessionDescriptor,
    ) -> Result<WmShortcutRegistry, WmIpcError> {
        self.stream
            .set_read_timeout(Some(self.config.response_timeout))
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        let frame = read_ipc_frame(&mut self.stream)?;
        let hello = decode_wm_hello_frame(&frame).map_err(WmIpcError::Codec)?;
        let registry = WmShortcutRegistry::from_hello(&hello)?;
        let frame = encode_wm_session_descriptor_frame(descriptor).map_err(WmIpcError::Codec)?;
        self.stream
            .write_all(&frame)
            .and_then(|()| self.stream.flush())
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        Ok(registry)
    }

    pub fn request(&mut self, request: &WmRequestPacket) -> Result<WmResponsePacket, WmIpcError> {
        self.stream
            .set_read_timeout(Some(self.config.response_timeout))
            .map_err(|error| WmIpcError::Io(error.to_string()))?;
        request_wm_over_stream(&mut self.stream, request)
    }
}

pub fn request_wm_over_stream<S>(
    stream: &mut S,
    request: &WmRequestPacket,
) -> Result<WmResponsePacket, WmIpcError>
where
    S: Read + Write,
{
    let frame = encode_wm_request_frame(request).map_err(WmIpcError::Codec)?;
    debug!(
        transaction = request.transaction.raw(),
        request_bytes = frame.len(),
        "sending WM request frame"
    );
    stream
        .write_all(&frame)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    stream
        .flush()
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    let response = read_wm_response_frame(stream)?;
    if response.transaction != request.transaction {
        warn!(
            expected_transaction = request.transaction.raw(),
            actual_transaction = response.transaction.raw(),
            "rejected WM response with mismatched transaction"
        );
        return Err(WmIpcError::TransactionMismatch {
            expected: request.transaction,
            actual: response.transaction,
        });
    }
    debug!(
        transaction = response.transaction.raw(),
        response_commands = response.commands.len(),
        "received WM response frame"
    );

    Ok(response)
}

pub fn read_wm_response_frame<R>(reader: &mut R) -> Result<WmResponsePacket, WmIpcError>
where
    R: Read,
{
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        warn!(
            payload_len,
            max_payload_len = SOPHIA_IPC_MAX_PAYLOAD_LEN,
            "rejected oversized WM response frame"
        );
        return Err(WmIpcError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    reader
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| WmIpcError::Io(error.to_string()))?;

    decode_wm_response_frame(&frame).map_err(WmIpcError::Codec)
}

pub fn read_ipc_frame<R>(reader: &mut R) -> Result<Vec<u8>, WmIpcError>
where
    R: Read,
{
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(WmIpcError::Codec(IpcCodecError::PayloadTooLarge(
            payload_len,
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    reader
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| WmIpcError::Io(error.to_string()))?;
    Ok(frame)
}

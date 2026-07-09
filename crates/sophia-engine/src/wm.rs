use crate::WmTransactionUpdate;
use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmIpcError {
    Codec(IpcCodecError),
    Io(String),
    TransactionMismatch {
        expected: TransactionId,
        actual: TransactionId,
    },
}

impl fmt::Display for WmIpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

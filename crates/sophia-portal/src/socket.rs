#[cfg(unix)]
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::unix::{
    fs::PermissionsExt,
    net::{UnixListener, UnixStream},
};
#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
use sophia_protocol::{
    PortalBrokerRequestPacket, PortalBrokerResponseDecision, PortalBrokerResponsePacket,
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, decode_portal_broker_request_frame,
    decode_portal_broker_response_frame, encode_portal_broker_request_frame,
    encode_portal_broker_response_frame,
};

#[cfg(unix)]
use crate::{
    DeterministicPortalBroker, HeadlessPortalPolicy, PortalBrokerDecision,
    PortalCapabilityAdmission,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortalBrokerSocketError(String);

impl core::fmt::Display for PortalBrokerSocketError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PortalBrokerSocketError {}

#[cfg(unix)]
pub fn run_portal_broker_socket_server_once(
    path: impl AsRef<Path>,
    broker_generation: u64,
    policy: HeadlessPortalPolicy,
    now_msec: u64,
) -> Result<(), PortalBrokerSocketError> {
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(socket_error("remove stale socket", error)),
    }
    let listener = UnixListener::bind(path).map_err(|error| socket_error("bind socket", error))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| socket_error("restrict socket", error))?;
    let result = (|| {
        let (mut stream, _) = listener
            .accept()
            .map_err(|error| socket_error("accept client", error))?;
        let request = decode_portal_broker_request_frame(&read_frame(&mut stream)?)
            .map_err(|error| PortalBrokerSocketError(format!("decode request: {error:?}")))?;
        let mut broker = DeterministicPortalBroker::new(broker_generation, policy)
            .map_err(|error| PortalBrokerSocketError(format!("create broker: {error:?}")))?;
        let transfer = request.request.transfer.transfer;
        let decision = broker
            .request(
                request.request,
                PortalCapabilityAdmission {
                    source_may_publish: request.source_may_publish,
                    target_may_request: request.target_may_request,
                },
                now_msec,
            )
            .map_err(|error| PortalBrokerSocketError(format!("evaluate request: {error:?}")))?;
        let response = PortalBrokerResponsePacket {
            transfer,
            decision: match decision {
                PortalBrokerDecision::Allowed(grant) => {
                    PortalBrokerResponseDecision::Allowed(grant)
                }
                PortalBrokerDecision::Denied => PortalBrokerResponseDecision::Denied,
            },
        };
        let frame = encode_portal_broker_response_frame(&response)
            .map_err(|error| PortalBrokerSocketError(format!("encode response: {error:?}")))?;
        stream
            .write_all(&frame)
            .and_then(|()| stream.flush())
            .map_err(|error| socket_error("write response", error))
    })();
    let _ = std::fs::remove_file(path);
    result
}

#[cfg(unix)]
pub fn request_portal_broker(
    path: impl AsRef<Path>,
    request: &PortalBrokerRequestPacket,
) -> Result<PortalBrokerResponsePacket, PortalBrokerSocketError> {
    let mut stream =
        UnixStream::connect(path.as_ref()).map_err(|error| socket_error("connect", error))?;
    let frame = encode_portal_broker_request_frame(request)
        .map_err(|error| PortalBrokerSocketError(format!("encode request: {error:?}")))?;
    stream
        .write_all(&frame)
        .and_then(|()| stream.flush())
        .map_err(|error| socket_error("write request", error))?;
    decode_portal_broker_response_frame(&read_frame(&mut stream)?)
        .map_err(|error| PortalBrokerSocketError(format!("decode response: {error:?}")))
}

#[cfg(unix)]
fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, PortalBrokerSocketError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream
        .read_exact(&mut header)
        .map_err(|error| socket_error("read header", error))?;
    let payload_len = u32::from_le_bytes(header[16..20].try_into().expect("fixed header")) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(PortalBrokerSocketError(
            "portal broker payload exceeds bound".to_owned(),
        ));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| socket_error("read payload", error))?;
    Ok(frame)
}

#[cfg(unix)]
fn socket_error(action: &str, error: std::io::Error) -> PortalBrokerSocketError {
    PortalBrokerSocketError(format!("portal broker failed to {action}: {error}"))
}

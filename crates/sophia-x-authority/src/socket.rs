#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
};

#[cfg(unix)]
use sophia_protocol::{SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN};

#[cfg(unix)]
use crate::{
    XAuthorityRequestPacket, XAuthorityResponsePacket, XAuthorityRuntime,
    decode_x_authority_request_frame, decode_x_authority_response_frame,
    encode_x_authority_request_frame, encode_x_authority_response_frame,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAuthoritySocketError {
    message: String,
}

impl XAuthoritySocketError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl core::fmt::Display for XAuthoritySocketError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for XAuthoritySocketError {}

#[cfg(unix)]
pub fn run_x_authority_socket_server(path: impl AsRef<Path>) -> Result<(), XAuthoritySocketError> {
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(XAuthoritySocketError::new(format!(
                "failed to remove stale X authority socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        XAuthoritySocketError::new(format!(
            "failed to bind X authority socket {}: {error}",
            path.display()
        ))
    })?;
    let mut runtime = XAuthorityRuntime::new();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => serve_x_authority_socket_client(&mut stream, &mut runtime)?,
            Err(error) => {
                return Err(XAuthoritySocketError::new(format!(
                    "failed to accept X authority socket client on {}: {error}",
                    path.display()
                )));
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
pub fn run_x_authority_socket_server_once(
    path: impl AsRef<Path>,
) -> Result<(), XAuthoritySocketError> {
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(XAuthoritySocketError::new(format!(
                "failed to remove stale X authority socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        XAuthoritySocketError::new(format!(
            "failed to bind X authority socket {}: {error}",
            path.display()
        ))
    })?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        XAuthoritySocketError::new(format!(
            "failed to accept X authority socket client on {}: {error}",
            path.display()
        ))
    })?;
    let mut runtime = XAuthorityRuntime::new();
    serve_x_authority_socket_client(&mut stream, &mut runtime)
}

#[cfg(unix)]
pub fn request_x_authority(
    path: impl AsRef<Path>,
    request: XAuthorityRequestPacket,
) -> Result<XAuthorityResponsePacket, XAuthoritySocketError> {
    let path = path.as_ref();
    let mut stream = UnixStream::connect(path).map_err(|error| {
        XAuthoritySocketError::new(format!(
            "failed to connect X authority socket {}: {error}",
            path.display()
        ))
    })?;
    write_x_authority_request(&mut stream, &request)?;
    read_x_authority_response(&mut stream)
}

#[cfg(unix)]
pub fn write_x_authority_request(
    stream: &mut UnixStream,
    request: &XAuthorityRequestPacket,
) -> Result<(), XAuthoritySocketError> {
    let frame = encode_x_authority_request_frame(request).map_err(|error| {
        XAuthoritySocketError::new(format!("failed to encode X authority request: {error:?}"))
    })?;
    stream.write_all(&frame).map_err(|error| {
        XAuthoritySocketError::new(format!("failed to write X authority request: {error}"))
    })?;
    stream.flush().map_err(|error| {
        XAuthoritySocketError::new(format!("failed to flush X authority request: {error}"))
    })
}

#[cfg(unix)]
pub fn read_x_authority_response(
    stream: &mut UnixStream,
) -> Result<XAuthorityResponsePacket, XAuthoritySocketError> {
    let frame = read_frame(stream, "response")?;
    decode_x_authority_response_frame(&frame).map_err(|error| {
        XAuthoritySocketError::new(format!("failed to decode X authority response: {error:?}"))
    })
}

#[cfg(unix)]
fn serve_x_authority_socket_client(
    stream: &mut UnixStream,
    runtime: &mut XAuthorityRuntime,
) -> Result<(), XAuthoritySocketError> {
    while let Some(request) = read_x_authority_request(stream)? {
        let response = runtime.apply(request);
        let frame = encode_x_authority_response_frame(&response).map_err(|error| {
            XAuthoritySocketError::new(format!("failed to encode X authority response: {error:?}"))
        })?;
        stream.write_all(&frame).map_err(|error| {
            XAuthoritySocketError::new(format!("failed to write X authority response: {error}"))
        })?;
        stream.flush().map_err(|error| {
            XAuthoritySocketError::new(format!("failed to flush X authority response: {error}"))
        })?;
    }

    Ok(())
}

#[cfg(unix)]
fn read_x_authority_request(
    stream: &mut UnixStream,
) -> Result<Option<XAuthorityRequestPacket>, XAuthoritySocketError> {
    let Some(frame) = read_optional_frame(stream, "request")? else {
        return Ok(None);
    };
    decode_x_authority_request_frame(&frame)
        .map(Some)
        .map_err(|error| {
            XAuthoritySocketError::new(format!("failed to decode X authority request: {error:?}"))
        })
}

#[cfg(unix)]
fn read_frame(
    stream: &mut UnixStream,
    label: &'static str,
) -> Result<Vec<u8>, XAuthoritySocketError> {
    read_optional_frame(stream, label)?.ok_or_else(|| {
        XAuthoritySocketError::new(format!("unexpected EOF reading X authority {label}"))
    })
}

#[cfg(unix)]
fn read_optional_frame(
    stream: &mut UnixStream,
    label: &'static str,
) -> Result<Option<Vec<u8>>, XAuthoritySocketError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(XAuthoritySocketError::new(format!(
                "failed to read X authority {label} header: {error}"
            )));
        }
    }

    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(XAuthoritySocketError::new(format!(
            "X authority {label} payload too large: {payload_len}"
        )));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| {
            XAuthoritySocketError::new(format!(
                "failed to read X authority {label} payload: {error}"
            ))
        })?;

    Ok(Some(frame))
}

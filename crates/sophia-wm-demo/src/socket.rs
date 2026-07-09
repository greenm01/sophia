#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
};

#[cfg(unix)]
use sophia_protocol::{
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, WmRequestPacket, decode_wm_request_frame,
    encode_wm_response_frame,
};

#[cfg(unix)]
use crate::{WmProcessError, handle_wm_request};

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

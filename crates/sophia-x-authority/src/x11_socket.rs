#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
};

#[cfg(unix)]
use crate::{
    X_SETUP_CLIENT_PREFIX_LEN, X_SETUP_MAX_AUTH_FIELD_LEN, XAtomTable, XAuthorityRuntime,
    XDispatchContext, XPropertyTable, XSetupRequest, XSetupSuccess, XWireClientContext,
    decode_x11_core_request, dispatch_x11_parse_error, dispatch_x11_wire_request,
    encode_x11_setup_success, parse_x11_setup_request, x11_setup_request_total_len,
};
#[cfg(unix)]
use sophia_protocol::{NamespaceId, TransactionId};

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

    let listener = UnixListener::bind(path).map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to bind X11 core socket {}: {error}",
            path.display()
        ))
    })?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        X11SetupSocketError::new(format!(
            "failed to accept X11 core client on {}: {error}",
            path.display()
        ))
    })?;
    serve_x11_core_socket_client(&mut stream, namespace)
}

#[cfg(unix)]
pub fn serve_x11_setup_socket_client(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let request = read_x11_setup_request(stream)?;
    let response = encode_x11_setup_success(request.byte_order, &XSetupSuccess::minimal())
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to encode X11 setup success: {error}"))
        })?;
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
    let setup = serve_x11_setup_socket_client(stream)?;
    let mut runtime = XAuthorityRuntime::new();
    let mut atoms = XAtomTable::new();
    let mut properties = XPropertyTable::new();
    let mut sequence = 0u16;

    while let Some((major_opcode, request)) = read_x11_core_request(stream, setup.byte_order)? {
        sequence = sequence.wrapping_add(1);
        let dispatch_context = XDispatchContext {
            byte_order: setup.byte_order,
            namespace,
            sequence,
            major_opcode,
        };
        let output = match decode_x11_core_request(
            XWireClientContext {
                byte_order: setup.byte_order,
                namespace,
                transaction: TransactionId::from_raw(u64::from(sequence)),
            },
            &request,
        ) {
            Ok(request) => dispatch_x11_wire_request(
                dispatch_context,
                request,
                &mut runtime,
                &mut atoms,
                &mut properties,
            ),
            Err(error) => dispatch_x11_parse_error(dispatch_context, error),
        };
        for record in output.encoded_outputs(setup.byte_order) {
            stream.write_all(&record).map_err(|error| {
                X11SetupSocketError::new(format!("failed to write X11 output: {error}"))
            })?;
        }
        stream.flush().map_err(|error| {
            X11SetupSocketError::new(format!("failed to flush X11 output: {error}"))
        })?;
    }

    Ok(())
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
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
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

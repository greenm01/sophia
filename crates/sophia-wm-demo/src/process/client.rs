use std::{
    path::{Path, PathBuf},
    process::Command,
};

use sophia_protocol::{WmRequestPacket, WmResponsePacket};

use crate::handle_wm_request;

use super::{
    error::WmProcessError, request::parse_process_request, request::request_to_process_args,
    response::decode_process_response, response::encode_process_response,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalWmClient {
    program: PathBuf,
}

impl ExternalWmClient {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn request(&self, request: &WmRequestPacket) -> Result<WmResponsePacket, WmProcessError> {
        let output = Command::new(&self.program)
            .args(request_to_process_args(request))
            .output()
            .map_err(|error| {
                WmProcessError::new(format!(
                    "failed to spawn external WM {}: {error}",
                    self.program.display()
                ))
            })?;

        if !output.status.success() {
            return Err(WmProcessError::new(format!(
                "external WM {} exited with status {}: {}",
                self.program.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let stdout = String::from_utf8(output.stdout).map_err(|error| {
            WmProcessError::new(format!("external WM emitted non-UTF8: {error}"))
        })?;
        decode_process_response(stdout.trim())
    }
}

pub fn run_process_request(args: &[String]) -> Result<String, WmProcessError> {
    if args.first().map(String::as_str) == Some("serve-socket") {
        return Err(WmProcessError::new(
            "serve-socket is long-running; call run_socket_server instead",
        ));
    }

    let request = parse_process_request(args)?;
    let response = handle_wm_request(request);
    Ok(format!("{}\n", encode_process_response(&response)))
}

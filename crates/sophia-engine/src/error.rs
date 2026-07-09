use crate::WmIpcError;
use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    InvalidOutput,
    InvalidSurface,
    InvalidFrame,
    WmIpc(WmIpcError),
    RuntimeObservation(SessionRuntimeObservationError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput => f.write_str("invalid output ID"),
            Self::InvalidSurface => f.write_str("invalid surface ID"),
            Self::InvalidFrame => f.write_str("invalid frame snapshot"),
            Self::WmIpc(error) => write!(f, "WM IPC failed: {error}"),
            Self::RuntimeObservation(error) => write!(f, "runtime observation failed: {error}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl SophiaErrorExt for EngineError {
    fn kind(&self) -> SophiaErrorKind {
        match self {
            Self::InvalidOutput => SophiaErrorKind::InvalidOutput,
            Self::InvalidSurface => SophiaErrorKind::InvalidSurface,
            Self::InvalidFrame => SophiaErrorKind::InvalidFrame,
            Self::WmIpc(_) => SophiaErrorKind::ExternalProcess,
            Self::RuntimeObservation(_) => SophiaErrorKind::InvalidFrame,
        }
    }
}

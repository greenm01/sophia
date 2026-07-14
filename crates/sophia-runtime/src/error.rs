use crate::prelude::*;

pub type SophiaResult<T, E = SophiaError> = Result<T, E>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SophiaErrorKind {
    InvalidOutput,
    InvalidSurface,
    InvalidFrame,
    InvalidNamespace,
    ExternalProcess,
    RuntimeAlreadyInitialized,
}

pub trait SophiaErrorExt {
    fn kind(&self) -> SophiaErrorKind;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SophiaError {
    kind: SophiaErrorKind,
    message: String,
}

impl SophiaError {
    pub fn new(kind: SophiaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl SophiaErrorExt for SophiaError {
    fn kind(&self) -> SophiaErrorKind {
        self.kind
    }
}

impl fmt::Display for SophiaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SophiaError {}

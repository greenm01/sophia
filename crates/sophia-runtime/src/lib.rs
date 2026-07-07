//! Shared runtime conventions for Sophia processes.
//!
//! Libraries emit structured diagnostics through `tracing`; binaries decide
//! when and how to install a subscriber.

use core::fmt;

use tracing_subscriber::EnvFilter;

pub type SophiaResult<T, E = SophiaError> = Result<T, E>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SophiaErrorKind {
    InvalidOutput,
    InvalidSurface,
    InvalidFrame,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceLevel {
    Info,
    Debug,
    Trace,
}

impl TraceLevel {
    const fn filter(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TracingInitError {
    AlreadyInitialized,
}

impl SophiaErrorExt for TracingInitError {
    fn kind(&self) -> SophiaErrorKind {
        SophiaErrorKind::RuntimeAlreadyInitialized
    }
}

impl fmt::Display for TracingInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInitialized => f.write_str("tracing subscriber is already initialized"),
        }
    }
}

impl std::error::Error for TracingInitError {}

pub fn init_tracing(level: TraceLevel) -> Result<(), TracingInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.filter()));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|_| TracingInitError::AlreadyInitialized)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLibdrmPollerDiagnostics {
    pub status: LiveLibdrmPollerDiagnosticsStatus,
    pub route_count: usize,
    pub pending_callbacks: usize,
    pub decoded_callbacks: usize,
    pub rejected_callbacks: usize,
}

impl LiveLibdrmPollerDiagnostics {
    pub const fn not_configured() -> Self {
        Self {
            status: LiveLibdrmPollerDiagnosticsStatus::NotConfigured,
            route_count: 0,
            pending_callbacks: 0,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }
}

impl Default for LiveLibdrmPollerDiagnostics {
    fn default() -> Self {
        Self::not_configured()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveLibdrmPollerDiagnosticsStatus {
    NotConfigured,
    Idle,
    WouldBlock,
    CallbackDecoded,
    CallbackRejected,
    ReadFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveLibdrmPollerStartupReport {
    pub status: LiveLibdrmPollerStartupStatus,
    pub route_count: usize,
}

impl LiveLibdrmPollerStartupReport {
    pub const fn not_configured() -> Self {
        Self {
            status: LiveLibdrmPollerStartupStatus::NotConfigured,
            route_count: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveLibdrmPollerStartupStatus {
    NotConfigured,
    Ready,
    NoOutputs,
    BackendNotReady,
}

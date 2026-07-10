pub const LIVE_PAGE_FLIP_CALLBACK_CHANNEL_CAPACITY: usize = 128;
pub const SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE: &str = "SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveHardwareValidationGateReport {
    pub target: LiveHardwareValidationTarget,
    pub status: LiveHardwareValidationGateStatus,
}

impl LiveHardwareValidationGateReport {
    pub const fn from_env_presence(target: LiveHardwareValidationTarget, present: bool) -> Self {
        Self {
            target,
            status: if present {
                LiveHardwareValidationGateStatus::Requested
            } else {
                LiveHardwareValidationGateStatus::SkippedOptInRequired
            },
        }
    }

    pub const fn is_requested(self) -> bool {
        matches!(self.status, LiveHardwareValidationGateStatus::Requested)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHardwareValidationTarget {
    LibdrmEvents,
    LibinputEvents,
    AtomicScanout,
}

impl LiveHardwareValidationTarget {
    pub const fn env_var(self) -> &'static str {
        match self {
            Self::LibdrmEvents => SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE,
            Self::LibinputEvents => SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE,
            Self::AtomicScanout => SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHardwareValidationGateStatus {
    SkippedOptInRequired,
    Requested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveHardwareValidationSmokeReport {
    pub target: LiveHardwareValidationTarget,
    pub status: LiveHardwareValidationSmokeStatus,
}

impl LiveHardwareValidationSmokeReport {
    pub const fn fail_closed_from_gate(gate: LiveHardwareValidationGateReport) -> Self {
        Self {
            target: gate.target,
            status: if gate.is_requested() {
                LiveHardwareValidationSmokeStatus::BackendUnavailable
            } else {
                LiveHardwareValidationSmokeStatus::SkippedOptInRequired
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveHardwareValidationSmokeStatus {
    SkippedOptInRequired,
    BackendUnavailable,
    Passed,
    Failed,
}

pub fn real_libdrm_events_validation_gate() -> LiveHardwareValidationGateReport {
    let target = LiveHardwareValidationTarget::LibdrmEvents;
    LiveHardwareValidationGateReport::from_env_presence(
        target,
        std::env::var_os(target.env_var()).is_some(),
    )
}

pub fn real_libinput_events_validation_gate() -> LiveHardwareValidationGateReport {
    let target = LiveHardwareValidationTarget::LibinputEvents;
    LiveHardwareValidationGateReport::from_env_presence(
        target,
        std::env::var_os(target.env_var()).is_some(),
    )
}

pub fn real_atomic_scanout_validation_gate() -> LiveHardwareValidationGateReport {
    let target = LiveHardwareValidationTarget::AtomicScanout;
    LiveHardwareValidationGateReport::from_env_presence(
        target,
        std::env::var_os(target.env_var()).is_some(),
    )
}

pub fn real_libdrm_events_validation_smoke_report() -> LiveHardwareValidationSmokeReport {
    LiveHardwareValidationSmokeReport::fail_closed_from_gate(real_libdrm_events_validation_gate())
}

pub fn real_libinput_events_validation_smoke_report() -> LiveHardwareValidationSmokeReport {
    LiveHardwareValidationSmokeReport::fail_closed_from_gate(real_libinput_events_validation_gate())
}

pub fn real_atomic_scanout_validation_smoke_report() -> LiveHardwareValidationSmokeReport {
    LiveHardwareValidationSmokeReport::fail_closed_from_gate(real_atomic_scanout_validation_gate())
}

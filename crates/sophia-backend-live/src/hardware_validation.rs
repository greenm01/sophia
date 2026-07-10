pub const LIVE_PAGE_FLIP_CALLBACK_CHANNEL_CAPACITY: usize = 128;
pub const SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE: &str = "SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE";
pub const SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE: &str = "SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE";
pub const LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS: u8 = 8;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveAtomicScanoutPreflightReport {
    pub target: LiveHardwareValidationTarget,
    pub status: LiveAtomicScanoutPreflightStatus,
    pub primary_card_nodes: u8,
    pub openable_primary_card_nodes: u8,
    pub atomic_capable_primary_card_nodes: u8,
}

impl LiveAtomicScanoutPreflightReport {
    pub const fn from_primary_card_counts(
        device_directory_available: bool,
        primary_card_nodes: usize,
        openable_primary_card_nodes: usize,
        atomic_capable_primary_card_nodes: usize,
    ) -> Self {
        let primary_card_nodes = if device_directory_available {
            capped_primary_card_count(primary_card_nodes)
        } else {
            0
        };
        let openable_primary_card_nodes = if device_directory_available {
            capped_primary_card_count(openable_primary_card_nodes)
        } else {
            0
        };
        let atomic_capable_primary_card_nodes = if device_directory_available {
            capped_primary_card_count(atomic_capable_primary_card_nodes)
        } else {
            0
        };
        let atomic_capable_primary_card_nodes = capped_atomic_capable_primary_card_count(
            atomic_capable_primary_card_nodes,
            openable_primary_card_nodes,
        );
        let status = if !device_directory_available {
            LiveAtomicScanoutPreflightStatus::DeviceDirectoryUnavailable
        } else if primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::NoPrimaryCardNodes
        } else if openable_primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::PrimaryCardOpenUnavailable
        } else if atomic_capable_primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::AtomicClientCapabilityUnavailable
        } else {
            LiveAtomicScanoutPreflightStatus::CandidatePrimaryCardsAtomicCapable
        };

        Self {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status,
            primary_card_nodes,
            openable_primary_card_nodes,
            atomic_capable_primary_card_nodes,
        }
    }

    pub fn reduced_log_line(self) -> String {
        format!(
            "sophia_atomic_scanout_preflight schema=3 target={:?} status={:?} primary_card_nodes={} openable_primary_card_nodes={} atomic_capable_primary_card_nodes={}",
            self.target,
            self.status,
            self.primary_card_nodes,
            self.openable_primary_card_nodes,
            self.atomic_capable_primary_card_nodes
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveAtomicScanoutPreflightStatus {
    DeviceDirectoryUnavailable,
    NoPrimaryCardNodes,
    PrimaryCardOpenUnavailable,
    AtomicClientCapabilityUnavailable,
    CandidatePrimaryCardsAtomicCapable,
}

pub const fn capped_primary_card_count(primary_card_nodes: usize) -> u8 {
    if primary_card_nodes > LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS as usize {
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    } else {
        primary_card_nodes as u8
    }
}

pub const fn capped_atomic_capable_primary_card_count(
    atomic_capable_primary_card_nodes: u8,
    openable_primary_card_nodes: u8,
) -> u8 {
    if atomic_capable_primary_card_nodes > openable_primary_card_nodes {
        openable_primary_card_nodes
    } else {
        atomic_capable_primary_card_nodes
    }
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

pub fn real_atomic_scanout_preflight_report() -> LiveAtomicScanoutPreflightReport {
    real_atomic_scanout_preflight_report_from_dev_dri(std::path::Path::new("/dev/dri"))
}

pub fn real_atomic_scanout_preflight_report_from_dev_dri(
    dev_dri: &std::path::Path,
) -> LiveAtomicScanoutPreflightReport {
    let Ok(entries) = std::fs::read_dir(dev_dri) else {
        return LiveAtomicScanoutPreflightReport::from_primary_card_counts(false, 0, 0, 0);
    };

    let mut primary_card_nodes = 0usize;
    let mut openable_primary_card_nodes = 0usize;
    let mut atomic_capable_primary_card_nodes = 0usize;

    for entry in entries
        .filter_map(Result::ok)
        .filter(is_primary_card_node_entry)
    {
        primary_card_nodes = primary_card_nodes.saturating_add(1);
        if primary_card_nodes > LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS as usize
            && openable_primary_card_nodes
                > LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS as usize
            && atomic_capable_primary_card_nodes
                > LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS as usize
        {
            continue;
        }
        let path = entry.path();
        if can_open_primary_card_node_read_write(&path) {
            openable_primary_card_nodes = openable_primary_card_nodes.saturating_add(1);
        }
        if can_admit_atomic_scanout_client_capabilities(&path) {
            atomic_capable_primary_card_nodes = atomic_capable_primary_card_nodes.saturating_add(1);
        }
    }

    LiveAtomicScanoutPreflightReport::from_primary_card_counts(
        true,
        primary_card_nodes,
        openable_primary_card_nodes,
        atomic_capable_primary_card_nodes,
    )
}

fn is_primary_card_node_entry(entry: &std::fs::DirEntry) -> bool {
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
        return false;
    };
    if !is_primary_card_node_name(name) {
        return false;
    }
    entry
        .file_type()
        .map(|file_type| is_drm_card_node_file_type(&file_type))
        .unwrap_or(false)
}

fn can_open_primary_card_node_read_write(path: &std::path::Path) -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .is_ok()
}

#[cfg(feature = "libdrm-events")]
fn can_admit_atomic_scanout_client_capabilities(path: &std::path::Path) -> bool {
    #[derive(Debug)]
    struct AtomicPreflightCard(std::fs::File);

    impl std::os::fd::AsFd for AtomicPreflightCard {
        fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
            self.0.as_fd()
        }
    }

    impl drm::Device for AtomicPreflightCard {}

    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    else {
        return false;
    };
    let card = AtomicPreflightCard(file);

    drm::Device::set_client_capability(&card, drm::ClientCapability::UniversalPlanes, true).is_ok()
        && drm::Device::set_client_capability(&card, drm::ClientCapability::Atomic, true).is_ok()
}

#[cfg(not(feature = "libdrm-events"))]
fn can_admit_atomic_scanout_client_capabilities(_path: &std::path::Path) -> bool {
    false
}

fn is_primary_card_node_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("card") else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(unix)]
fn is_drm_card_node_file_type(file_type: &std::fs::FileType) -> bool {
    use std::os::unix::fs::FileTypeExt;

    file_type.is_char_device()
}

#[cfg(not(unix))]
fn is_drm_card_node_file_type(_file_type: &std::fs::FileType) -> bool {
    false
}

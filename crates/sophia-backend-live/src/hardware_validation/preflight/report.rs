use super::super::LiveHardwareValidationTarget;
use super::counts::{
    capped_atomic_capable_primary_card_count, capped_atomic_property_primary_card_count,
    capped_primary_card_count, capped_scanout_target_primary_card_count,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveAtomicScanoutPreflightReport {
    pub target: LiveHardwareValidationTarget,
    pub status: LiveAtomicScanoutPreflightStatus,
    pub primary_card_nodes: u8,
    pub openable_primary_card_nodes: u8,
    pub atomic_capable_primary_card_nodes: u8,
    pub scanout_target_primary_card_nodes: u8,
    pub atomic_property_primary_card_nodes: u8,
}

impl LiveAtomicScanoutPreflightReport {
    pub const fn from_primary_card_counts(
        device_directory_available: bool,
        primary_card_nodes: usize,
        openable_primary_card_nodes: usize,
        atomic_capable_primary_card_nodes: usize,
        scanout_target_primary_card_nodes: usize,
        atomic_property_primary_card_nodes: usize,
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
        let scanout_target_primary_card_nodes = if device_directory_available {
            capped_primary_card_count(scanout_target_primary_card_nodes)
        } else {
            0
        };
        let scanout_target_primary_card_nodes = capped_scanout_target_primary_card_count(
            scanout_target_primary_card_nodes,
            atomic_capable_primary_card_nodes,
        );
        let atomic_property_primary_card_nodes = if device_directory_available {
            capped_primary_card_count(atomic_property_primary_card_nodes)
        } else {
            0
        };
        let atomic_property_primary_card_nodes = capped_atomic_property_primary_card_count(
            atomic_property_primary_card_nodes,
            scanout_target_primary_card_nodes,
        );
        let status = if !device_directory_available {
            LiveAtomicScanoutPreflightStatus::DeviceDirectoryUnavailable
        } else if primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::NoPrimaryCardNodes
        } else if openable_primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::PrimaryCardOpenUnavailable
        } else if atomic_capable_primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::AtomicClientCapabilityUnavailable
        } else if scanout_target_primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::KmsScanoutTargetUnavailable
        } else if atomic_property_primary_card_nodes == 0 {
            LiveAtomicScanoutPreflightStatus::AtomicPropertyDiscoveryUnavailable
        } else {
            LiveAtomicScanoutPreflightStatus::CandidatePrimaryCardsAtomicReady
        };

        Self {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status,
            primary_card_nodes,
            openable_primary_card_nodes,
            atomic_capable_primary_card_nodes,
            scanout_target_primary_card_nodes,
            atomic_property_primary_card_nodes,
        }
    }

    pub fn reduced_log_line(self) -> String {
        format!(
            "sophia_atomic_scanout_preflight schema=5 target={:?} status={:?} primary_card_nodes={} openable_primary_card_nodes={} atomic_capable_primary_card_nodes={} scanout_target_primary_card_nodes={} atomic_property_primary_card_nodes={}",
            self.target,
            self.status,
            self.primary_card_nodes,
            self.openable_primary_card_nodes,
            self.atomic_capable_primary_card_nodes,
            self.scanout_target_primary_card_nodes,
            self.atomic_property_primary_card_nodes
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveAtomicScanoutPreflightStatus {
    DeviceDirectoryUnavailable,
    NoPrimaryCardNodes,
    PrimaryCardOpenUnavailable,
    AtomicClientCapabilityUnavailable,
    KmsScanoutTargetUnavailable,
    AtomicPropertyDiscoveryUnavailable,
    CandidatePrimaryCardsAtomicReady,
}

use std::path::Path;

use super::{RealAtomicScanoutCard, inspect_opened_real_atomic_scanout_card};
use crate::hardware_validation::preflight::nodes::is_primary_card_node_entry;
use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutCardSelection {
    pub status: RealAtomicScanoutCardSelectionStatus,
    pub card: Option<RealAtomicScanoutCard>,
    pub selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

impl RealAtomicScanoutCardSelection {
    pub(super) fn failed(status: RealAtomicScanoutCardSelectionStatus) -> Self {
        Self {
            status,
            card: None,
            selection: None,
        }
    }

    pub(super) fn selected(
        card: RealAtomicScanoutCard,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> Self {
        Self {
            status: RealAtomicScanoutCardSelectionStatus::Selected,
            card: Some(card),
            selection: Some(selection),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutCardSelectionStatus {
    Selected,
    DeviceDirectoryUnavailable,
    NoPrimaryCardNodes,
    PrimaryCardOpenUnavailable,
    AtomicClientCapabilityUnavailable,
    KmsScanoutTargetUnavailable,
    AtomicPropertyDiscoveryUnavailable,
}

impl RealAtomicScanoutCardSelectionStatus {
    pub const fn failure_evidence(self) -> LibdrmNativeAtomicScanoutSmokeEvidence {
        match self {
            RealAtomicScanoutCardSelectionStatus::Selected => {
                LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed()
            }
            RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable
            | RealAtomicScanoutCardSelectionStatus::NoPrimaryCardNodes => {
                LibdrmNativeAtomicScanoutSmokeEvidence::no_primary_card()
            }
            RealAtomicScanoutCardSelectionStatus::PrimaryCardOpenUnavailable => {
                LibdrmNativeAtomicScanoutSmokeEvidence::primary_card_open_failed()
            }
            RealAtomicScanoutCardSelectionStatus::AtomicClientCapabilityUnavailable => {
                LibdrmNativeAtomicScanoutSmokeEvidence::client_capability_failed()
            }
            RealAtomicScanoutCardSelectionStatus::KmsScanoutTargetUnavailable => {
                LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed()
            }
            RealAtomicScanoutCardSelectionStatus::AtomicPropertyDiscoveryUnavailable => {
                LibdrmNativeAtomicScanoutSmokeEvidence::property_discovery_failed()
            }
        }
    }
}

pub fn select_real_atomic_scanout_card() -> RealAtomicScanoutCardSelection {
    select_real_atomic_scanout_card_from_dev_dri(Path::new("/dev/dri"))
}

pub fn select_real_atomic_scanout_card_from_dev_dri(
    dev_dri: &Path,
) -> RealAtomicScanoutCardSelection {
    let Ok(entries) = std::fs::read_dir(dev_dri) else {
        return RealAtomicScanoutCardSelection::failed(
            RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable,
        );
    };
    let mut candidates = entries
        .filter_map(Result::ok)
        .filter(is_primary_card_node_entry)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return RealAtomicScanoutCardSelection::failed(
            RealAtomicScanoutCardSelectionStatus::NoPrimaryCardNodes,
        );
    }
    candidates.sort();

    let mut saw_openable = false;
    let mut saw_atomic_capable = false;
    let mut saw_scanout_target = false;

    for path in candidates {
        let Ok(card) = RealAtomicScanoutCard::open_nonblocking(&path) else {
            continue;
        };
        let inspection = inspect_opened_real_atomic_scanout_card(&card);
        saw_openable |= inspection.readiness.openable;
        saw_atomic_capable |= inspection.readiness.atomic_capable;
        saw_scanout_target |= inspection.readiness.scanout_target;

        let Some(selection) = inspection.selection else {
            continue;
        };
        return RealAtomicScanoutCardSelection::selected(card, selection);
    }

    RealAtomicScanoutCardSelection::failed(if !saw_openable {
        RealAtomicScanoutCardSelectionStatus::PrimaryCardOpenUnavailable
    } else if !saw_atomic_capable {
        RealAtomicScanoutCardSelectionStatus::AtomicClientCapabilityUnavailable
    } else if !saw_scanout_target {
        RealAtomicScanoutCardSelectionStatus::KmsScanoutTargetUnavailable
    } else {
        RealAtomicScanoutCardSelectionStatus::AtomicPropertyDiscoveryUnavailable
    })
}

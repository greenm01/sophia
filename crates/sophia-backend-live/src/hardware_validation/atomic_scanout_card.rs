use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::hardware_validation::preflight::nodes::is_primary_card_node_entry;
use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutCard(std::fs::File);

impl RealAtomicScanoutCard {
    fn open_nonblocking(path: &Path) -> io::Result<Self> {
        Ok(Self(
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
                .open(path)?,
        ))
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }

    pub fn try_clone_file(&self) -> io::Result<std::fs::File> {
        self.0.try_clone()
    }
}

impl AsFd for RealAtomicScanoutCard {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for RealAtomicScanoutCard {}
impl drm::control::Device for RealAtomicScanoutCard {}

#[derive(Debug)]
pub struct RealAtomicScanoutCardSelection {
    pub status: RealAtomicScanoutCardSelectionStatus,
    pub card: Option<RealAtomicScanoutCard>,
    pub selection: Option<LibdrmNativePrimaryPlaneSelection>,
}

impl RealAtomicScanoutCardSelection {
    fn failed(status: RealAtomicScanoutCardSelectionStatus) -> Self {
        Self {
            status,
            card: None,
            selection: None,
        }
    }

    fn selected(card: RealAtomicScanoutCard, selection: LibdrmNativePrimaryPlaneSelection) -> Self {
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
        saw_openable = true;

        if !admit_atomic_scanout_client_capabilities(&card) {
            continue;
        }
        saw_atomic_capable = true;

        let target = select_native_primary_plane_target(&card);
        let Some(selection) = target.selection else {
            continue;
        };
        if target.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
            continue;
        }
        saw_scanout_target = true;

        let properties = discover_native_primary_plane_property_handles(
            &card,
            selection.connector,
            selection.crtc,
            selection.plane,
        );
        if properties.status != LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered {
            continue;
        }

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

fn admit_atomic_scanout_client_capabilities(card: &RealAtomicScanoutCard) -> bool {
    drm::Device::set_client_capability(card, drm::ClientCapability::UniversalPlanes, true).is_ok()
        && drm::Device::set_client_capability(card, drm::ClientCapability::Atomic, true).is_ok()
}

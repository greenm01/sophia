use super::counts::LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS;
use super::nodes::is_primary_card_node_entry;
use super::report::LiveAtomicScanoutPreflightReport;

pub fn real_atomic_scanout_preflight_report() -> LiveAtomicScanoutPreflightReport {
    real_atomic_scanout_preflight_report_from_dev_dri(std::path::Path::new("/dev/dri"))
}

pub fn real_atomic_scanout_preflight_report_from_dev_dri(
    dev_dri: &std::path::Path,
) -> LiveAtomicScanoutPreflightReport {
    let Ok(entries) = std::fs::read_dir(dev_dri) else {
        return LiveAtomicScanoutPreflightReport::from_primary_card_counts(false, 0, 0, 0, 0, 0);
    };

    let mut primary_card_nodes = 0usize;
    let mut openable_primary_card_nodes = 0usize;
    let mut atomic_capable_primary_card_nodes = 0usize;
    let mut scanout_target_primary_card_nodes = 0usize;
    let mut atomic_property_primary_card_nodes = 0usize;

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
            && scanout_target_primary_card_nodes
                > LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS as usize
            && atomic_property_primary_card_nodes
                > LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS as usize
        {
            continue;
        }
        let path = entry.path();
        let readiness = inspect_atomic_preflight_card(&path);
        if readiness.openable {
            openable_primary_card_nodes = openable_primary_card_nodes.saturating_add(1);
        }
        if readiness.atomic_capable {
            atomic_capable_primary_card_nodes = atomic_capable_primary_card_nodes.saturating_add(1);
        }
        if readiness.scanout_target {
            scanout_target_primary_card_nodes = scanout_target_primary_card_nodes.saturating_add(1);
        }
        if readiness.atomic_properties {
            atomic_property_primary_card_nodes =
                atomic_property_primary_card_nodes.saturating_add(1);
        }
    }

    LiveAtomicScanoutPreflightReport::from_primary_card_counts(
        true,
        primary_card_nodes,
        openable_primary_card_nodes,
        atomic_capable_primary_card_nodes,
        scanout_target_primary_card_nodes,
        atomic_property_primary_card_nodes,
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AtomicPreflightCardReadiness {
    openable: bool,
    atomic_capable: bool,
    scanout_target: bool,
    atomic_properties: bool,
}

#[cfg(feature = "libdrm-events")]
fn inspect_atomic_preflight_card(path: &std::path::Path) -> AtomicPreflightCardReadiness {
    let Some(card) = open_atomic_preflight_card(path) else {
        return AtomicPreflightCardReadiness::default();
    };

    let mut readiness = AtomicPreflightCardReadiness {
        openable: true,
        ..AtomicPreflightCardReadiness::default()
    };

    if !admit_atomic_scanout_client_capabilities(&card) {
        return readiness;
    }
    readiness.atomic_capable = true;

    let selection = crate::select_native_primary_plane_target(&card);
    if selection.status != crate::LibdrmNativePrimaryPlaneSelectionStatus::Selected {
        return readiness;
    }
    readiness.scanout_target = true;

    let Some(selection) = selection.selection else {
        return readiness;
    };
    let properties = crate::discover_native_primary_plane_property_handles(
        &card,
        selection.connector,
        selection.crtc,
        selection.plane,
    );
    readiness.atomic_properties =
        properties.status == crate::LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered;
    readiness
}

#[cfg(not(feature = "libdrm-events"))]
fn inspect_atomic_preflight_card(path: &std::path::Path) -> AtomicPreflightCardReadiness {
    AtomicPreflightCardReadiness {
        openable: std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .is_ok(),
        ..AtomicPreflightCardReadiness::default()
    }
}

#[cfg(feature = "libdrm-events")]
fn open_atomic_preflight_card(path: &std::path::Path) -> Option<AtomicPreflightCard> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .ok()?;
    Some(AtomicPreflightCard(file))
}

#[cfg(feature = "libdrm-events")]
fn admit_atomic_scanout_client_capabilities(card: &AtomicPreflightCard) -> bool {
    drm::Device::set_client_capability(card, drm::ClientCapability::UniversalPlanes, true).is_ok()
        && drm::Device::set_client_capability(card, drm::ClientCapability::Atomic, true).is_ok()
}

#[cfg(feature = "libdrm-events")]
#[derive(Debug)]
struct AtomicPreflightCard(std::fs::File);

#[cfg(feature = "libdrm-events")]
impl std::os::fd::AsFd for AtomicPreflightCard {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.0.as_fd()
    }
}

#[cfg(feature = "libdrm-events")]
impl drm::Device for AtomicPreflightCard {}

#[cfg(feature = "libdrm-events")]
impl drm::control::Device for AtomicPreflightCard {}

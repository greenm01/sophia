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
        if can_open_primary_card_node_read_write(&path) {
            openable_primary_card_nodes = openable_primary_card_nodes.saturating_add(1);
        }
        if can_admit_atomic_scanout_client_capabilities(&path) {
            atomic_capable_primary_card_nodes = atomic_capable_primary_card_nodes.saturating_add(1);
        }
        if can_select_primary_plane_scanout_target(&path) {
            scanout_target_primary_card_nodes = scanout_target_primary_card_nodes.saturating_add(1);
        }
        if can_discover_primary_plane_atomic_properties(&path) {
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

fn can_open_primary_card_node_read_write(path: &std::path::Path) -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .is_ok()
}

#[cfg(feature = "libdrm-events")]
fn can_admit_atomic_scanout_client_capabilities(path: &std::path::Path) -> bool {
    open_atomic_preflight_card_with_client_capabilities(path).is_some()
}

#[cfg(not(feature = "libdrm-events"))]
fn can_admit_atomic_scanout_client_capabilities(_path: &std::path::Path) -> bool {
    false
}

#[cfg(feature = "libdrm-events")]
fn can_select_primary_plane_scanout_target(path: &std::path::Path) -> bool {
    let Some(card) = open_atomic_preflight_card_with_client_capabilities(path) else {
        return false;
    };

    crate::select_native_primary_plane_target(&card).status
        == crate::LibdrmNativePrimaryPlaneSelectionStatus::Selected
}

#[cfg(not(feature = "libdrm-events"))]
fn can_select_primary_plane_scanout_target(_path: &std::path::Path) -> bool {
    false
}

#[cfg(feature = "libdrm-events")]
fn can_discover_primary_plane_atomic_properties(path: &std::path::Path) -> bool {
    let Some(card) = open_atomic_preflight_card_with_client_capabilities(path) else {
        return false;
    };
    let selection = crate::select_native_primary_plane_target(&card);
    let Some(selection) = selection.selection else {
        return false;
    };

    crate::discover_native_primary_plane_property_handles(
        &card,
        selection.connector,
        selection.crtc,
        selection.plane,
    )
    .status
        == crate::LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered
}

#[cfg(not(feature = "libdrm-events"))]
fn can_discover_primary_plane_atomic_properties(_path: &std::path::Path) -> bool {
    false
}

#[cfg(feature = "libdrm-events")]
fn open_atomic_preflight_card_with_client_capabilities(
    path: &std::path::Path,
) -> Option<AtomicPreflightCard> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .ok()?;
    let card = AtomicPreflightCard(file);
    drm::Device::set_client_capability(&card, drm::ClientCapability::UniversalPlanes, true).ok()?;
    drm::Device::set_client_capability(&card, drm::ClientCapability::Atomic, true).ok()?;
    Some(card)
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

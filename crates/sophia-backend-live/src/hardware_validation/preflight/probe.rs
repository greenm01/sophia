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
    let readiness =
        crate::hardware_validation::atomic_scanout_card::inspect_real_atomic_scanout_card(path);
    AtomicPreflightCardReadiness {
        openable: readiness.openable,
        atomic_capable: readiness.atomic_capable,
        scanout_target: readiness.scanout_target,
        atomic_properties: readiness.atomic_properties,
    }
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

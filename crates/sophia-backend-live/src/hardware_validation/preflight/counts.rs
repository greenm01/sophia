pub const LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS: u8 = 8;

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

pub const fn capped_scanout_target_primary_card_count(
    scanout_target_primary_card_nodes: u8,
    atomic_capable_primary_card_nodes: u8,
) -> u8 {
    if scanout_target_primary_card_nodes > atomic_capable_primary_card_nodes {
        atomic_capable_primary_card_nodes
    } else {
        scanout_target_primary_card_nodes
    }
}

pub const fn capped_atomic_property_primary_card_count(
    atomic_property_primary_card_nodes: u8,
    scanout_target_primary_card_nodes: u8,
) -> u8 {
    if atomic_property_primary_card_nodes > scanout_target_primary_card_nodes {
        scanout_target_primary_card_nodes
    } else {
        atomic_property_primary_card_nodes
    }
}

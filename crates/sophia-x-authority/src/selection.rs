use std::collections::BTreeMap;

use sophia_protocol::NamespaceId;

use crate::{XResourceId, XWindowTable};

pub type XAtom = u32;
pub type XTimestamp = u32;

pub const X_ATOM_NONE: XAtom = 0;
pub const MAX_CLIPBOARD_TEXT_HANDOFF_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XSelectionChangeKind {
    SetOwner,
    ClearOwner,
    SelectionWindowDestroyed,
    SelectionClientClosed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionEvent {
    pub selection: XAtom,
    pub owner: Option<XResourceId>,
    pub timestamp: XTimestamp,
    pub selection_timestamp: XTimestamp,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerRecord {
    pub selection: XAtom,
    pub namespace: Option<NamespaceId>,
    pub owner: Option<XResourceId>,
    pub generation: u64,
    pub timestamp: XTimestamp,
    pub selection_timestamp: XTimestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerUpdate {
    pub previous: Option<XSelectionOwnerRecord>,
    pub current: XSelectionOwnerRecord,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XSelectionMonitor {
    owners: BTreeMap<(XAtom, Option<NamespaceId>), XSelectionOwnerRecord>,
}

impl XSelectionMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn owner(
        &self,
        selection: XAtom,
        namespace: Option<NamespaceId>,
    ) -> Option<XSelectionOwnerRecord> {
        self.owners.get(&(selection, namespace)).copied()
    }

    pub fn current_owner_for_selection(&self, selection: XAtom) -> Option<XSelectionOwnerRecord> {
        self.owners
            .values()
            .filter(|record| record.selection == selection && record.owner.is_some())
            .max_by_key(|record| record.generation)
            .copied()
    }

    pub fn apply_event(
        &mut self,
        event: XSelectionEvent,
        windows: &XWindowTable,
    ) -> XSelectionOwnerUpdate {
        let namespace_from_owner = event
            .owner
            .and_then(|owner| windows.get(owner).map(|window| window.namespace));
        let namespace =
            namespace_from_owner.or_else(|| self.namespace_for_existing_selection(event.selection));
        let key = (event.selection, namespace);
        let previous = self.owners.get(&key).copied();
        let generation = previous
            .map(|record| record.generation.saturating_add(1))
            .unwrap_or(1);
        let current = XSelectionOwnerRecord {
            selection: event.selection,
            namespace,
            owner: event.owner,
            generation,
            timestamp: event.timestamp,
            selection_timestamp: event.selection_timestamp,
        };

        self.owners.insert(key, current);

        XSelectionOwnerUpdate {
            previous,
            current,
            kind: event.kind,
        }
    }

    pub fn clear_window_owner(
        &mut self,
        window: XResourceId,
        windows: &XWindowTable,
        kind: XSelectionChangeKind,
    ) {
        let owners = self
            .owners
            .values()
            .filter(|record| record.owner == Some(window))
            .copied()
            .collect::<Vec<_>>();
        for owner in owners {
            self.apply_event(
                XSelectionEvent {
                    selection: owner.selection,
                    owner: None,
                    timestamp: owner.timestamp,
                    selection_timestamp: owner.selection_timestamp,
                    kind,
                },
                windows,
            );
        }
    }

    fn namespace_for_existing_selection(&self, selection: XAtom) -> Option<NamespaceId> {
        self.owners
            .iter()
            .find_map(|((record_selection, namespace), record)| {
                if *record_selection == selection && record.owner.is_some() {
                    *namespace
                } else {
                    None
                }
            })
    }
}

use super::*;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionAtoms {
    pub primary: Atom,
    pub secondary: Atom,
    pub clipboard: Atom,
}

impl XSelectionAtoms {
    pub const fn all(self) -> [Atom; 3] {
        [self.primary, self.secondary, self.clipboard]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XSelectionChangeKind {
    SetOwner,
    OwnerWindowDestroyed,
    OwnerClientClosed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionEvent {
    pub selection: Atom,
    pub owner: Option<XWindowId>,
    pub timestamp: u32,
    pub selection_timestamp: u32,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerRecord {
    pub selection: Atom,
    pub namespace: Option<NamespaceId>,
    pub owner: Option<XWindowId>,
    pub generation: u64,
    pub timestamp: u32,
    pub selection_timestamp: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerUpdate {
    pub previous: Option<XSelectionOwnerRecord>,
    pub current: XSelectionOwnerRecord,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardPortalOwnerChange {
    pub source_namespace: NamespaceId,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionFailureRequest {
    pub transfer: PortalTransferId,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub time: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionPortalRequest {
    pub request: ClipboardTransferRequest,
    pub failure: ClipboardSelectionFailureRequest,
    pub property: Atom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionRequestError {
    UnknownRequestorNamespace,
    UnknownSourceOwner,
    MissingSourceNamespace,
    SameNamespace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionDispatch {
    pub portal_request: ClipboardSelectionPortalRequest,
    pub command: PortalCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionDispatchError {
    NotSelectionRequest,
    Request(ClipboardSelectionRequestError),
    Portal(PortalError),
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XSelectionMonitor {
    owners: BTreeMap<(Atom, Option<NamespaceId>), XSelectionOwnerRecord>,
}

impl XSelectionMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn owner(
        &self,
        selection: Atom,
        namespace: Option<NamespaceId>,
    ) -> Option<XSelectionOwnerRecord> {
        self.owners.get(&(selection, namespace)).copied()
    }

    pub fn current_owner_for_selection(&self, selection: Atom) -> Option<XSelectionOwnerRecord> {
        self.owners
            .values()
            .filter(|record| record.selection == selection && record.owner.is_some())
            .max_by_key(|record| record.generation)
            .copied()
    }

    pub fn apply_event(
        &mut self,
        event: XSelectionEvent,
        mirror: &XMirrorState,
    ) -> XSelectionOwnerUpdate {
        let namespace_from_owner = event
            .owner
            .and_then(|owner| mirror.namespace_for_window(owner));
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

    fn namespace_for_existing_selection(&self, selection: Atom) -> Option<NamespaceId> {
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
impl XSelectionEvent {
    pub fn from_x11_event(event: &Event) -> Option<Self> {
        let Event::XfixesSelectionNotify(event) = event else {
            return None;
        };

        Some(Self {
            selection: event.selection,
            owner: nonzero_window(event.owner).map(wrap_xid),
            timestamp: event.timestamp,
            selection_timestamp: event.selection_timestamp,
            kind: selection_change_kind(event.subtype),
        })
    }
}

fn selection_change_kind(kind: SelectionEvent) -> XSelectionChangeKind {
    if kind == SelectionEvent::SET_SELECTION_OWNER {
        XSelectionChangeKind::SetOwner
    } else if kind == SelectionEvent::SELECTION_WINDOW_DESTROY {
        XSelectionChangeKind::OwnerWindowDestroyed
    } else if kind == SelectionEvent::SELECTION_CLIENT_CLOSE {
        XSelectionChangeKind::OwnerClientClosed
    } else {
        XSelectionChangeKind::Unknown
    }
}

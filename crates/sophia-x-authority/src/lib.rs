//! Passive Sophia X Authority resource model.
//!
//! This crate intentionally starts without a live socket parser. It models the
//! authority-owned tables that later X protocol dispatch will mutate.

use std::collections::{BTreeMap, BTreeSet};

use sophia_portal::{
    ClipboardPortal, ClipboardTarget, ClipboardTransferRequest, PortalCommand, PortalError,
};
use sophia_protocol::{
    AuthorityKind, AuthorityLocalId, AuthoritySurface, BufferSource, NamespaceId, PortalTransferId,
    Rect, Region, SurfaceConstraints, SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness,
    TransactionId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct XResourceId {
    pub local: AuthorityLocalId,
}

impl XResourceId {
    pub const NONE: Self = Self {
        local: AuthorityLocalId::NONE,
    };

    pub const fn new(raw: u64, generation: u32) -> Self {
        Self {
            local: AuthorityLocalId::new(raw, generation),
        }
    }

    pub const fn is_valid(self) -> bool {
        self.local.is_valid()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XResourceKind {
    Window,
    Pixmap,
    Atom,
    Property,
    GraphicsContext,
    Cursor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XMapState {
    Unmapped,
    Mapped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XResourceRecord {
    pub id: XResourceId,
    pub kind: XResourceKind,
    pub owner_namespace: NamespaceId,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XAuthorityAccessError {
    InvalidResource,
    InvalidNamespace,
    InvalidSurface,
    UnknownResource,
    WrongResourceKind,
    CrossNamespaceDenied,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XResourceTable {
    records: BTreeMap<XResourceId, XResourceRecord>,
}

impl XResourceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        id: XResourceId,
        kind: XResourceKind,
        owner_namespace: NamespaceId,
        generation: u64,
    ) -> Result<(), XAuthorityAccessError> {
        if !id.is_valid() {
            return Err(XAuthorityAccessError::InvalidResource);
        }
        if !owner_namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }

        self.records.insert(
            id,
            XResourceRecord {
                id,
                kind,
                owner_namespace,
                generation,
            },
        );
        Ok(())
    }

    pub fn get(&self, id: XResourceId) -> Option<&XResourceRecord> {
        self.records.get(&id)
    }

    pub fn lookup(
        &self,
        requester_namespace: NamespaceId,
        id: XResourceId,
        expected_kind: XResourceKind,
    ) -> Result<&XResourceRecord, XAuthorityAccessError> {
        if !requester_namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }

        let record = self
            .records
            .get(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;

        if record.kind != expected_kind {
            return Err(XAuthorityAccessError::WrongResourceKind);
        }
        if record.owner_namespace != requester_namespace {
            return Err(XAuthorityAccessError::CrossNamespaceDenied);
        }

        Ok(record)
    }

    pub fn remove(&mut self, id: XResourceId) -> Option<XResourceRecord> {
        self.records.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum XEventClass {
    Structure,
    Property,
    Focus,
    Keyboard,
    Pointer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XEventSubscriptionTable {
    subscriptions: BTreeMap<XResourceId, BTreeMap<NamespaceId, BTreeSet<XEventClass>>>,
}

impl XEventSubscriptionTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &mut self,
        resources: &XResourceTable,
        requester_namespace: NamespaceId,
        target: XResourceId,
        class: XEventClass,
    ) -> Result<(), XAuthorityAccessError> {
        resources.lookup(requester_namespace, target, XResourceKind::Window)?;
        self.subscriptions
            .entry(target)
            .or_default()
            .entry(requester_namespace)
            .or_default()
            .insert(class);
        Ok(())
    }

    pub fn subscribers(
        &self,
        target: XResourceId,
        owner_namespace: NamespaceId,
        class: XEventClass,
    ) -> Vec<NamespaceId> {
        self.subscriptions
            .get(&target)
            .into_iter()
            .flat_map(|by_namespace| by_namespace.iter())
            .filter_map(|(namespace, classes)| {
                (*namespace == owner_namespace && classes.contains(&class)).then_some(*namespace)
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XWindowRecord {
    pub id: XResourceId,
    pub surface: SurfaceId,
    pub namespace: NamespaceId,
    pub map_state: XMapState,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

impl XWindowRecord {
    pub fn authority_surface(&self) -> AuthoritySurface {
        AuthoritySurface {
            authority: AuthorityKind::SophiaX,
            local_id: self.id.local,
            surface: self.surface,
            namespace: Some(self.namespace),
            mapped: self.map_state == XMapState::Mapped,
            geometry: self.geometry,
            constraints: self.constraints,
            generation: self.generation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWindowLifecycleEvent {
    Created {
        id: XResourceId,
        surface: SurfaceId,
        namespace: NamespaceId,
        geometry: Rect,
        constraints: SurfaceConstraints,
        generation: u64,
    },
    Mapped {
        id: XResourceId,
        generation: u64,
    },
    Unmapped {
        id: XResourceId,
        generation: u64,
    },
    Destroyed {
        id: XResourceId,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XWindowTable {
    windows: BTreeMap<XResourceId, XWindowRecord>,
}

impl XWindowTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        event: XWindowLifecycleEvent,
    ) -> Result<Option<AuthoritySurface>, XAuthorityAccessError> {
        match event {
            XWindowLifecycleEvent::Created {
                id,
                surface,
                namespace,
                geometry,
                constraints,
                generation,
            } => {
                if !id.is_valid() {
                    return Err(XAuthorityAccessError::InvalidResource);
                }
                if !surface.is_valid() {
                    return Err(XAuthorityAccessError::InvalidResource);
                }
                if !namespace.is_valid() {
                    return Err(XAuthorityAccessError::InvalidNamespace);
                }

                let record = XWindowRecord {
                    id,
                    surface,
                    namespace,
                    map_state: XMapState::Unmapped,
                    geometry,
                    constraints,
                    generation,
                };
                let authority_surface = record.authority_surface();
                self.windows.insert(id, record);
                Ok(Some(authority_surface))
            }
            XWindowLifecycleEvent::Mapped { id, generation } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                record.map_state = XMapState::Mapped;
                record.generation = generation;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Unmapped { id, generation } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                record.map_state = XMapState::Unmapped;
                record.generation = generation;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Destroyed { id } => {
                self.windows.remove(&id);
                Ok(None)
            }
        }
    }

    pub fn get(&self, id: XResourceId) -> Option<&XWindowRecord> {
        self.windows.get(&id)
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XDrawingUpdateKind {
    PresentPixmap,
    ShmPutImage,
    CoreDraw,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDrawingUpdate {
    pub transaction: TransactionId,
    pub requester_namespace: NamespaceId,
    pub target_window: XResourceId,
    pub kind: XDrawingUpdateKind,
    pub buffer: BufferSource,
    pub damage: Region,
    pub previous_committed_generation: u64,
    pub timeout_msec: u32,
}

impl XDrawingUpdate {
    pub fn present_pixmap(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        pixmap: u32,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::PresentPixmap,
            buffer: BufferSource::XPixmap { pixmap },
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }

    pub fn shm_put_image(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        handle: u64,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::ShmPutImage,
            buffer: BufferSource::CpuBuffer { handle },
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }

    pub fn core_draw(
        transaction: TransactionId,
        requester_namespace: NamespaceId,
        target_window: XResourceId,
        handle: u64,
        damage: Region,
        previous_committed_generation: u64,
        timeout_msec: u32,
    ) -> Self {
        Self {
            transaction,
            requester_namespace,
            target_window,
            kind: XDrawingUpdateKind::CoreDraw,
            buffer: BufferSource::CpuBuffer { handle },
            damage,
            previous_committed_generation,
            timeout_msec,
        }
    }
}

pub fn surface_transaction_from_drawing_update(
    windows: &XWindowTable,
    update: XDrawingUpdate,
) -> Result<SurfaceTransaction, XAuthorityAccessError> {
    if !update.transaction.is_valid() {
        return Err(XAuthorityAccessError::InvalidResource);
    }
    if !update.requester_namespace.is_valid() {
        return Err(XAuthorityAccessError::InvalidNamespace);
    }
    if !update.target_window.is_valid() {
        return Err(XAuthorityAccessError::InvalidResource);
    }
    if matches!(update.buffer, BufferSource::None) {
        return Err(XAuthorityAccessError::InvalidResource);
    }

    let window = windows
        .get(update.target_window)
        .ok_or(XAuthorityAccessError::UnknownResource)?;

    if window.namespace != update.requester_namespace {
        return Err(XAuthorityAccessError::CrossNamespaceDenied);
    }
    if !window.surface.is_valid() {
        return Err(XAuthorityAccessError::InvalidSurface);
    }

    Ok(SurfaceTransaction {
        transaction: update.transaction,
        authority: AuthorityKind::SophiaX,
        surface: window.surface,
        namespace: Some(window.namespace),
        target_geometry: window.geometry,
        target_buffer: update.buffer,
        damage: update.damage,
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: update.timeout_msec,
        previous_committed_generation: update.previous_committed_generation,
    })
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XSelectionRequest {
    pub requestor: XResourceId,
    pub selection: XAtom,
    pub target: XAtom,
    pub target_name: String,
    pub property: XAtom,
    pub time: XTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionPortalRequest {
    pub request: ClipboardTransferRequest,
    pub failure: ClipboardSelectionFailureRequest,
    pub property: XAtom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionRequestError {
    UnknownRequestorNamespace,
    UnknownSourceOwner,
    MissingSourceNamespace,
    SameNamespace,
    Portal(PortalError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionDispatch {
    pub portal_request: ClipboardSelectionPortalRequest,
    pub command: PortalCommand,
}

pub fn dispatch_clipboard_selection_request(
    request: XSelectionRequest,
    monitor: &XSelectionMonitor,
    windows: &XWindowTable,
    transfer: PortalTransferId,
    portal: &mut ClipboardPortal,
) -> Result<ClipboardSelectionDispatch, ClipboardSelectionRequestError> {
    let portal_request =
        clipboard_portal_request_from_selection_request(request, monitor, windows, transfer)?;
    let command = portal
        .request_import(portal_request.request.clone())
        .map_err(ClipboardSelectionRequestError::Portal)?;

    Ok(ClipboardSelectionDispatch {
        portal_request,
        command,
    })
}

pub fn clipboard_portal_request_from_selection_request(
    request: XSelectionRequest,
    monitor: &XSelectionMonitor,
    windows: &XWindowTable,
    transfer: PortalTransferId,
) -> Result<ClipboardSelectionPortalRequest, ClipboardSelectionRequestError> {
    let requestor = windows
        .get(request.requestor)
        .ok_or(ClipboardSelectionRequestError::UnknownRequestorNamespace)?;
    let source_owner = monitor
        .current_owner_for_selection(request.selection)
        .ok_or(ClipboardSelectionRequestError::UnknownSourceOwner)?;
    let source_namespace = source_owner
        .namespace
        .ok_or(ClipboardSelectionRequestError::MissingSourceNamespace)?;

    if source_namespace == requestor.namespace {
        return Err(ClipboardSelectionRequestError::SameNamespace);
    }

    Ok(ClipboardSelectionPortalRequest {
        request: ClipboardTransferRequest {
            transfer,
            source_namespace,
            target_namespace: requestor.namespace,
            target: ClipboardTarget::Atom(request.target_name),
            byte_size: 0,
            generation: source_owner.generation,
        },
        failure: ClipboardSelectionFailureRequest {
            transfer,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            time: request.time,
        },
        property: request.property,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionFailureRequest {
    pub transfer: PortalTransferId,
    pub requestor: XResourceId,
    pub selection: XAtom,
    pub target: XAtom,
    pub time: XTimestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionNotify {
    pub time: XTimestamp,
    pub requestor: XResourceId,
    pub selection: XAtom,
    pub target: XAtom,
    pub property: XAtom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionFailure {
    pub transfer: PortalTransferId,
    pub notify: ClipboardSelectionNotify,
}

impl ClipboardSelectionFailure {
    pub fn failed_normally(&self) -> bool {
        self.notify.property == X_ATOM_NONE
    }
}

pub fn clipboard_selection_failure_notify(
    request: ClipboardSelectionFailureRequest,
) -> ClipboardSelectionFailure {
    ClipboardSelectionFailure {
        transfer: request.transfer,
        notify: ClipboardSelectionNotify {
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: X_ATOM_NONE,
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardTextProperty {
    pub requestor: XResourceId,
    pub property: XAtom,
    pub target: XAtom,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionHandoff {
    pub transfer: PortalTransferId,
    pub property: ClipboardTextProperty,
    pub notify: ClipboardSelectionNotify,
}

impl ClipboardSelectionHandoff {
    pub fn succeeded_normally(&self) -> bool {
        self.notify.property == self.property.property && self.notify.property != X_ATOM_NONE
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionHandoffError {
    NotHandoffCommand,
    TransferMismatch,
    MissingProperty,
    UnsupportedTarget,
    TextTooLarge { len: usize, max: usize },
}

pub fn clipboard_selection_text_handoff_artifact(
    command: &PortalCommand,
    request: &ClipboardSelectionPortalRequest,
    text: impl AsRef<str>,
) -> Result<ClipboardSelectionHandoff, ClipboardSelectionHandoffError> {
    let PortalCommand::HandoffClipboard { transfer } = command else {
        return Err(ClipboardSelectionHandoffError::NotHandoffCommand);
    };

    if *transfer != request.request.transfer {
        return Err(ClipboardSelectionHandoffError::TransferMismatch);
    }
    if request.property == X_ATOM_NONE {
        return Err(ClipboardSelectionHandoffError::MissingProperty);
    }
    if !request.request.target.is_text() {
        return Err(ClipboardSelectionHandoffError::UnsupportedTarget);
    }

    let bytes = text.as_ref().as_bytes();
    if bytes.len() > MAX_CLIPBOARD_TEXT_HANDOFF_BYTES {
        return Err(ClipboardSelectionHandoffError::TextTooLarge {
            len: bytes.len(),
            max: MAX_CLIPBOARD_TEXT_HANDOFF_BYTES,
        });
    }

    let failure = request.failure;
    Ok(ClipboardSelectionHandoff {
        transfer: *transfer,
        property: ClipboardTextProperty {
            requestor: failure.requestor,
            property: request.property,
            target: failure.target,
            bytes: bytes.to_vec(),
        },
        notify: ClipboardSelectionNotify {
            time: failure.time,
            requestor: failure.requestor,
            selection: failure.selection,
            target: failure.target,
            property: request.property,
        },
    })
}

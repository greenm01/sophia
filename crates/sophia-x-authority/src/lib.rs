//! Passive Sophia X Authority resource model.
//!
//! This crate intentionally starts without a live socket parser. It models the
//! authority-owned tables that later X protocol dispatch will mutate.

use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::{
    AuthorityKind, AuthorityLocalId, AuthoritySurface, NamespaceId, Rect, SurfaceConstraints,
    SurfaceId,
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

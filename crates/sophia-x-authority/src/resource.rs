use std::collections::BTreeMap;

use sophia_protocol::{AuthorityLocalId, NamespaceId};

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
    Font,
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
    StaleGeneration,
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

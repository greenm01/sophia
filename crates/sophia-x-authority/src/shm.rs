use std::collections::BTreeMap;

use sophia_protocol::NamespaceId;

use crate::{XAuthorityAccessError, XResourceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XShmSegmentRecord {
    pub id: XResourceId,
    pub namespace: NamespaceId,
    pub shmid: u32,
    pub read_only: bool,
    pub generation: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XShmSegmentTable {
    records: BTreeMap<XResourceId, XShmSegmentRecord>,
}

impl XShmSegmentTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
        shmid: u32,
        read_only: bool,
        generation: u64,
    ) -> Result<(), XAuthorityAccessError> {
        if !namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }
        if !id.is_valid() {
            return Err(XAuthorityAccessError::InvalidResource);
        }

        self.records.insert(
            id,
            XShmSegmentRecord {
                id,
                namespace,
                shmid,
                read_only,
                generation,
            },
        );
        Ok(())
    }

    pub fn lookup(
        &self,
        namespace: NamespaceId,
        id: XResourceId,
    ) -> Result<&XShmSegmentRecord, XAuthorityAccessError> {
        if !namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }
        if !id.is_valid() {
            return Err(XAuthorityAccessError::InvalidResource);
        }

        let record = self
            .records
            .get(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        if record.namespace != namespace {
            return Err(XAuthorityAccessError::CrossNamespaceDenied);
        }
        Ok(record)
    }

    pub fn detach(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
    ) -> Result<(), XAuthorityAccessError> {
        self.lookup(namespace, id)?;
        self.records.remove(&id);
        Ok(())
    }

    pub fn ids_for_namespace_in_client_range(
        &self,
        namespace: NamespaceId,
        range: crate::XWireClientResourceRange,
    ) -> Vec<XResourceId> {
        self.records
            .values()
            .filter(|record| {
                record.namespace == namespace
                    && u32::try_from(record.id.local.raw())
                        .is_ok_and(|raw| range.owns_new_resource(raw))
            })
            .map(|record| record.id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

use std::collections::BTreeMap;

use sophia_protocol::NamespaceId;

use crate::{XAtom, XAtomTable, XResourceId, is_metadata_candidate_name};

pub const X_PROPERTY_MAX_VALUE_BYTES: usize = 64 * 1024;
pub const X_PROPERTY_ANY_TYPE: XAtom = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XPropertyMode {
    Replace,
    Prepend,
    Append,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyChange {
    pub mode: XPropertyMode,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_type: XAtom,
    pub format: u8,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyRead {
    pub delete: bool,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_type: XAtom,
    pub long_offset: u32,
    pub long_length: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyReadReply {
    pub property_type: XAtom,
    pub format: u8,
    pub bytes_after: u32,
    pub item_count: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyRecord {
    pub namespace: NamespaceId,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_type: XAtom,
    pub format: u8,
    pub bytes: Vec<u8>,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XMetadataPropertyCandidate {
    pub namespace: NamespaceId,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_name: String,
    pub property_type: XAtom,
    pub property_type_name: Option<String>,
    pub format: u8,
    pub byte_len: usize,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XPropertyError {
    InvalidNamespace,
    InvalidWindow,
    InvalidFormat(u8),
    ValueTooLarge { len: usize, max: usize },
    TypeMismatch,
    InvalidOffset,
    ReadTooLarge { len: usize, max: usize },
}

pub fn metadata_property_candidate(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
) -> Option<XMetadataPropertyCandidate> {
    let property_name = atoms.name(record.property)?;
    if !is_metadata_candidate_name(property_name) {
        return None;
    }
    Some(XMetadataPropertyCandidate {
        namespace: record.namespace,
        window: record.window,
        property: record.property,
        property_name: property_name.to_owned(),
        property_type: record.property_type,
        property_type_name: atoms
            .name(record.property_type)
            .map(std::borrow::ToOwned::to_owned),
        format: record.format,
        byte_len: record.bytes.len(),
        generation: record.generation,
    })
}

impl core::fmt::Display for XPropertyError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for XPropertyError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XPropertyTable {
    records: BTreeMap<(NamespaceId, XResourceId, XAtom), XPropertyRecord>,
}

impl XPropertyTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_change(
        &mut self,
        namespace: NamespaceId,
        change: XPropertyChange,
    ) -> Result<XPropertyRecord, XPropertyError> {
        if !namespace.is_valid() {
            return Err(XPropertyError::InvalidNamespace);
        }
        if !change.window.is_valid() {
            return Err(XPropertyError::InvalidWindow);
        }
        validate_property_format(change.format)?;
        if change.bytes.len() > X_PROPERTY_MAX_VALUE_BYTES {
            return Err(XPropertyError::ValueTooLarge {
                len: change.bytes.len(),
                max: X_PROPERTY_MAX_VALUE_BYTES,
            });
        }

        let key = (namespace, change.window, change.property);
        let previous = self.records.get(&key);
        let generation = previous
            .map(|record| record.generation.saturating_add(1))
            .unwrap_or(1);
        let bytes = match (change.mode, previous) {
            (XPropertyMode::Replace, _) | (_, None) => change.bytes,
            (XPropertyMode::Append, Some(record)) => {
                ensure_same_property_shape(record, &change)?;
                joined_bytes(&record.bytes, &change.bytes)?
            }
            (XPropertyMode::Prepend, Some(record)) => {
                ensure_same_property_shape(record, &change)?;
                joined_bytes(&change.bytes, &record.bytes)?
            }
        };

        let record = XPropertyRecord {
            namespace,
            window: change.window,
            property: change.property,
            property_type: change.property_type,
            format: change.format,
            bytes,
            generation,
        };
        self.records.insert(key, record.clone());
        Ok(record)
    }

    pub fn get(
        &self,
        namespace: NamespaceId,
        window: XResourceId,
        property: XAtom,
    ) -> Option<&XPropertyRecord> {
        self.records.get(&(namespace, window, property))
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn read_property(
        &self,
        namespace: NamespaceId,
        read: XPropertyRead,
    ) -> Result<XPropertyReadReply, XPropertyError> {
        if !namespace.is_valid() {
            return Err(XPropertyError::InvalidNamespace);
        }
        if !read.window.is_valid() {
            return Err(XPropertyError::InvalidWindow);
        }

        let Some(record) = self.get(namespace, read.window, read.property) else {
            return Ok(XPropertyReadReply {
                property_type: X_PROPERTY_ANY_TYPE,
                format: 0,
                bytes_after: 0,
                item_count: 0,
                bytes: Vec::new(),
            });
        };

        if read.property_type != X_PROPERTY_ANY_TYPE && read.property_type != record.property_type {
            return Ok(XPropertyReadReply {
                property_type: record.property_type,
                format: record.format,
                bytes_after: u32::try_from(record.bytes.len()).unwrap_or(u32::MAX),
                item_count: 0,
                bytes: Vec::new(),
            });
        }

        let offset = usize::try_from(read.long_offset)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or(XPropertyError::InvalidOffset)?;
        if offset > record.bytes.len() {
            return Err(XPropertyError::InvalidOffset);
        }

        let max_read = usize::try_from(read.long_length)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or(XPropertyError::ReadTooLarge {
                len: usize::MAX,
                max: X_PROPERTY_MAX_VALUE_BYTES,
            })?;
        if max_read > X_PROPERTY_MAX_VALUE_BYTES {
            return Err(XPropertyError::ReadTooLarge {
                len: max_read,
                max: X_PROPERTY_MAX_VALUE_BYTES,
            });
        }

        let remaining = record.bytes.len() - offset;
        let returned_len = remaining.min(max_read);
        let bytes_after = remaining - returned_len;
        let item_width = usize::from(record.format / 8);
        Ok(XPropertyReadReply {
            property_type: record.property_type,
            format: record.format,
            bytes_after: u32::try_from(bytes_after).unwrap_or(u32::MAX),
            item_count: u32::try_from(returned_len / item_width).unwrap_or(u32::MAX),
            bytes: record.bytes[offset..offset + returned_len].to_vec(),
        })
    }
}

pub(crate) fn validate_property_format(format: u8) -> Result<(), XPropertyError> {
    match format {
        8 | 16 | 32 => Ok(()),
        other => Err(XPropertyError::InvalidFormat(other)),
    }
}

fn ensure_same_property_shape(
    record: &XPropertyRecord,
    change: &XPropertyChange,
) -> Result<(), XPropertyError> {
    if record.property_type != change.property_type || record.format != change.format {
        return Err(XPropertyError::TypeMismatch);
    }
    Ok(())
}

fn joined_bytes(first: &[u8], second: &[u8]) -> Result<Vec<u8>, XPropertyError> {
    let len = first
        .len()
        .checked_add(second.len())
        .ok_or(XPropertyError::ValueTooLarge {
            len: usize::MAX,
            max: X_PROPERTY_MAX_VALUE_BYTES,
        })?;
    if len > X_PROPERTY_MAX_VALUE_BYTES {
        return Err(XPropertyError::ValueTooLarge {
            len,
            max: X_PROPERTY_MAX_VALUE_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(len);
    bytes.extend_from_slice(first);
    bytes.extend_from_slice(second);
    Ok(bytes)
}

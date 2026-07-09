use std::collections::BTreeMap;

use sophia_protocol::NamespaceId;

use crate::{XAtom, XResourceId};

pub const X_PROPERTY_MAX_VALUE_BYTES: usize = 64 * 1024;

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
pub enum XPropertyError {
    InvalidNamespace,
    InvalidWindow,
    InvalidFormat(u8),
    ValueTooLarge { len: usize, max: usize },
    TypeMismatch,
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

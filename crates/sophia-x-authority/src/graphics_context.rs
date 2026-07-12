use std::collections::BTreeMap;

use sophia_protocol::{NamespaceId, Rect};

use crate::{XAuthorityAccessError, XResourceId};

pub const X_GX_COPY: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XPoint {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XGraphicsContextValues {
    pub function: u8,
    pub plane_mask: u32,
    pub foreground: u32,
    pub background: u32,
    pub line_width: u16,
    pub fill_style: u8,
    pub font: Option<XResourceId>,
    pub clip_x_origin: i16,
    pub clip_y_origin: i16,
    pub clip_rectangles: Vec<Rect>,
}

impl Default for XGraphicsContextValues {
    fn default() -> Self {
        Self {
            function: X_GX_COPY,
            plane_mask: u32::MAX,
            foreground: 0,
            background: 1,
            line_width: 0,
            fill_style: 0,
            font: None,
            clip_x_origin: 0,
            clip_y_origin: 0,
            clip_rectangles: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XGraphicsContextRecord {
    pub id: XResourceId,
    pub drawable: XResourceId,
    pub namespace: NamespaceId,
    pub values: XGraphicsContextValues,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XGraphicsContextTable {
    records: BTreeMap<XResourceId, XGraphicsContextRecord>,
}

impl XGraphicsContextTable {
    pub fn create(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
        drawable: XResourceId,
        values: XGraphicsContextValues,
    ) -> Result<(), XAuthorityAccessError> {
        if !namespace.is_valid() {
            return Err(XAuthorityAccessError::InvalidNamespace);
        }
        if !id.is_valid() || !drawable.is_valid() {
            return Err(XAuthorityAccessError::InvalidResource);
        }
        self.records.insert(
            id,
            XGraphicsContextRecord {
                id,
                drawable,
                namespace,
                values,
            },
        );
        Ok(())
    }

    pub fn get(
        &self,
        namespace: NamespaceId,
        id: XResourceId,
    ) -> Result<&XGraphicsContextRecord, XAuthorityAccessError> {
        let record = self
            .records
            .get(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        if record.namespace != namespace {
            return Err(XAuthorityAccessError::CrossNamespaceDenied);
        }
        Ok(record)
    }

    pub fn set_clip_rectangles(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
        rectangles: Vec<Rect>,
    ) -> Result<(), XAuthorityAccessError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        if record.namespace != namespace {
            return Err(XAuthorityAccessError::CrossNamespaceDenied);
        }
        record.values.clip_rectangles = rectangles;
        Ok(())
    }

    pub fn remove(
        &mut self,
        namespace: NamespaceId,
        id: XResourceId,
    ) -> Result<(), XAuthorityAccessError> {
        self.get(namespace, id)?;
        self.records.remove(&id);
        Ok(())
    }
}

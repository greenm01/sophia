use sophia_protocol::{
    NamespaceId, PortalTransferId, Rect, SurfaceConstraints, SurfaceId, TransactionId,
};

use crate::{
    XAtom, XAuthorityRequestKind, XAuthorityRequestPacket, XByteOrder, XPropertyChange,
    XPropertyMode, XResourceId, XSelectionChangeKind, padded_len,
};

const X_CREATE_WINDOW: u8 = 1;
const X_MAP_WINDOW: u8 = 8;
const X_INTERN_ATOM: u8 = 16;
const X_GET_ATOM_NAME: u8 = 17;
const X_CHANGE_PROPERTY: u8 = 18;
const X_SET_SELECTION_OWNER: u8 = 22;
const X_CONVERT_SELECTION: u8 = 24;

const X_CREATE_WINDOW_REQ_LEN: usize = 32;
const X_MAP_WINDOW_REQ_LEN: usize = 8;
const X_INTERN_ATOM_REQ_LEN: usize = 8;
const X_GET_ATOM_NAME_REQ_LEN: usize = 8;
const X_CHANGE_PROPERTY_REQ_LEN: usize = 24;
const X_SET_SELECTION_OWNER_REQ_LEN: usize = 16;
const X_CONVERT_SELECTION_REQ_LEN: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub transaction: TransactionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireRequest {
    Authority(XAuthorityRequestPacket),
    InternAtom { only_if_exists: bool, name: String },
    GetAtomName { atom: XAtom },
    ChangeProperty(XPropertyChange),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireParseError {
    Truncated {
        needed: usize,
        actual: usize,
    },
    InvalidLength {
        opcode: u8,
        expected_at_least: usize,
        actual: usize,
    },
    TrailingBytes(usize),
    UnknownOpcode(u8),
    InvalidPropertyMode(u8),
    InvalidPropertyFormat(u8),
    PropertyValueTooLarge {
        len: usize,
        max: usize,
    },
}

impl core::fmt::Display for XWireParseError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for XWireParseError {}

pub fn decode_x11_core_request(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    if bytes.len() < 4 {
        return Err(XWireParseError::Truncated {
            needed: 4,
            actual: bytes.len(),
        });
    }

    let opcode = bytes[0];
    let declared_len = usize::from(context.byte_order.u16(&bytes[2..4])) * 4;
    if declared_len < 4 {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least: 4,
            actual: declared_len,
        });
    }
    if bytes.len() < declared_len {
        return Err(XWireParseError::Truncated {
            needed: declared_len,
            actual: bytes.len(),
        });
    }
    if bytes.len() > declared_len {
        return Err(XWireParseError::TrailingBytes(bytes.len() - declared_len));
    }

    match opcode {
        X_CREATE_WINDOW => decode_create_window(context, bytes),
        X_MAP_WINDOW => decode_map_window(context, bytes),
        X_INTERN_ATOM => decode_intern_atom(context, bytes),
        X_GET_ATOM_NAME => decode_get_atom_name(context, bytes),
        X_CHANGE_PROPERTY => decode_change_property(context, bytes),
        X_SET_SELECTION_OWNER => decode_set_selection_owner(context, bytes),
        X_CONVERT_SELECTION => decode_convert_selection(context, bytes),
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}

fn decode_intern_atom(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_INTERN_ATOM, X_INTERN_ATOM_REQ_LEN, bytes.len())?;
    let name_len = usize::from(context.byte_order.u16(&bytes[4..6]));
    let expected_len = X_INTERN_ATOM_REQ_LEN + padded_len(name_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_INTERN_ATOM,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let name =
        core::str::from_utf8(&bytes[X_INTERN_ATOM_REQ_LEN..X_INTERN_ATOM_REQ_LEN + name_len])
            .map_err(|_| XWireParseError::InvalidLength {
                opcode: X_INTERN_ATOM,
                expected_at_least: expected_len,
                actual: bytes.len(),
            })?;
    Ok(XWireRequest::InternAtom {
        only_if_exists: bytes[1] != 0,
        name: name.to_owned(),
    })
}

fn decode_get_atom_name(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_ATOM_NAME, X_GET_ATOM_NAME_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GetAtomName {
        atom: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_create_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_CREATE_WINDOW, X_CREATE_WINDOW_REQ_LEN, bytes.len())?;
    let window_raw = context.byte_order.u32(&bytes[4..8]);
    let window = XResourceId::new(u64::from(window_raw), 1);
    Ok(XWireRequest::Authority(XAuthorityRequestPacket {
        transaction: context.transaction,
        namespace: context.namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(window_raw, 1),
            geometry: Rect {
                x: i32::from(context.byte_order.i16(&bytes[12..14])),
                y: i32::from(context.byte_order.i16(&bytes[14..16])),
                width: i32::from(context.byte_order.u16(&bytes[16..18])),
                height: i32::from(context.byte_order.u16(&bytes[18..20])),
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    }))
}

fn decode_map_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_MAP_WINDOW, X_MAP_WINDOW_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::Authority(XAuthorityRequestPacket {
        transaction: context.transaction,
        namespace: context.namespace,
        kind: XAuthorityRequestKind::MapWindow {
            window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            generation: 2,
        },
    }))
}

fn decode_change_property(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_CHANGE_PROPERTY, X_CHANGE_PROPERTY_REQ_LEN, bytes.len())?;
    let mode = match bytes[1] {
        0 => XPropertyMode::Replace,
        1 => XPropertyMode::Prepend,
        2 => XPropertyMode::Append,
        other => return Err(XWireParseError::InvalidPropertyMode(other)),
    };
    let format = bytes[16];
    validate_wire_property_format(format)?;
    let units = context.byte_order.u32(&bytes[20..24]) as usize;
    let unit_width = usize::from(format / 8);
    let value_len =
        units
            .checked_mul(unit_width)
            .ok_or(XWireParseError::PropertyValueTooLarge {
                len: usize::MAX,
                max: crate::X_PROPERTY_MAX_VALUE_BYTES,
            })?;
    if value_len > crate::X_PROPERTY_MAX_VALUE_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: value_len,
            max: crate::X_PROPERTY_MAX_VALUE_BYTES,
        });
    }
    let expected_len = X_CHANGE_PROPERTY_REQ_LEN + padded_len(value_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CHANGE_PROPERTY,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }

    Ok(XWireRequest::ChangeProperty(XPropertyChange {
        mode,
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        property: context.byte_order.u32(&bytes[8..12]),
        property_type: context.byte_order.u32(&bytes[12..16]),
        format,
        bytes: bytes[X_CHANGE_PROPERTY_REQ_LEN..X_CHANGE_PROPERTY_REQ_LEN + value_len].to_vec(),
    }))
}

fn decode_set_selection_owner(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_SET_SELECTION_OWNER,
        X_SET_SELECTION_OWNER_REQ_LEN,
        bytes.len(),
    )?;
    let owner_raw = context.byte_order.u32(&bytes[4..8]);
    let owner = if owner_raw == 0 {
        None
    } else {
        Some(XResourceId::new(u64::from(owner_raw), 1))
    };
    Ok(XWireRequest::Authority(XAuthorityRequestPacket {
        transaction: context.transaction,
        namespace: context.namespace,
        kind: XAuthorityRequestKind::SetSelectionOwner {
            selection: context.byte_order.u32(&bytes[8..12]),
            owner,
            timestamp: context.byte_order.u32(&bytes[12..16]),
            selection_timestamp: context.byte_order.u32(&bytes[12..16]),
            kind: if owner.is_some() {
                XSelectionChangeKind::SetOwner
            } else {
                XSelectionChangeKind::ClearOwner
            },
        },
    }))
}

fn decode_convert_selection(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_CONVERT_SELECTION,
        X_CONVERT_SELECTION_REQ_LEN,
        bytes.len(),
    )?;
    let target = context.byte_order.u32(&bytes[12..16]);
    Ok(XWireRequest::Authority(XAuthorityRequestPacket {
        transaction: context.transaction,
        namespace: context.namespace,
        kind: XAuthorityRequestKind::RequestSelection {
            requestor: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            selection: context.byte_order.u32(&bytes[8..12]),
            target,
            target_name: format!("atom:{target}"),
            property: context.byte_order.u32(&bytes[16..20]),
            time: context.byte_order.u32(&bytes[20..24]),
            transfer: PortalTransferId::from_raw(context.transaction.raw()),
        },
    }))
}

fn require_len(opcode: u8, expected_at_least: usize, actual: usize) -> Result<(), XWireParseError> {
    if actual < expected_at_least {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least,
            actual,
        });
    }
    Ok(())
}

fn require_exact_len(opcode: u8, expected: usize, actual: usize) -> Result<(), XWireParseError> {
    if actual != expected {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least: expected,
            actual,
        });
    }
    Ok(())
}

fn validate_wire_property_format(format: u8) -> Result<(), XWireParseError> {
    match format {
        8 | 16 | 32 => Ok(()),
        other => Err(XWireParseError::InvalidPropertyFormat(other)),
    }
}

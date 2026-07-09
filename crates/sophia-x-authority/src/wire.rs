use sophia_protocol::{
    NamespaceId, PortalTransferId, Rect, Region, SurfaceConstraints, SurfaceId, TransactionId,
};

use crate::{
    XAtom, XAuthorityRequestKind, XAuthorityRequestPacket, XByteOrder, XPropertyChange,
    XPropertyMode, XPropertyRead, XResourceId, XSelectionChangeKind, padded_len,
};

const X_CREATE_WINDOW: u8 = 1;
const X_DESTROY_WINDOW: u8 = 4;
const X_MAP_WINDOW: u8 = 8;
const X_INTERN_ATOM: u8 = 16;
const X_GET_ATOM_NAME: u8 = 17;
const X_CHANGE_PROPERTY: u8 = 18;
const X_GET_PROPERTY: u8 = 20;
const X_SET_SELECTION_OWNER: u8 = 22;
const X_CONVERT_SELECTION: u8 = 24;
const X_GET_INPUT_FOCUS: u8 = 43;
const X_CREATE_GC: u8 = 55;
const X_FREE_GC: u8 = 60;
const X_POLY_FILL_RECTANGLE: u8 = 70;
const X_PUT_IMAGE: u8 = 72;
const X_QUERY_EXTENSION: u8 = 98;
const X_LIST_EXTENSIONS: u8 = 99;
const X_QUERY_BEST_SIZE: u8 = 97;

pub const X_SOPHIA_PRESENT_EXTENSION_NAME: &str = "SOPHIA-PRESENT";
pub const X_SOPHIA_PRESENT_MAJOR_OPCODE: u8 = 130;
pub const X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE: u8 = 0;

const X_CREATE_WINDOW_REQ_LEN: usize = 32;
const X_DESTROY_WINDOW_REQ_LEN: usize = 8;
const X_MAP_WINDOW_REQ_LEN: usize = 8;
const X_INTERN_ATOM_REQ_LEN: usize = 8;
const X_GET_ATOM_NAME_REQ_LEN: usize = 8;
const X_CHANGE_PROPERTY_REQ_LEN: usize = 24;
const X_GET_PROPERTY_REQ_LEN: usize = 24;
const X_SET_SELECTION_OWNER_REQ_LEN: usize = 16;
const X_CONVERT_SELECTION_REQ_LEN: usize = 24;
const X_GET_INPUT_FOCUS_REQ_LEN: usize = 4;
const X_CREATE_GC_REQ_LEN: usize = 16;
const X_FREE_GC_REQ_LEN: usize = 8;
const X_POLY_FILL_RECTANGLE_REQ_LEN: usize = 12;
const X_PUT_IMAGE_REQ_LEN: usize = 24;
const X_QUERY_EXTENSION_REQ_LEN: usize = 8;
const X_LIST_EXTENSIONS_REQ_LEN: usize = 4;
const X_QUERY_BEST_SIZE_REQ_LEN: usize = 12;
const X_SOPHIA_PRESENT_PIXMAP_REQ_LEN: usize = 32;

pub const X_PUT_IMAGE_MAX_DATA_BYTES: usize = crate::X_PROPERTY_MAX_VALUE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub transaction: TransactionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireRequest {
    Authority(XAuthorityRequestPacket),
    DestroyWindow {
        window: XResourceId,
    },
    InternAtom {
        only_if_exists: bool,
        name: String,
    },
    GetAtomName {
        atom: XAtom,
    },
    ChangeProperty(XPropertyChange),
    GetProperty(XPropertyRead),
    CreateGraphicsContext {
        gc: XResourceId,
        drawable: XResourceId,
    },
    FreeGraphicsContext {
        gc: XResourceId,
    },
    PolyFillRectangle {
        drawable: XResourceId,
        gc: XResourceId,
        rectangles: Vec<Rect>,
    },
    PutImage {
        format: u8,
        drawable: XResourceId,
        gc: XResourceId,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
        left_pad: u8,
        depth: u8,
        data_len: usize,
    },
    GetInputFocus,
    QueryExtension {
        name: String,
    },
    ListExtensions,
    QueryBestSize {
        class: u8,
        drawable: XResourceId,
        width: u16,
        height: u16,
    },
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
        X_DESTROY_WINDOW => decode_destroy_window(context, bytes),
        X_MAP_WINDOW => decode_map_window(context, bytes),
        X_INTERN_ATOM => decode_intern_atom(context, bytes),
        X_GET_ATOM_NAME => decode_get_atom_name(context, bytes),
        X_CHANGE_PROPERTY => decode_change_property(context, bytes),
        X_GET_PROPERTY => decode_get_property(context, bytes),
        X_SET_SELECTION_OWNER => decode_set_selection_owner(context, bytes),
        X_CONVERT_SELECTION => decode_convert_selection(context, bytes),
        X_GET_INPUT_FOCUS => decode_get_input_focus(bytes),
        X_CREATE_GC => decode_create_gc(context, bytes),
        X_FREE_GC => decode_free_gc(context, bytes),
        X_POLY_FILL_RECTANGLE => decode_poly_fill_rectangle(context, bytes),
        X_PUT_IMAGE => decode_put_image(context, bytes),
        X_QUERY_BEST_SIZE => decode_query_best_size(context, bytes),
        X_QUERY_EXTENSION => decode_query_extension(context, bytes),
        X_LIST_EXTENSIONS => decode_list_extensions(bytes),
        X_SOPHIA_PRESENT_MAJOR_OPCODE => decode_sophia_present(context, bytes),
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}

fn decode_sophia_present(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    if bytes[1] != X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE {
        return Err(XWireParseError::UnknownOpcode(bytes[0]));
    }
    require_exact_len(
        X_SOPHIA_PRESENT_MAJOR_OPCODE,
        X_SOPHIA_PRESENT_PIXMAP_REQ_LEN,
        bytes.len(),
    )?;
    let window = XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1);
    let damage = Region::single(Rect {
        x: i32::from(context.byte_order.i16(&bytes[12..14])),
        y: i32::from(context.byte_order.i16(&bytes[14..16])),
        width: i32::from(context.byte_order.u16(&bytes[16..18])),
        height: i32::from(context.byte_order.u16(&bytes[18..20])),
    });
    Ok(XWireRequest::Authority(XAuthorityRequestPacket {
        transaction: context.transaction,
        namespace: context.namespace,
        kind: XAuthorityRequestKind::PresentPixmap {
            window,
            pixmap: context.byte_order.u32(&bytes[8..12]),
            damage,
            previous_committed_generation: context.byte_order.u64(&bytes[20..28]),
            timeout_msec: context.byte_order.u32(&bytes[28..32]),
        },
    }))
}

fn decode_put_image(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_PUT_IMAGE, X_PUT_IMAGE_REQ_LEN, bytes.len())?;
    validate_wire_image_format(bytes[1])?;
    let data_len = bytes.len() - X_PUT_IMAGE_REQ_LEN;
    if data_len > X_PUT_IMAGE_MAX_DATA_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: data_len,
            max: X_PUT_IMAGE_MAX_DATA_BYTES,
        });
    }

    Ok(XWireRequest::PutImage {
        format: bytes[1],
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        width: context.byte_order.u16(&bytes[12..14]),
        height: context.byte_order.u16(&bytes[14..16]),
        dst_x: context.byte_order.i16(&bytes[16..18]),
        dst_y: context.byte_order.i16(&bytes[18..20]),
        left_pad: bytes[20],
        depth: bytes[21],
        data_len,
    })
}

fn decode_poly_fill_rectangle(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(
        X_POLY_FILL_RECTANGLE,
        X_POLY_FILL_RECTANGLE_REQ_LEN,
        bytes.len(),
    )?;
    let rectangle_bytes = &bytes[X_POLY_FILL_RECTANGLE_REQ_LEN..];
    if rectangle_bytes.len() % 8 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_POLY_FILL_RECTANGLE,
            expected_at_least: X_POLY_FILL_RECTANGLE_REQ_LEN + ((rectangle_bytes.len() + 7) & !7),
            actual: bytes.len(),
        });
    }
    let mut rectangles = Vec::with_capacity(rectangle_bytes.len() / 8);
    for rectangle in rectangle_bytes.chunks_exact(8) {
        rectangles.push(Rect {
            x: i32::from(context.byte_order.i16(&rectangle[0..2])),
            y: i32::from(context.byte_order.i16(&rectangle[2..4])),
            width: i32::from(context.byte_order.u16(&rectangle[4..6])),
            height: i32::from(context.byte_order.u16(&rectangle[6..8])),
        });
    }
    Ok(XWireRequest::PolyFillRectangle {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        rectangles,
    })
}

fn decode_free_gc(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_FREE_GC, X_FREE_GC_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::FreeGraphicsContext {
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_query_best_size(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_QUERY_BEST_SIZE, X_QUERY_BEST_SIZE_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::QueryBestSize {
        class: bytes[1],
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        width: context.byte_order.u16(&bytes[8..10]),
        height: context.byte_order.u16(&bytes[10..12]),
    })
}

fn decode_get_property(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_PROPERTY, X_GET_PROPERTY_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GetProperty(XPropertyRead {
        delete: bytes[1] != 0,
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        property: context.byte_order.u32(&bytes[8..12]),
        property_type: context.byte_order.u32(&bytes[12..16]),
        long_offset: context.byte_order.u32(&bytes[16..20]),
        long_length: context.byte_order.u32(&bytes[20..24]),
    }))
}

fn decode_create_gc(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_CREATE_GC, X_CREATE_GC_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::CreateGraphicsContext {
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
    })
}

fn decode_query_extension(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_QUERY_EXTENSION, X_QUERY_EXTENSION_REQ_LEN, bytes.len())?;
    let name_len = usize::from(context.byte_order.u16(&bytes[4..6]));
    let expected_len = X_QUERY_EXTENSION_REQ_LEN + padded_len(name_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_QUERY_EXTENSION,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let name = core::str::from_utf8(
        &bytes[X_QUERY_EXTENSION_REQ_LEN..X_QUERY_EXTENSION_REQ_LEN + name_len],
    )
    .map_err(|_| XWireParseError::InvalidLength {
        opcode: X_QUERY_EXTENSION,
        expected_at_least: expected_len,
        actual: bytes.len(),
    })?;
    Ok(XWireRequest::QueryExtension {
        name: name.to_owned(),
    })
}

fn decode_list_extensions(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_LIST_EXTENSIONS, X_LIST_EXTENSIONS_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::ListExtensions)
}

fn decode_destroy_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_DESTROY_WINDOW, X_DESTROY_WINDOW_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::DestroyWindow {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
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

fn decode_get_input_focus(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_INPUT_FOCUS, X_GET_INPUT_FOCUS_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GetInputFocus)
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

fn validate_wire_image_format(format: u8) -> Result<(), XWireParseError> {
    match format {
        0..=2 => Ok(()),
        other => Err(XWireParseError::InvalidPropertyFormat(other)),
    }
}

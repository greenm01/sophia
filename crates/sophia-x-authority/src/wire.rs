use sophia_protocol::{
    NamespaceId, PortalTransferId, Rect, Region, SurfaceConstraints, SurfaceId, TransactionId,
};

use crate::{
    XAtom, XAuthorityRequestKind, XAuthorityRequestPacket, XByteOrder, XPropertyChange,
    XPropertyMode, XPropertyRead, XResourceId, XSelectionChangeKind, padded_len,
};

const X_CREATE_WINDOW: u8 = 1;
const X_CHANGE_WINDOW_ATTRIBUTES: u8 = 2;
const X_GET_WINDOW_ATTRIBUTES: u8 = 3;
const X_DESTROY_WINDOW: u8 = 4;
const X_MAP_WINDOW: u8 = 8;
const X_MAP_SUBWINDOWS: u8 = 9;
const X_GET_GEOMETRY: u8 = 14;
const X_QUERY_TREE: u8 = 15;
const X_INTERN_ATOM: u8 = 16;
const X_GET_ATOM_NAME: u8 = 17;
const X_CHANGE_PROPERTY: u8 = 18;
const X_GET_PROPERTY: u8 = 20;
const X_LIST_PROPERTIES: u8 = 21;
const X_SET_SELECTION_OWNER: u8 = 22;
const X_CONVERT_SELECTION: u8 = 24;
const X_TRANSLATE_COORDINATES: u8 = 40;
const X_GET_INPUT_FOCUS: u8 = 43;
const X_OPEN_FONT: u8 = 45;
const X_CLOSE_FONT: u8 = 46;
const X_QUERY_FONT: u8 = 47;
const X_LIST_FONTS: u8 = 49;
const X_LIST_FONTS_WITH_INFO: u8 = 50;
const X_CREATE_PIXMAP: u8 = 53;
const X_FREE_PIXMAP: u8 = 54;
const X_CREATE_GC: u8 = 55;
const X_SET_CLIP_RECTANGLES: u8 = 59;
const X_FREE_GC: u8 = 60;
const X_CLEAR_AREA: u8 = 61;
const X_COPY_AREA: u8 = 62;
const X_POLY_LINE: u8 = 65;
const X_POLY_SEGMENT: u8 = 66;
const X_FILL_POLY: u8 = 69;
const X_POLY_FILL_RECTANGLE: u8 = 70;
const X_POLY_FILL_ARC: u8 = 71;
const X_PUT_IMAGE: u8 = 72;
const X_POLY_TEXT8: u8 = 74;
const X_QUERY_COLORS: u8 = 91;
const X_CREATE_GLYPH_CURSOR: u8 = 94;
const X_FREE_CURSOR: u8 = 95;
const X_QUERY_EXTENSION: u8 = 98;
const X_LIST_EXTENSIONS: u8 = 99;
const X_QUERY_BEST_SIZE: u8 = 97;

pub const X_SOPHIA_PRESENT_EXTENSION_NAME: &str = "SOPHIA-PRESENT";
pub const X_SOPHIA_PRESENT_MAJOR_OPCODE: u8 = 130;
pub const X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE: u8 = 0;
pub const X_MIT_SHM_EXTENSION_NAME: &str = "MIT-SHM";
pub const X_MIT_SHM_MAJOR_OPCODE: u8 = 131;
pub const X_MIT_SHM_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_MIT_SHM_ATTACH_MINOR_OPCODE: u8 = 1;
pub const X_MIT_SHM_DETACH_MINOR_OPCODE: u8 = 2;
pub const X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE: u8 = 3;
pub const X_RANDR_EXTENSION_NAME: &str = "RANDR";
pub const X_RANDR_MAJOR_OPCODE: u8 = 132;
pub const X_RANDR_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_RANDR_GET_SCREEN_SIZE_RANGE_MINOR_OPCODE: u8 = 6;
pub const X_RANDR_GET_SCREEN_RESOURCES_MINOR_OPCODE: u8 = 8;
pub const X_RANDR_GET_SCREEN_RESOURCES_CURRENT_MINOR_OPCODE: u8 = 25;

const X_CREATE_WINDOW_REQ_LEN: usize = 32;
const X_CHANGE_WINDOW_ATTRIBUTES_REQ_LEN: usize = 12;
const X_GET_WINDOW_ATTRIBUTES_REQ_LEN: usize = 8;
const X_DESTROY_WINDOW_REQ_LEN: usize = 8;
const X_MAP_WINDOW_REQ_LEN: usize = 8;
const X_MAP_SUBWINDOWS_REQ_LEN: usize = 8;
const X_GET_GEOMETRY_REQ_LEN: usize = 8;
const X_QUERY_TREE_REQ_LEN: usize = 8;
const X_INTERN_ATOM_REQ_LEN: usize = 8;
const X_GET_ATOM_NAME_REQ_LEN: usize = 8;
const X_CHANGE_PROPERTY_REQ_LEN: usize = 24;
const X_GET_PROPERTY_REQ_LEN: usize = 24;
const X_LIST_PROPERTIES_REQ_LEN: usize = 8;
const X_SET_SELECTION_OWNER_REQ_LEN: usize = 16;
const X_CONVERT_SELECTION_REQ_LEN: usize = 24;
const X_TRANSLATE_COORDINATES_REQ_LEN: usize = 16;
const X_GET_INPUT_FOCUS_REQ_LEN: usize = 4;
const X_OPEN_FONT_REQ_LEN: usize = 12;
const X_CLOSE_FONT_REQ_LEN: usize = 8;
const X_QUERY_FONT_REQ_LEN: usize = 8;
const X_LIST_FONTS_REQ_LEN: usize = 8;
const X_LIST_FONTS_WITH_INFO_REQ_LEN: usize = 8;
const X_CREATE_PIXMAP_REQ_LEN: usize = 16;
const X_FREE_PIXMAP_REQ_LEN: usize = 8;
const X_CREATE_GC_REQ_LEN: usize = 16;
const X_SET_CLIP_RECTANGLES_REQ_LEN: usize = 12;
const X_FREE_GC_REQ_LEN: usize = 8;
const X_CLEAR_AREA_REQ_LEN: usize = 16;
const X_COPY_AREA_REQ_LEN: usize = 28;
const X_POLY_LINE_REQ_LEN: usize = 12;
const X_POLY_SEGMENT_REQ_LEN: usize = 12;
const X_FILL_POLY_REQ_LEN: usize = 16;
const X_POLY_FILL_RECTANGLE_REQ_LEN: usize = 12;
const X_POLY_FILL_ARC_REQ_LEN: usize = 12;
const X_PUT_IMAGE_REQ_LEN: usize = 24;
const X_POLY_TEXT8_REQ_LEN: usize = 16;
const X_QUERY_COLORS_REQ_LEN: usize = 8;
const X_CREATE_GLYPH_CURSOR_REQ_LEN: usize = 32;
const X_FREE_CURSOR_REQ_LEN: usize = 8;
const X_QUERY_EXTENSION_REQ_LEN: usize = 8;
const X_LIST_EXTENSIONS_REQ_LEN: usize = 4;
const X_QUERY_BEST_SIZE_REQ_LEN: usize = 12;
const X_SOPHIA_PRESENT_PIXMAP_REQ_LEN: usize = 32;
const X_MIT_SHM_QUERY_VERSION_REQ_LEN: usize = 4;
const X_MIT_SHM_ATTACH_REQ_LEN: usize = 16;
const X_MIT_SHM_DETACH_REQ_LEN: usize = 8;
const X_MIT_SHM_PUT_IMAGE_REQ_LEN: usize = 40;
const X_RANDR_QUERY_VERSION_REQ_LEN: usize = 12;
const X_RANDR_GET_SCREEN_SIZE_RANGE_REQ_LEN: usize = 8;
const X_RANDR_GET_SCREEN_RESOURCES_REQ_LEN: usize = 8;

pub const X_PUT_IMAGE_MAX_DATA_BYTES: usize = crate::X_PROPERTY_MAX_VALUE_BYTES;
pub const X_QUERY_COLORS_MAX_PIXELS: usize = 256;
pub const X_POLY_TEXT8_MAX_BYTES: usize = crate::X_PROPERTY_MAX_VALUE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub transaction: TransactionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireRequest {
    Authority(XAuthorityRequestPacket),
    ChangeWindowAttributes {
        window: XResourceId,
    },
    GetWindowAttributes {
        window: XResourceId,
    },
    DestroyWindow {
        window: XResourceId,
    },
    MapSubwindows {
        window: XResourceId,
    },
    GetGeometry {
        drawable: XResourceId,
    },
    QueryTree {
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
    ListProperties {
        window: XResourceId,
    },
    CreateGraphicsContext {
        gc: XResourceId,
        drawable: XResourceId,
    },
    SetClipRectangles {
        gc: XResourceId,
        rectangles: Vec<Rect>,
    },
    FreeGraphicsContext {
        gc: XResourceId,
    },
    ClearArea {
        exposures: bool,
        window: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
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
    PolyText8 {
        drawable: XResourceId,
        gc: XResourceId,
        x: i16,
        y: i16,
        glyph_count: usize,
    },
    GetInputFocus,
    OpenFont {
        font: XResourceId,
        name: String,
    },
    CloseFont {
        font: XResourceId,
    },
    QueryFont {
        font: XResourceId,
    },
    ListFonts {
        max_names: u16,
        pattern: String,
    },
    ListFontsWithInfo {
        max_names: u16,
        pattern: String,
    },
    CreatePixmap {
        depth: u8,
        pixmap: XResourceId,
        drawable: XResourceId,
        width: u16,
        height: u16,
    },
    FreePixmap {
        pixmap: XResourceId,
    },
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
    CopyArea {
        source: XResourceId,
        destination: XResourceId,
        gc: XResourceId,
        src_x: i16,
        src_y: i16,
        dst_x: i16,
        dst_y: i16,
        width: u16,
        height: u16,
    },
    PolySegment {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Vec<Rect>,
    },
    PolyLine {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Option<Rect>,
    },
    FillPoly {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Option<Rect>,
    },
    PolyFillArc {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Vec<Rect>,
    },
    ShmQueryVersion,
    ShmAttach {
        segment: XResourceId,
        shmid: u32,
        read_only: bool,
    },
    ShmDetach {
        segment: XResourceId,
    },
    ShmPutImage {
        drawable: XResourceId,
        gc: XResourceId,
        total_width: u16,
        total_height: u16,
        src_x: u16,
        src_y: u16,
        src_width: u16,
        src_height: u16,
        dst_x: i16,
        dst_y: i16,
        depth: u8,
        format: u8,
        send_event: bool,
        segment: XResourceId,
        offset: u32,
    },
    RandrQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    RandrGetScreenSizeRange {
        window: XResourceId,
    },
    RandrGetScreenResources {
        window: XResourceId,
        current: bool,
    },
    QueryColors {
        colormap: XResourceId,
        pixels: Vec<u32>,
    },
    CreateGlyphCursor {
        cursor: XResourceId,
        source_font: XResourceId,
        mask_font: Option<XResourceId>,
    },
    FreeCursor {
        cursor: XResourceId,
    },
    TranslateCoordinates {
        source: XResourceId,
        destination: XResourceId,
        src_x: i16,
        src_y: i16,
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
        X_CHANGE_WINDOW_ATTRIBUTES => decode_change_window_attributes(context, bytes),
        X_GET_WINDOW_ATTRIBUTES => decode_get_window_attributes(context, bytes),
        X_DESTROY_WINDOW => decode_destroy_window(context, bytes),
        X_MAP_WINDOW => decode_map_window(context, bytes),
        X_MAP_SUBWINDOWS => decode_map_subwindows(context, bytes),
        X_GET_GEOMETRY => decode_get_geometry(context, bytes),
        X_QUERY_TREE => decode_query_tree(context, bytes),
        X_INTERN_ATOM => decode_intern_atom(context, bytes),
        X_GET_ATOM_NAME => decode_get_atom_name(context, bytes),
        X_CHANGE_PROPERTY => decode_change_property(context, bytes),
        X_GET_PROPERTY => decode_get_property(context, bytes),
        X_LIST_PROPERTIES => decode_list_properties(context, bytes),
        X_SET_SELECTION_OWNER => decode_set_selection_owner(context, bytes),
        X_CONVERT_SELECTION => decode_convert_selection(context, bytes),
        X_TRANSLATE_COORDINATES => decode_translate_coordinates(context, bytes),
        X_GET_INPUT_FOCUS => decode_get_input_focus(bytes),
        X_OPEN_FONT => decode_open_font(context, bytes),
        X_CLOSE_FONT => decode_close_font(context, bytes),
        X_QUERY_FONT => decode_query_font(context, bytes),
        X_LIST_FONTS => decode_list_fonts(context, bytes),
        X_LIST_FONTS_WITH_INFO => decode_list_fonts_with_info(context, bytes),
        X_CREATE_PIXMAP => decode_create_pixmap(context, bytes),
        X_FREE_PIXMAP => decode_free_pixmap(context, bytes),
        X_CREATE_GC => decode_create_gc(context, bytes),
        X_SET_CLIP_RECTANGLES => decode_set_clip_rectangles(context, bytes),
        X_FREE_GC => decode_free_gc(context, bytes),
        X_CLEAR_AREA => decode_clear_area(context, bytes),
        X_COPY_AREA => decode_copy_area(context, bytes),
        X_POLY_LINE => decode_poly_line(context, bytes),
        X_POLY_SEGMENT => decode_poly_segment(context, bytes),
        X_FILL_POLY => decode_fill_poly(context, bytes),
        X_POLY_FILL_RECTANGLE => decode_poly_fill_rectangle(context, bytes),
        X_POLY_FILL_ARC => decode_poly_fill_arc(context, bytes),
        X_PUT_IMAGE => decode_put_image(context, bytes),
        X_POLY_TEXT8 => decode_poly_text8(context, bytes),
        X_QUERY_COLORS => decode_query_colors(context, bytes),
        X_CREATE_GLYPH_CURSOR => decode_create_glyph_cursor(context, bytes),
        X_FREE_CURSOR => decode_free_cursor(context, bytes),
        X_QUERY_BEST_SIZE => decode_query_best_size(context, bytes),
        X_QUERY_EXTENSION => decode_query_extension(context, bytes),
        X_LIST_EXTENSIONS => decode_list_extensions(bytes),
        X_SOPHIA_PRESENT_MAJOR_OPCODE => decode_sophia_present(context, bytes),
        X_MIT_SHM_MAJOR_OPCODE => decode_mit_shm(context, bytes),
        X_RANDR_MAJOR_OPCODE => decode_randr(context, bytes),
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}

fn decode_randr(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_RANDR_QUERY_VERSION_MINOR_OPCODE => {
            require_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrQueryVersion {
                major_version: context.byte_order.u32(&bytes[4..8]),
                minor_version: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_RANDR_GET_SCREEN_SIZE_RANGE_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_SCREEN_SIZE_RANGE_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetScreenSizeRange {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_RANDR_GET_SCREEN_RESOURCES_MINOR_OPCODE
        | X_RANDR_GET_SCREEN_RESOURCES_CURRENT_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_SCREEN_RESOURCES_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetScreenResources {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                current: bytes[1] == X_RANDR_GET_SCREEN_RESOURCES_CURRENT_MINOR_OPCODE,
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[0])),
    }
}

fn decode_mit_shm(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_MIT_SHM_QUERY_VERSION_MINOR_OPCODE => {
            require_exact_len(
                X_MIT_SHM_MAJOR_OPCODE,
                X_MIT_SHM_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::ShmQueryVersion)
        }
        X_MIT_SHM_ATTACH_MINOR_OPCODE => {
            require_exact_len(
                X_MIT_SHM_MAJOR_OPCODE,
                X_MIT_SHM_ATTACH_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::ShmAttach {
                segment: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                shmid: context.byte_order.u32(&bytes[8..12]),
                read_only: bytes[12] != 0,
            })
        }
        X_MIT_SHM_DETACH_MINOR_OPCODE => {
            require_exact_len(
                X_MIT_SHM_MAJOR_OPCODE,
                X_MIT_SHM_DETACH_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::ShmDetach {
                segment: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE => {
            require_exact_len(
                X_MIT_SHM_MAJOR_OPCODE,
                X_MIT_SHM_PUT_IMAGE_REQ_LEN,
                bytes.len(),
            )?;
            validate_wire_image_format(bytes[29])?;
            Ok(XWireRequest::ShmPutImage {
                drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                total_width: context.byte_order.u16(&bytes[12..14]),
                total_height: context.byte_order.u16(&bytes[14..16]),
                src_x: context.byte_order.u16(&bytes[16..18]),
                src_y: context.byte_order.u16(&bytes[18..20]),
                src_width: context.byte_order.u16(&bytes[20..22]),
                src_height: context.byte_order.u16(&bytes[22..24]),
                dst_x: context.byte_order.i16(&bytes[24..26]),
                dst_y: context.byte_order.i16(&bytes[26..28]),
                depth: bytes[28],
                format: bytes[29],
                send_event: bytes[30] != 0,
                segment: XResourceId::new(u64::from(context.byte_order.u32(&bytes[32..36])), 1),
                offset: context.byte_order.u32(&bytes[36..40]),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[0])),
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

fn decode_poly_text8(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_TEXT8, X_POLY_TEXT8_REQ_LEN, bytes.len())?;
    let item_bytes = &bytes[X_POLY_TEXT8_REQ_LEN..];
    if item_bytes.len() > X_POLY_TEXT8_MAX_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: item_bytes.len(),
            max: X_POLY_TEXT8_MAX_BYTES,
        });
    }

    let mut offset = 0usize;
    let mut glyph_count = 0usize;
    while offset < item_bytes.len() {
        let len = item_bytes[offset];
        offset += 1;
        if len == u8::MAX {
            if item_bytes.len().saturating_sub(offset) < 4 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_POLY_TEXT8,
                    expected_at_least: X_POLY_TEXT8_REQ_LEN + offset + 4,
                    actual: bytes.len(),
                });
            }
            offset += 4;
            continue;
        }

        let item_len = 1usize + usize::from(len);
        if item_bytes.len().saturating_sub(offset) < item_len {
            return Err(XWireParseError::InvalidLength {
                opcode: X_POLY_TEXT8,
                expected_at_least: X_POLY_TEXT8_REQ_LEN + offset + item_len,
                actual: bytes.len(),
            });
        }
        offset += item_len;
        glyph_count = glyph_count.saturating_add(usize::from(len));
    }

    Ok(XWireRequest::PolyText8 {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        x: context.byte_order.i16(&bytes[12..14]),
        y: context.byte_order.i16(&bytes[14..16]),
        glyph_count,
    })
}

fn decode_copy_area(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_COPY_AREA, X_COPY_AREA_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::CopyArea {
        source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        destination: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[12..16])), 1),
        src_x: context.byte_order.i16(&bytes[16..18]),
        src_y: context.byte_order.i16(&bytes[18..20]),
        dst_x: context.byte_order.i16(&bytes[20..22]),
        dst_y: context.byte_order.i16(&bytes[22..24]),
        width: context.byte_order.u16(&bytes[24..26]),
        height: context.byte_order.u16(&bytes[26..28]),
    })
}

fn decode_query_tree(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_QUERY_TREE, X_QUERY_TREE_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::QueryTree {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_get_window_attributes(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_GET_WINDOW_ATTRIBUTES,
        X_GET_WINDOW_ATTRIBUTES_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::GetWindowAttributes {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_translate_coordinates(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_TRANSLATE_COORDINATES,
        X_TRANSLATE_COORDINATES_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::TranslateCoordinates {
        source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        destination: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        src_x: context.byte_order.i16(&bytes[12..14]),
        src_y: context.byte_order.i16(&bytes[14..16]),
    })
}

fn decode_get_geometry(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_GEOMETRY, X_GET_GEOMETRY_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GetGeometry {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_clear_area(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_CLEAR_AREA, X_CLEAR_AREA_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::ClearArea {
        exposures: bytes[1] != 0,
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        x: context.byte_order.i16(&bytes[8..10]),
        y: context.byte_order.i16(&bytes[10..12]),
        width: context.byte_order.u16(&bytes[12..14]),
        height: context.byte_order.u16(&bytes[14..16]),
    })
}

fn decode_query_colors(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_QUERY_COLORS, X_QUERY_COLORS_REQ_LEN, bytes.len())?;
    let pixel_bytes = &bytes[X_QUERY_COLORS_REQ_LEN..];
    if pixel_bytes.len() % 4 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_QUERY_COLORS,
            expected_at_least: X_QUERY_COLORS_REQ_LEN + ((pixel_bytes.len() + 3) & !3),
            actual: bytes.len(),
        });
    }
    if pixel_bytes.len() / 4 > X_QUERY_COLORS_MAX_PIXELS {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: pixel_bytes.len(),
            max: X_QUERY_COLORS_MAX_PIXELS * 4,
        });
    }

    Ok(XWireRequest::QueryColors {
        colormap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        pixels: pixel_bytes
            .chunks_exact(4)
            .map(|pixel| context.byte_order.u32(pixel))
            .collect(),
    })
}

fn decode_poly_segment(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_SEGMENT, X_POLY_SEGMENT_REQ_LEN, bytes.len())?;
    let segment_bytes = &bytes[X_POLY_SEGMENT_REQ_LEN..];
    if segment_bytes.len() % 8 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_POLY_SEGMENT,
            expected_at_least: X_POLY_SEGMENT_REQ_LEN + ((segment_bytes.len() + 7) & !7),
            actual: bytes.len(),
        });
    }
    let mut damage = Vec::with_capacity(segment_bytes.len() / 8);
    for segment in segment_bytes.chunks_exact(8) {
        let x1 = i32::from(context.byte_order.i16(&segment[0..2]));
        let y1 = i32::from(context.byte_order.i16(&segment[2..4]));
        let x2 = i32::from(context.byte_order.i16(&segment[4..6]));
        let y2 = i32::from(context.byte_order.i16(&segment[6..8]));
        let x = x1.min(x2);
        let y = y1.min(y2);
        damage.push(Rect {
            x,
            y,
            width: x1.max(x2).saturating_sub(x).saturating_add(1),
            height: y1.max(y2).saturating_sub(y).saturating_add(1),
        });
    }
    Ok(XWireRequest::PolySegment {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage,
    })
}

fn decode_poly_line(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_LINE, X_POLY_LINE_REQ_LEN, bytes.len())?;
    let point_bytes = &bytes[X_POLY_LINE_REQ_LEN..];
    if point_bytes.len() % 4 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_POLY_LINE,
            expected_at_least: X_POLY_LINE_REQ_LEN + ((point_bytes.len() + 3) & !3),
            actual: bytes.len(),
        });
    }

    Ok(XWireRequest::PolyLine {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage: point_damage_bounds(context, point_bytes),
    })
}

fn decode_fill_poly(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_FILL_POLY, X_FILL_POLY_REQ_LEN, bytes.len())?;
    let point_bytes = &bytes[X_FILL_POLY_REQ_LEN..];
    if point_bytes.len() % 4 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_FILL_POLY,
            expected_at_least: X_FILL_POLY_REQ_LEN + ((point_bytes.len() + 3) & !3),
            actual: bytes.len(),
        });
    }

    Ok(XWireRequest::FillPoly {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage: point_damage_bounds(context, point_bytes),
    })
}

fn point_damage_bounds(context: XWireClientContext, point_bytes: &[u8]) -> Option<Rect> {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for point in point_bytes.chunks_exact(4) {
        let x = i32::from(context.byte_order.i16(&point[0..2]));
        let y = i32::from(context.byte_order.i16(&point[2..4]));
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if min_x == i32::MAX {
        None
    } else {
        Some(Rect {
            x: min_x,
            y: min_y,
            width: max_x.saturating_sub(min_x).saturating_add(1),
            height: max_y.saturating_sub(min_y).saturating_add(1),
        })
    }
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

fn decode_create_glyph_cursor(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_CREATE_GLYPH_CURSOR,
        X_CREATE_GLYPH_CURSOR_REQ_LEN,
        bytes.len(),
    )?;
    let mask_font = context.byte_order.u32(&bytes[12..16]);
    Ok(XWireRequest::CreateGlyphCursor {
        cursor: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        source_font: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        mask_font: (mask_font != 0).then(|| XResourceId::new(u64::from(mask_font), 1)),
    })
}

fn decode_free_cursor(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_FREE_CURSOR, X_FREE_CURSOR_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::FreeCursor {
        cursor: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_set_clip_rectangles(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(
        X_SET_CLIP_RECTANGLES,
        X_SET_CLIP_RECTANGLES_REQ_LEN,
        bytes.len(),
    )?;
    let rectangle_bytes = &bytes[X_SET_CLIP_RECTANGLES_REQ_LEN..];
    if rectangle_bytes.len() % 8 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_SET_CLIP_RECTANGLES,
            expected_at_least: X_SET_CLIP_RECTANGLES_REQ_LEN + ((rectangle_bytes.len() + 7) & !7),
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
    Ok(XWireRequest::SetClipRectangles {
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        rectangles,
    })
}

fn decode_poly_fill_arc(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_POLY_FILL_ARC, X_POLY_FILL_ARC_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::PolyFillArc {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        damage: arc_damage_bounds(context, X_POLY_FILL_ARC, X_POLY_FILL_ARC_REQ_LEN, bytes)?,
    })
}

fn arc_damage_bounds(
    context: XWireClientContext,
    opcode: u8,
    header_len: usize,
    bytes: &[u8],
) -> Result<Vec<Rect>, XWireParseError> {
    let arc_bytes = &bytes[header_len..];
    if arc_bytes.len() % 12 != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least: header_len + ((arc_bytes.len() + 11) / 12) * 12,
            actual: bytes.len(),
        });
    }

    let mut damage = Vec::with_capacity(arc_bytes.len() / 12);
    for arc in arc_bytes.chunks_exact(12) {
        damage.push(Rect {
            x: i32::from(context.byte_order.i16(&arc[0..2])),
            y: i32::from(context.byte_order.i16(&arc[2..4])),
            width: i32::from(context.byte_order.u16(&arc[4..6])),
            height: i32::from(context.byte_order.u16(&arc[6..8])),
        });
    }
    Ok(damage)
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

fn decode_create_pixmap(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_CREATE_PIXMAP, X_CREATE_PIXMAP_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::CreatePixmap {
        depth: bytes[1],
        pixmap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        width: context.byte_order.u16(&bytes[12..14]),
        height: context.byte_order.u16(&bytes[14..16]),
    })
}

fn decode_open_font(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_OPEN_FONT, X_OPEN_FONT_REQ_LEN, bytes.len())?;
    let name_len = usize::from(context.byte_order.u16(&bytes[8..10]));
    let expected_len = X_OPEN_FONT_REQ_LEN + padded_len(name_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_OPEN_FONT,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let name = core::str::from_utf8(&bytes[X_OPEN_FONT_REQ_LEN..X_OPEN_FONT_REQ_LEN + name_len])
        .map_err(|_| XWireParseError::InvalidLength {
            opcode: X_OPEN_FONT,
            expected_at_least: expected_len,
            actual: bytes.len(),
        })?;
    Ok(XWireRequest::OpenFont {
        font: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        name: name.to_owned(),
    })
}

fn decode_close_font(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_CLOSE_FONT, X_CLOSE_FONT_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::CloseFont {
        font: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_query_font(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_QUERY_FONT, X_QUERY_FONT_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::QueryFont {
        font: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_list_fonts(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_LIST_FONTS, X_LIST_FONTS_REQ_LEN, bytes.len())?;
    let pattern_len = usize::from(context.byte_order.u16(&bytes[6..8]));
    let expected_len = X_LIST_FONTS_REQ_LEN + padded_len(pattern_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_LIST_FONTS,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let pattern =
        core::str::from_utf8(&bytes[X_LIST_FONTS_REQ_LEN..X_LIST_FONTS_REQ_LEN + pattern_len])
            .map_err(|_| XWireParseError::InvalidLength {
                opcode: X_LIST_FONTS,
                expected_at_least: expected_len,
                actual: bytes.len(),
            })?;
    Ok(XWireRequest::ListFonts {
        max_names: context.byte_order.u16(&bytes[4..6]),
        pattern: pattern.to_owned(),
    })
}

fn decode_list_fonts_with_info(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(
        X_LIST_FONTS_WITH_INFO,
        X_LIST_FONTS_WITH_INFO_REQ_LEN,
        bytes.len(),
    )?;
    let pattern_len = usize::from(context.byte_order.u16(&bytes[6..8]));
    let expected_len = X_LIST_FONTS_WITH_INFO_REQ_LEN + padded_len(pattern_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_LIST_FONTS_WITH_INFO,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let pattern = core::str::from_utf8(
        &bytes[X_LIST_FONTS_WITH_INFO_REQ_LEN..X_LIST_FONTS_WITH_INFO_REQ_LEN + pattern_len],
    )
    .map_err(|_| XWireParseError::InvalidLength {
        opcode: X_LIST_FONTS_WITH_INFO,
        expected_at_least: expected_len,
        actual: bytes.len(),
    })?;
    Ok(XWireRequest::ListFontsWithInfo {
        max_names: context.byte_order.u16(&bytes[4..6]),
        pattern: pattern.to_owned(),
    })
}

fn decode_free_pixmap(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_FREE_PIXMAP, X_FREE_PIXMAP_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::FreePixmap {
        pixmap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
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

fn decode_list_properties(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_LIST_PROPERTIES, X_LIST_PROPERTIES_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::ListProperties {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
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

fn decode_change_window_attributes(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(
        X_CHANGE_WINDOW_ATTRIBUTES,
        X_CHANGE_WINDOW_ATTRIBUTES_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::ChangeWindowAttributes {
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

fn decode_map_subwindows(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_MAP_SUBWINDOWS, X_MAP_SUBWINDOWS_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::MapSubwindows {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
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

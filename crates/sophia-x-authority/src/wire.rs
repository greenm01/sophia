use sophia_protocol::{
    NamespaceId, PortalTransferId, Rect, Region, SurfaceConstraints, SurfaceId, TransactionId,
};

use crate::{
    XAtom, XAuthorityRequestKind, XAuthorityRequestPacket, XByteOrder, XClientEvent,
    XGraphicsContextValues, XPoint, XPropertyChange, XPropertyMode, XPropertyRead, XResourceId,
    XSelectionChangeKind, padded_len,
};

const X_CREATE_WINDOW: u8 = 1;
const X_CHANGE_WINDOW_ATTRIBUTES: u8 = 2;
const X_GET_WINDOW_ATTRIBUTES: u8 = 3;
const X_DESTROY_WINDOW: u8 = 4;
const X_REPARENT_WINDOW: u8 = 7;
const X_MAP_WINDOW: u8 = 8;
const X_MAP_SUBWINDOWS: u8 = 9;
const X_UNMAP_WINDOW: u8 = 10;
const X_CONFIGURE_WINDOW: u8 = 12;
const X_GET_GEOMETRY: u8 = 14;
const X_QUERY_TREE: u8 = 15;
const X_INTERN_ATOM: u8 = 16;
const X_GET_ATOM_NAME: u8 = 17;
const X_CHANGE_PROPERTY: u8 = 18;
const X_DELETE_PROPERTY: u8 = 19;
const X_GET_PROPERTY: u8 = 20;
const X_QUERY_POINTER: u8 = 38;
const X_LIST_PROPERTIES: u8 = 21;
const X_SET_SELECTION_OWNER: u8 = 22;
const X_GET_SELECTION_OWNER: u8 = 23;
const X_CONVERT_SELECTION: u8 = 24;
const X_SEND_EVENT: u8 = 25;
const X_GRAB_POINTER: u8 = 26;
const X_UNGRAB_POINTER: u8 = 27;
const X_GRAB_BUTTON: u8 = 28;
const X_UNGRAB_BUTTON: u8 = 29;
const X_GRAB_KEYBOARD: u8 = 31;
const X_UNGRAB_KEYBOARD: u8 = 32;
const X_GRAB_KEY: u8 = 33;
const X_UNGRAB_KEY: u8 = 34;
const X_ALLOW_EVENTS: u8 = 35;
const X_GRAB_SERVER: u8 = 36;
const X_UNGRAB_SERVER: u8 = 37;
const X_TRANSLATE_COORDINATES: u8 = 40;
const X_SET_INPUT_FOCUS: u8 = 42;
const X_GET_INPUT_FOCUS: u8 = 43;
const X_OPEN_FONT: u8 = 45;
const X_CLOSE_FONT: u8 = 46;
const X_QUERY_FONT: u8 = 47;
const X_LIST_FONTS: u8 = 49;
const X_LIST_FONTS_WITH_INFO: u8 = 50;
const X_CREATE_PIXMAP: u8 = 53;
const X_FREE_PIXMAP: u8 = 54;
const X_CREATE_GC: u8 = 55;
const X_CHANGE_GC: u8 = 56;
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
const X_GET_IMAGE: u8 = 73;
const X_POLY_TEXT8: u8 = 74;
const X_IMAGE_TEXT8: u8 = 76;
const X_CREATE_COLORMAP: u8 = 78;
const X_FREE_COLORMAP: u8 = 79;
const X_ALLOC_COLOR: u8 = 84;
const X_ALLOC_NAMED_COLOR: u8 = 85;
const X_QUERY_COLORS: u8 = 91;
const X_CREATE_CURSOR: u8 = 93;
const X_CREATE_GLYPH_CURSOR: u8 = 94;
const X_FREE_CURSOR: u8 = 95;
const X_RECOLOR_CURSOR: u8 = 96;
const X_QUERY_EXTENSION: u8 = 98;
const X_LIST_EXTENSIONS: u8 = 99;
const X_GET_KEYBOARD_MAPPING: u8 = 101;
const X_GET_KEYBOARD_CONTROL: u8 = 103;
const X_BELL: u8 = 104;
const X_QUERY_BEST_SIZE: u8 = 97;
const X_GET_MODIFIER_MAPPING: u8 = 119;

pub const X_SOPHIA_PRESENT_EXTENSION_NAME: &str = "SOPHIA-PRESENT";
pub const X_SOPHIA_PRESENT_MAJOR_OPCODE: u8 = 130;
pub const X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE: u8 = 0;
pub const X_MIT_SHM_EXTENSION_NAME: &str = "MIT-SHM";
pub const X_MIT_SHM_MAJOR_OPCODE: u8 = 131;
pub const X_MIT_SHM_FIRST_EVENT: u8 = 112;
pub const X_MIT_SHM_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_MIT_SHM_ATTACH_MINOR_OPCODE: u8 = 1;
pub const X_MIT_SHM_DETACH_MINOR_OPCODE: u8 = 2;
pub const X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE: u8 = 3;
pub const X_MIT_SHM_CREATE_PIXMAP_MINOR_OPCODE: u8 = 5;
pub const X_RANDR_EXTENSION_NAME: &str = "RANDR";
pub const X_RANDR_MAJOR_OPCODE: u8 = 132;
pub const X_RANDR_FIRST_EVENT: u8 = 64;
pub const X_RANDR_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_RANDR_SELECT_INPUT_MINOR_OPCODE: u8 = 4;
pub const X_RANDR_GET_SCREEN_SIZE_RANGE_MINOR_OPCODE: u8 = 6;
pub const X_RANDR_GET_SCREEN_RESOURCES_MINOR_OPCODE: u8 = 8;
pub const X_RANDR_GET_OUTPUT_INFO_MINOR_OPCODE: u8 = 9;
pub const X_RANDR_GET_OUTPUT_PROPERTY_MINOR_OPCODE: u8 = 15;
pub const X_RANDR_GET_CRTC_INFO_MINOR_OPCODE: u8 = 20;
pub const X_RANDR_GET_CRTC_GAMMA_SIZE_MINOR_OPCODE: u8 = 22;
pub const X_RANDR_GET_SCREEN_RESOURCES_CURRENT_MINOR_OPCODE: u8 = 25;
pub const X_RANDR_GET_OUTPUT_PRIMARY_MINOR_OPCODE: u8 = 31;
pub const X_RANDR_GET_PROVIDERS_MINOR_OPCODE: u8 = 32;
pub const X_RANDR_GET_MONITORS_MINOR_OPCODE: u8 = 42;
pub const X_KEYBOARD_EXTENSION_NAME: &str = "XKEYBOARD";
pub const X_KEYBOARD_MAJOR_OPCODE: u8 = 133;
pub const X_KEYBOARD_FIRST_EVENT: u8 = 80;
pub const X_KEYBOARD_USE_EXTENSION_MINOR_OPCODE: u8 = 0;
pub const X_KEYBOARD_SELECT_EVENTS_MINOR_OPCODE: u8 = 1;
pub const X_KEYBOARD_GET_STATE_MINOR_OPCODE: u8 = 4;
pub const X_KEYBOARD_GET_CONTROLS_MINOR_OPCODE: u8 = 6;
pub const X_KEYBOARD_GET_MAP_MINOR_OPCODE: u8 = 8;
pub const X_KEYBOARD_GET_COMPAT_MAP_MINOR_OPCODE: u8 = 10;
pub const X_KEYBOARD_GET_INDICATOR_MAP_MINOR_OPCODE: u8 = 13;
pub const X_KEYBOARD_GET_NAMES_MINOR_OPCODE: u8 = 17;
pub const X_KEYBOARD_PER_CLIENT_FLAGS_MINOR_OPCODE: u8 = 21;
pub const X_KEYBOARD_GET_DEVICE_INFO_MINOR_OPCODE: u8 = 24;
pub const X_BIG_REQUESTS_EXTENSION_NAME: &str = "BIG-REQUESTS";
pub const X_BIG_REQUESTS_MAJOR_OPCODE: u8 = 134;
pub const X_BIG_REQUESTS_ENABLE_MINOR_OPCODE: u8 = 0;
pub const X_INPUT_EXTENSION_NAME: &str = "XInputExtension";
pub const X_INPUT_MAJOR_OPCODE: u8 = 135;
pub const X_INPUT_FIRST_EVENT: u8 = 96;
pub const X_INPUT_FIRST_ERROR: u8 = 160;
pub const X_INPUT_GET_EXTENSION_VERSION_MINOR_OPCODE: u8 = 1;
pub const X_INPUT_DEVICE_BELL_MINOR_OPCODE: u8 = 32;
pub const X_INPUT_QUERY_POINTER_MINOR_OPCODE: u8 = 40;
pub const X_INPUT_CHANGE_CURSOR_MINOR_OPCODE: u8 = 42;
const X_INPUT_QUERY_POINTER_REQ_LEN: usize = 12;
const X_INPUT_CHANGE_CURSOR_REQ_LEN: usize = 16;
const X_INPUT_UNGRAB_DEVICE_REQ_LEN: usize = 12;
pub const X_INPUT_SELECT_EVENTS_MINOR_OPCODE: u8 = 46;
pub const X_INPUT_GET_CLIENT_POINTER_MINOR_OPCODE: u8 = 45;
pub const X_INPUT_QUERY_VERSION_MINOR_OPCODE: u8 = 47;
pub const X_INPUT_QUERY_DEVICE_MINOR_OPCODE: u8 = 48;
pub const X_INPUT_GET_FOCUS_MINOR_OPCODE: u8 = 50;
pub const X_INPUT_UNGRAB_DEVICE_MINOR_OPCODE: u8 = 52;
pub const X_INPUT_GET_PROPERTY_MINOR_OPCODE: u8 = 59;
pub const X_GENERIC_EVENT_EXTENSION_NAME: &str = "Generic Event Extension";
pub const X_GENERIC_EVENT_MAJOR_OPCODE: u8 = 136;
pub const X_GENERIC_EVENT_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_DRI3_EXTENSION_NAME: &str = "DRI3";
pub const X_DRI3_MAJOR_OPCODE: u8 = 137;
pub const X_DRI3_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_DRI3_OPEN_MINOR_OPCODE: u8 = 1;
pub const X_DRI3_PIXMAP_FROM_BUFFER_MINOR_OPCODE: u8 = 2;
pub const X_DRI3_FENCE_FROM_FD_MINOR_OPCODE: u8 = 4;
pub const X_DRI3_GET_SUPPORTED_MODIFIERS_MINOR_OPCODE: u8 = 6;
pub const X_DRI3_PIXMAP_FROM_BUFFERS_MINOR_OPCODE: u8 = 7;
pub const X_PRESENT_EXTENSION_NAME: &str = "Present";
pub const X_PRESENT_MAJOR_OPCODE: u8 = 138;
pub const X_PRESENT_FIRST_EVENT: u8 = 0;
pub const X_PRESENT_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_PRESENT_PIXMAP_MINOR_OPCODE: u8 = 1;
pub const X_PRESENT_SELECT_INPUT_MINOR_OPCODE: u8 = 3;
pub const X_PRESENT_QUERY_CAPABILITIES_MINOR_OPCODE: u8 = 4;
pub const X_XFIXES_EXTENSION_NAME: &str = "XFIXES";
pub const X_XFIXES_MAJOR_OPCODE: u8 = 139;
pub const X_XFIXES_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE: u8 = 2;
pub const X_XFIXES_CREATE_REGION_MINOR_OPCODE: u8 = 5;
pub const X_XFIXES_DESTROY_REGION_MINOR_OPCODE: u8 = 10;
pub const X_XFIXES_SET_REGION_MINOR_OPCODE: u8 = 11;
pub const X_GLX_EXTENSION_NAME: &str = "GLX";
pub const X_GLX_MAJOR_OPCODE: u8 = 140;
pub const X_GLX_DESTROY_CONTEXT_MINOR_OPCODE: u8 = 4;
pub const X_GLX_IS_DIRECT_MINOR_OPCODE: u8 = 6;
pub const X_GLX_QUERY_VERSION_MINOR_OPCODE: u8 = 7;
pub const X_GLX_GET_VISUAL_CONFIGS_MINOR_OPCODE: u8 = 14;
pub const X_GLX_QUERY_EXTENSIONS_STRING_MINOR_OPCODE: u8 = 18;
pub const X_GLX_QUERY_SERVER_STRING_MINOR_OPCODE: u8 = 19;
pub const X_GLX_CLIENT_INFO_MINOR_OPCODE: u8 = 20;
pub const X_GLX_GET_FB_CONFIGS_MINOR_OPCODE: u8 = 21;
pub const X_GLX_CREATE_NEW_CONTEXT_MINOR_OPCODE: u8 = 24;
pub const X_GLX_GET_DRAWABLE_ATTRIBUTES_MINOR_OPCODE: u8 = 29;
pub const X_GLX_CREATE_WINDOW_MINOR_OPCODE: u8 = 31;
pub const X_GLX_DELETE_WINDOW_MINOR_OPCODE: u8 = 32;
pub const X_GLX_SET_CLIENT_INFO_ARB_MINOR_OPCODE: u8 = 33;
pub const X_GLX_CREATE_CONTEXT_ATTRIBS_ARB_MINOR_OPCODE: u8 = 34;
pub const X_GLX_SET_CLIENT_INFO_2_ARB_MINOR_OPCODE: u8 = 35;
pub const X_SYNC_EXTENSION_NAME: &str = "SYNC";
pub const X_SYNC_MAJOR_OPCODE: u8 = 141;
pub const X_SYNC_INITIALIZE_MINOR_OPCODE: u8 = 0;
pub const X_SYNC_DESTROY_FENCE_MINOR_OPCODE: u8 = 17;

const X_CREATE_WINDOW_REQ_LEN: usize = 32;
const X_CHANGE_WINDOW_ATTRIBUTES_REQ_LEN: usize = 12;
const X_GET_WINDOW_ATTRIBUTES_REQ_LEN: usize = 8;
const X_DESTROY_WINDOW_REQ_LEN: usize = 8;
const X_REPARENT_WINDOW_REQ_LEN: usize = 16;
const X_MAP_WINDOW_REQ_LEN: usize = 8;
const X_MAP_SUBWINDOWS_REQ_LEN: usize = 8;
const X_UNMAP_WINDOW_REQ_LEN: usize = 8;
const X_CONFIGURE_WINDOW_REQ_LEN: usize = 12;
const X_GET_GEOMETRY_REQ_LEN: usize = 8;
const X_QUERY_TREE_REQ_LEN: usize = 8;
const X_INTERN_ATOM_REQ_LEN: usize = 8;
const X_GET_ATOM_NAME_REQ_LEN: usize = 8;
const X_CHANGE_PROPERTY_REQ_LEN: usize = 24;
const X_DELETE_PROPERTY_REQ_LEN: usize = 12;
const X_GET_PROPERTY_REQ_LEN: usize = 24;
const X_QUERY_POINTER_REQ_LEN: usize = 8;
const X_LIST_PROPERTIES_REQ_LEN: usize = 8;
const X_SET_SELECTION_OWNER_REQ_LEN: usize = 16;
const X_GET_SELECTION_OWNER_REQ_LEN: usize = 8;
const X_CONVERT_SELECTION_REQ_LEN: usize = 24;
const X_SEND_EVENT_REQ_LEN: usize = 44;
const X_GRAB_BUTTON_REQ_LEN: usize = 24;
const X_UNGRAB_BUTTON_REQ_LEN: usize = 12;
const X_GRAB_POINTER_REQ_LEN: usize = 24;
const X_UNGRAB_POINTER_REQ_LEN: usize = 8;
const X_GRAB_KEYBOARD_REQ_LEN: usize = 16;
const X_UNGRAB_KEYBOARD_REQ_LEN: usize = 8;
const X_GRAB_KEY_REQ_LEN: usize = 16;
const X_UNGRAB_KEY_REQ_LEN: usize = 12;
const X_ALLOW_EVENTS_REQ_LEN: usize = 8;
const X_GRAB_SERVER_REQ_LEN: usize = 4;
const X_UNGRAB_SERVER_REQ_LEN: usize = 4;
const X_TRANSLATE_COORDINATES_REQ_LEN: usize = 16;
const X_SET_INPUT_FOCUS_REQ_LEN: usize = 12;
const X_GET_INPUT_FOCUS_REQ_LEN: usize = 4;
const X_GET_IMAGE_REQ_LEN: usize = 20;
const X_OPEN_FONT_REQ_LEN: usize = 12;
const X_CLOSE_FONT_REQ_LEN: usize = 8;
const X_QUERY_FONT_REQ_LEN: usize = 8;
const X_LIST_FONTS_REQ_LEN: usize = 8;
const X_LIST_FONTS_WITH_INFO_REQ_LEN: usize = 8;
const X_CREATE_PIXMAP_REQ_LEN: usize = 16;
const X_FREE_PIXMAP_REQ_LEN: usize = 8;
const X_CREATE_GC_REQ_LEN: usize = 16;
const X_CHANGE_GC_REQ_LEN: usize = 12;
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
const X_IMAGE_TEXT8_REQ_LEN: usize = 16;
const X_CREATE_COLORMAP_REQ_LEN: usize = 16;
const X_FREE_COLORMAP_REQ_LEN: usize = 8;
const X_ALLOC_COLOR_REQ_LEN: usize = 16;
const X_ALLOC_NAMED_COLOR_REQ_LEN: usize = 12;
const X_QUERY_COLORS_REQ_LEN: usize = 8;
const X_CREATE_CURSOR_REQ_LEN: usize = 32;
const X_CREATE_GLYPH_CURSOR_REQ_LEN: usize = 32;
const X_FREE_CURSOR_REQ_LEN: usize = 8;
const X_RECOLOR_CURSOR_REQ_LEN: usize = 20;
const X_QUERY_EXTENSION_REQ_LEN: usize = 8;
const X_LIST_EXTENSIONS_REQ_LEN: usize = 4;
const X_GET_KEYBOARD_MAPPING_REQ_LEN: usize = 8;
const X_QUERY_BEST_SIZE_REQ_LEN: usize = 12;
const X_GET_MODIFIER_MAPPING_REQ_LEN: usize = 4;
const X_SOPHIA_PRESENT_PIXMAP_REQ_LEN: usize = 32;
const X_MIT_SHM_QUERY_VERSION_REQ_LEN: usize = 4;
const X_MIT_SHM_ATTACH_REQ_LEN: usize = 16;
const X_MIT_SHM_DETACH_REQ_LEN: usize = 8;
const X_MIT_SHM_PUT_IMAGE_REQ_LEN: usize = 40;
const X_MIT_SHM_CREATE_PIXMAP_REQ_LEN: usize = 28;
const X_RANDR_QUERY_VERSION_REQ_LEN: usize = 12;
const X_RANDR_SELECT_INPUT_REQ_LEN: usize = 12;
const X_RANDR_GET_SCREEN_SIZE_RANGE_REQ_LEN: usize = 8;
const X_RANDR_GET_SCREEN_RESOURCES_REQ_LEN: usize = 8;
const X_RANDR_GET_OUTPUT_INFO_REQ_LEN: usize = 12;
const X_RANDR_GET_OUTPUT_PROPERTY_REQ_LEN: usize = 28;
const X_RANDR_GET_CRTC_INFO_REQ_LEN: usize = 12;
const X_RANDR_GET_CRTC_GAMMA_SIZE_REQ_LEN: usize = 8;
const X_RANDR_GET_OUTPUT_PRIMARY_REQ_LEN: usize = 8;
const X_RANDR_GET_MONITORS_REQ_LEN: usize = 12;
const X_KEYBOARD_USE_EXTENSION_REQ_LEN: usize = 8;
const X_KEYBOARD_SELECT_EVENTS_REQ_LEN: usize = 16;
const X_KEYBOARD_GET_MAP_REQ_LEN: usize = 28;
const X_KEYBOARD_GET_CONTROLS_REQ_LEN: usize = 8;
const X_KEYBOARD_PER_CLIENT_FLAGS_REQ_LEN: usize = 28;
const X_BIG_REQUESTS_ENABLE_REQ_LEN: usize = 4;
const X_INPUT_QUERY_VERSION_REQ_LEN: usize = 8;
const X_INPUT_GET_CLIENT_POINTER_REQ_LEN: usize = 8;
const X_INPUT_QUERY_DEVICE_REQ_LEN: usize = 8;
const X_INPUT_SELECT_EVENTS_REQ_LEN: usize = 12;
const X_INPUT_GET_FOCUS_REQ_LEN: usize = 8;
const X_INPUT_GET_PROPERTY_REQ_LEN: usize = 24;
const X_GENERIC_EVENT_QUERY_VERSION_REQ_LEN: usize = 8;

pub const X_PUT_IMAGE_MAX_DATA_BYTES: usize = 256 * 1024;
pub const X_QUERY_COLORS_MAX_PIXELS: usize = 256;
pub const X_POLY_TEXT8_MAX_BYTES: usize = 64 * 1024;
pub const X_IMAGE_TEXT8_MAX_BYTES: usize = 64 * 1024;
pub const X_ALLOC_NAMED_COLOR_MAX_NAME_BYTES: usize = 256;

/// The XID range granted to one X11 client during connection setup.
///
/// Server-owned resources such as the root window are intentionally outside
/// this range. It therefore applies only when a request creates a new client
/// resource, not when it references an existing drawable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientResourceRange {
    pub base: u32,
    pub mask: u32,
}

impl XWireClientResourceRange {
    pub const fn owns_new_resource(self, resource_id: u32) -> bool {
        resource_id != 0 && (resource_id & !self.mask) == self.base
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub transaction: TransactionId,
    /// `None` preserves deterministic decoder fixtures that are not attached
    /// to a live X11 setup. Socket clients must always provide their range.
    pub resource_id_range: Option<XWireClientResourceRange>,
}

impl XWireClientContext {
    fn validate_new_resource_id(self, resource_id: u32) -> Result<(), XWireParseError> {
        if self
            .resource_id_range
            .is_some_and(|range| !range.owns_new_resource(resource_id))
        {
            return Err(XWireParseError::ResourceIdOutsideClientRange { resource_id });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireRequest {
    Authority(XAuthorityRequestPacket),
    CreateWindow {
        packet: XAuthorityRequestPacket,
        parent: XResourceId,
        depth: u8,
        visual: u32,
        colormap: Option<XResourceId>,
        background_pixel: Option<u32>,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    },
    ChangeWindowAttributes {
        window: XResourceId,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    },
    GetWindowAttributes {
        window: XResourceId,
    },
    DestroyWindow {
        window: XResourceId,
    },
    ReparentWindow {
        window: XResourceId,
        parent: XResourceId,
        x: i16,
        y: i16,
    },
    MapSubwindows {
        window: XResourceId,
    },
    UnmapWindow {
        window: XResourceId,
    },
    ConfigureWindow {
        window: XResourceId,
        value_mask: u16,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
        sibling: Option<XResourceId>,
        stack_mode: Option<u8>,
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
    GetSelectionOwner {
        selection: XAtom,
    },
    SendSelectionNotify {
        destination: XResourceId,
        event_mask: u32,
        event: XClientEvent,
    },
    GrabPointer {
        window: XResourceId,
        event_mask: u16,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
        time: u32,
    },
    UngrabPointer {
        time: u32,
    },
    GrabButton {
        window: XResourceId,
        event_mask: u16,
        button: u8,
        modifiers: u16,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
    },
    UngrabButton {
        window: XResourceId,
        button: u8,
        modifiers: u16,
    },
    GrabKeyboard {
        window: XResourceId,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
        time: u32,
    },
    UngrabKeyboard {
        time: u32,
    },
    GrabKey {
        window: XResourceId,
        key: u8,
        modifiers: u16,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
    },
    UngrabKey {
        window: XResourceId,
        key: u8,
        modifiers: u16,
    },
    AllowEvents {
        mode: u8,
        time: u32,
    },
    GrabServer,
    UngrabServer,
    CreateGraphicsContext {
        gc: XResourceId,
        drawable: XResourceId,
        values: XGraphicsContextValues,
    },
    ChangeGraphicsContext {
        gc: XResourceId,
        value_mask: u32,
        values: XGraphicsContextValues,
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
        data: Vec<u8>,
    },
    GetImage {
        format: u8,
        drawable: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
    },
    PolyText8 {
        drawable: XResourceId,
        gc: XResourceId,
        x: i16,
        y: i16,
        text: Vec<u8>,
    },
    ImageText8 {
        drawable: XResourceId,
        gc: XResourceId,
        x: i16,
        y: i16,
        text: Vec<u8>,
    },
    CreateColormap {
        colormap: XResourceId,
        window: XResourceId,
        visual: u32,
    },
    FreeColormap {
        colormap: XResourceId,
    },
    AllocColor {
        colormap: XResourceId,
        red: u16,
        green: u16,
        blue: u16,
    },
    AllocNamedColor {
        colormap: XResourceId,
        name: String,
    },
    GetInputFocus,
    SetInputFocus {
        focus: XResourceId,
        revert_to: u8,
        time: u32,
    },
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
    DeleteProperty {
        window: XResourceId,
        property: u32,
    },
    QueryPointer {
        window: XResourceId,
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
        points: Vec<XPoint>,
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
    Dri3QueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    Dri3Open {
        drawable: XResourceId,
        provider: u32,
    },
    Dri3PixmapFromBuffer {
        pixmap: XResourceId,
        drawable: XResourceId,
        size_bytes: u32,
        width: u16,
        height: u16,
        stride: u16,
        depth: u8,
        bits_per_pixel: u8,
    },
    Dri3PixmapFromBuffers {
        pixmap: XResourceId,
        window: XResourceId,
        num_buffers: u8,
        width: u16,
        height: u16,
        strides: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
        offsets: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
        depth: u8,
        bits_per_pixel: u8,
        modifier: u64,
    },
    Dri3FenceFromFd {
        drawable: XResourceId,
        fence: XResourceId,
        initially_triggered: bool,
    },
    Dri3GetSupportedModifiers {
        window: XResourceId,
        depth: u8,
        bits_per_pixel: u8,
    },
    XfixesQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    XfixesSelectSelectionInput {
        window: XResourceId,
        selection: XAtom,
        event_mask: u32,
    },
    XfixesCreateRegion {
        region: XResourceId,
        rectangles: Vec<Rect>,
    },
    XfixesDestroyRegion {
        region: XResourceId,
    },
    XfixesSetRegion {
        region: XResourceId,
        rectangles: Vec<Rect>,
    },
    PresentQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    PresentPixmap {
        transaction: TransactionId,
        window: XResourceId,
        pixmap: XResourceId,
        serial: u32,
        valid_region: u32,
        update_region: u32,
        x_offset: i16,
        y_offset: i16,
        target_crtc: u32,
        wait_fence: Option<XResourceId>,
        idle_fence: Option<XResourceId>,
        options: u32,
        target_msc: u64,
        divisor: u64,
        remainder: u64,
        notifies: Vec<(XResourceId, u32)>,
    },
    PresentSelectInput {
        event_id: XResourceId,
        window: XResourceId,
        event_mask: u32,
    },
    PresentQueryCapabilities {
        target: XResourceId,
    },
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
    ShmCreatePixmap {
        pixmap: XResourceId,
        drawable: XResourceId,
        width: u16,
        height: u16,
        depth: u8,
        segment: XResourceId,
        offset: u32,
    },
    RandrQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    RandrSelectInput {
        window: XResourceId,
        enable: u16,
    },
    RandrGetScreenSizeRange {
        window: XResourceId,
    },
    RandrGetScreenResources {
        window: XResourceId,
        current: bool,
    },
    RandrGetOutputInfo {
        output: u32,
        config_timestamp: u32,
    },
    RandrGetOutputProperty {
        output: u32,
        property: XAtom,
        property_type: XAtom,
        long_offset: u32,
        long_length: u32,
        delete: bool,
        pending: bool,
    },
    RandrGetCrtcInfo {
        crtc: u32,
        config_timestamp: u32,
    },
    RandrGetCrtcGammaSize {
        crtc: u32,
    },
    RandrGetOutputPrimary {
        window: XResourceId,
    },
    RandrGetProviders {
        window: XResourceId,
    },
    RandrGetMonitors {
        window: XResourceId,
        get_active: bool,
    },
    XkbUseExtension {
        wanted_major: u16,
        wanted_minor: u16,
    },
    GlxQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    GlxGetVisualConfigs {
        screen: u32,
    },
    GlxGetFbConfigs {
        screen: u32,
    },
    GlxClientInfo,
    GlxCreateContext {
        context: XResourceId,
        fbconfig: u32,
        screen: u32,
        share: Option<XResourceId>,
        direct: bool,
    },
    GlxDestroyContext {
        context: XResourceId,
    },
    GlxIsDirect {
        context: XResourceId,
    },
    GlxCreateWindow {
        screen: u32,
        fbconfig: u32,
        window: XResourceId,
        glx_window: XResourceId,
    },
    GlxDeleteWindow {
        glx_window: XResourceId,
    },
    GlxGetDrawableAttributes {
        drawable: XResourceId,
    },
    SyncInitialize {
        desired_major: u8,
        desired_minor: u8,
    },
    SyncDestroyFence {
        fence: XResourceId,
    },
    GlxQueryExtensionsString,
    GlxQueryServerString {
        name: u32,
    },
    XkbGetMap {
        full: u16,
        partial: u16,
    },
    XkbGetCompatMap {
        device_spec: u16,
    },
    XkbGetIndicatorMap {
        device_spec: u16,
    },
    XkbGetState,
    XkbGetControls,
    XkbGetNames {
        which: u32,
    },
    XkbGetDeviceInfo {
        device_spec: u16,
        wanted: u16,
    },
    XkbSelectEvents {
        affect_which: u16,
        clear: u16,
        select_all: u16,
        state_details: Option<(u16, u16)>,
    },
    XkbPerClientFlags {
        change: u32,
        value: u32,
    },
    XiQueryVersion {
        major_version: u16,
        minor_version: u16,
    },
    XiQueryPointer {
        window: XResourceId,
        device_id: u16,
    },
    XiGetClientPointer,
    XiDeviceBell,
    XiUngrabDevice {
        device_id: u16,
        time: u32,
    },
    XiChangeCursor {
        window: XResourceId,
        cursor: Option<XResourceId>,
    },
    XiGetExtensionVersion,
    XiQueryDevice {
        device_id: u16,
    },
    XiSelectEvents {
        window: XResourceId,
        masks: Vec<(u16, Vec<u32>)>,
    },
    XiGetFocus {
        device_id: u16,
    },
    XiGetProperty,
    GeQueryVersion {
        major_version: u16,
        minor_version: u16,
    },
    BigRequestsEnable,
    QueryColors {
        colormap: XResourceId,
        pixels: Vec<u32>,
    },
    CreateCursor {
        cursor: XResourceId,
        source: XResourceId,
        mask: Option<XResourceId>,
    },
    CreateGlyphCursor {
        cursor: XResourceId,
        source_font: XResourceId,
        mask_font: Option<XResourceId>,
    },
    FreeCursor {
        cursor: XResourceId,
    },
    RecolorCursor {
        cursor: XResourceId,
    },
    GetModifierMapping,
    GetKeyboardMapping {
        first_keycode: u8,
        count: u8,
    },
    GetKeyboardControl,
    Bell,
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
    InvalidEventType(u8),
    InvalidValue(u32),
    PropertyValueTooLarge {
        len: usize,
        max: usize,
    },
    ResourceIdOutsideClientRange {
        resource_id: u32,
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
        X_REPARENT_WINDOW => decode_reparent_window(context, bytes),
        X_MAP_WINDOW => decode_map_window(context, bytes),
        X_MAP_SUBWINDOWS => decode_map_subwindows(context, bytes),
        X_UNMAP_WINDOW => decode_unmap_window(context, bytes),
        X_CONFIGURE_WINDOW => decode_configure_window(context, bytes),
        X_GET_GEOMETRY => decode_get_geometry(context, bytes),
        X_QUERY_TREE => decode_query_tree(context, bytes),
        X_INTERN_ATOM => decode_intern_atom(context, bytes),
        X_GET_ATOM_NAME => decode_get_atom_name(context, bytes),
        X_CHANGE_PROPERTY => decode_change_property(context, bytes),
        X_DELETE_PROPERTY => {
            require_exact_len(X_DELETE_PROPERTY, X_DELETE_PROPERTY_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::DeleteProperty {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                property: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_GET_PROPERTY => decode_get_property(context, bytes),
        X_LIST_PROPERTIES => decode_list_properties(context, bytes),
        X_SET_SELECTION_OWNER => decode_set_selection_owner(context, bytes),
        X_GET_SELECTION_OWNER => decode_get_selection_owner(context, bytes),
        X_CONVERT_SELECTION => decode_convert_selection(context, bytes),
        X_SEND_EVENT => decode_send_event(context, bytes),
        X_GRAB_POINTER => decode_grab_pointer(context, bytes),
        X_UNGRAB_POINTER => decode_ungrab_pointer(context, bytes),
        X_GRAB_BUTTON => decode_grab_button(context, bytes),
        X_UNGRAB_BUTTON => decode_ungrab_button(context, bytes),
        X_GRAB_KEYBOARD => decode_grab_keyboard(context, bytes),
        X_UNGRAB_KEYBOARD => decode_ungrab_keyboard(context, bytes),
        X_GRAB_KEY => decode_grab_key(context, bytes),
        X_UNGRAB_KEY => decode_ungrab_key(context, bytes),
        X_ALLOW_EVENTS => decode_allow_events(context, bytes),
        X_GRAB_SERVER => decode_grab_server(bytes),
        X_UNGRAB_SERVER => decode_ungrab_server(bytes),
        X_TRANSLATE_COORDINATES => decode_translate_coordinates(context, bytes),
        X_QUERY_POINTER => {
            require_exact_len(X_QUERY_POINTER, X_QUERY_POINTER_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::QueryPointer {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_SET_INPUT_FOCUS => decode_set_input_focus(context, bytes),
        X_GET_INPUT_FOCUS => decode_get_input_focus(bytes),
        X_GET_KEYBOARD_CONTROL => {
            require_exact_len(X_GET_KEYBOARD_CONTROL, 4, bytes.len())?;
            Ok(XWireRequest::GetKeyboardControl)
        }
        X_BELL => {
            require_exact_len(X_BELL, 4, bytes.len())?;
            Ok(XWireRequest::Bell)
        }
        X_OPEN_FONT => decode_open_font(context, bytes),
        X_CLOSE_FONT => decode_close_font(context, bytes),
        X_QUERY_FONT => decode_query_font(context, bytes),
        X_LIST_FONTS => decode_list_fonts(context, bytes),
        X_LIST_FONTS_WITH_INFO => decode_list_fonts_with_info(context, bytes),
        X_CREATE_PIXMAP => decode_create_pixmap(context, bytes),
        X_FREE_PIXMAP => decode_free_pixmap(context, bytes),
        X_CREATE_GC => decode_create_gc(context, bytes),
        X_SET_CLIP_RECTANGLES => decode_set_clip_rectangles(context, bytes),
        X_CHANGE_GC => decode_change_gc(context, bytes),
        X_FREE_GC => decode_free_gc(context, bytes),
        X_CLEAR_AREA => decode_clear_area(context, bytes),
        X_COPY_AREA => decode_copy_area(context, bytes),
        X_POLY_LINE => decode_poly_line(context, bytes),
        X_POLY_SEGMENT => decode_poly_segment(context, bytes),
        X_FILL_POLY => decode_fill_poly(context, bytes),
        X_POLY_FILL_RECTANGLE => decode_poly_fill_rectangle(context, bytes),
        X_POLY_FILL_ARC => decode_poly_fill_arc(context, bytes),
        X_PUT_IMAGE => decode_put_image(context, bytes),
        X_GET_IMAGE => decode_get_image(context, bytes),
        X_POLY_TEXT8 => decode_poly_text8(context, bytes),
        X_IMAGE_TEXT8 => decode_image_text8(context, bytes),
        X_CREATE_COLORMAP => decode_create_colormap(context, bytes),
        X_FREE_COLORMAP => {
            require_exact_len(X_FREE_COLORMAP, X_FREE_COLORMAP_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::FreeColormap {
                colormap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_ALLOC_COLOR => decode_alloc_color(context, bytes),
        X_ALLOC_NAMED_COLOR => decode_alloc_named_color(context, bytes),
        X_QUERY_COLORS => decode_query_colors(context, bytes),
        X_CREATE_CURSOR => decode_create_cursor(context, bytes),
        X_CREATE_GLYPH_CURSOR => decode_create_glyph_cursor(context, bytes),
        X_FREE_CURSOR => decode_free_cursor(context, bytes),
        X_RECOLOR_CURSOR => decode_recolor_cursor(context, bytes),
        X_QUERY_BEST_SIZE => decode_query_best_size(context, bytes),
        X_QUERY_EXTENSION => decode_query_extension(context, bytes),
        X_LIST_EXTENSIONS => decode_list_extensions(bytes),
        X_GET_KEYBOARD_MAPPING => decode_get_keyboard_mapping(bytes),
        X_GET_MODIFIER_MAPPING => decode_get_modifier_mapping(bytes),
        X_SOPHIA_PRESENT_MAJOR_OPCODE => decode_sophia_present(context, bytes),
        X_MIT_SHM_MAJOR_OPCODE => decode_mit_shm(context, bytes),
        X_RANDR_MAJOR_OPCODE => decode_randr(context, bytes),
        X_KEYBOARD_MAJOR_OPCODE => decode_x_keyboard(context, bytes),
        X_BIG_REQUESTS_MAJOR_OPCODE => decode_big_requests(bytes),
        X_INPUT_MAJOR_OPCODE => decode_x_input(context, bytes),
        X_GENERIC_EVENT_MAJOR_OPCODE => {
            require_exact_len(
                X_GENERIC_EVENT_MAJOR_OPCODE,
                X_GENERIC_EVENT_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            if bytes[1] != X_GENERIC_EVENT_QUERY_VERSION_MINOR_OPCODE {
                return Err(XWireParseError::UnknownOpcode(bytes[1]));
            }
            Ok(XWireRequest::GeQueryVersion {
                major_version: context.byte_order.u16(&bytes[4..6]),
                minor_version: context.byte_order.u16(&bytes[6..8]),
            })
        }
        X_DRI3_MAJOR_OPCODE => decode_dri3(context, bytes),
        X_PRESENT_MAJOR_OPCODE => decode_present(context, bytes),
        X_XFIXES_MAJOR_OPCODE => decode_xfixes(context, bytes),
        X_GLX_MAJOR_OPCODE => decode_glx(context, bytes),
        X_SYNC_MAJOR_OPCODE => match bytes[1] {
            X_SYNC_INITIALIZE_MINOR_OPCODE => {
                require_exact_len(X_SYNC_MAJOR_OPCODE, 8, bytes.len())?;
                Ok(XWireRequest::SyncInitialize {
                    desired_major: bytes[4],
                    desired_minor: bytes[5],
                })
            }
            X_SYNC_DESTROY_FENCE_MINOR_OPCODE => {
                require_exact_len(X_SYNC_MAJOR_OPCODE, 8, bytes.len())?;
                Ok(XWireRequest::SyncDestroyFence {
                    fence: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                })
            }
            other => Err(XWireParseError::UnknownOpcode(other)),
        },
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}

fn decode_glx(context: XWireClientContext, bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    let id = |offset: usize| {
        XResourceId::new(
            u64::from(context.byte_order.u32(&bytes[offset..offset + 4])),
            1,
        )
    };
    match bytes[1] {
        X_GLX_QUERY_VERSION_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::GlxQueryVersion {
                major_version: context.byte_order.u32(&bytes[4..8]),
                minor_version: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_GLX_GET_VISUAL_CONFIGS_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxGetVisualConfigs {
                screen: context.byte_order.u32(&bytes[4..8]),
            })
        }
        X_GLX_QUERY_EXTENSIONS_STRING_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxQueryExtensionsString)
        }
        X_GLX_QUERY_SERVER_STRING_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::GlxQueryServerString {
                name: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_GLX_GET_FB_CONFIGS_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxGetFbConfigs {
                screen: context.byte_order.u32(&bytes[4..8]),
            })
        }
        X_GLX_CLIENT_INFO_MINOR_OPCODE
        | X_GLX_SET_CLIENT_INFO_ARB_MINOR_OPCODE
        | X_GLX_SET_CLIENT_INFO_2_ARB_MINOR_OPCODE => {
            require_len(X_GLX_MAJOR_OPCODE, 16, bytes.len())?;
            Ok(XWireRequest::GlxClientInfo)
        }
        X_GLX_DESTROY_CONTEXT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxDestroyContext { context: id(4) })
        }
        X_GLX_IS_DIRECT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxIsDirect { context: id(4) })
        }
        X_GLX_CREATE_NEW_CONTEXT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 28, bytes.len())?;
            let context_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(context_id)?;
            let share = context.byte_order.u32(&bytes[20..24]);
            Ok(XWireRequest::GlxCreateContext {
                context: XResourceId::new(u64::from(context_id), 1),
                fbconfig: context.byte_order.u32(&bytes[8..12]),
                screen: context.byte_order.u32(&bytes[12..16]),
                share: (share != 0).then(|| XResourceId::new(u64::from(share), 1)),
                direct: bytes[24] != 0,
            })
        }
        X_GLX_CREATE_CONTEXT_ATTRIBS_ARB_MINOR_OPCODE => {
            require_len(X_GLX_MAJOR_OPCODE, 28, bytes.len())?;
            let count = context.byte_order.u32(&bytes[24..28]) as usize;
            require_exact_len(
                X_GLX_MAJOR_OPCODE,
                28usize.saturating_add(count.saturating_mul(8)),
                bytes.len(),
            )?;
            let context_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(context_id)?;
            let share = context.byte_order.u32(&bytes[16..20]);
            Ok(XWireRequest::GlxCreateContext {
                context: XResourceId::new(u64::from(context_id), 1),
                fbconfig: context.byte_order.u32(&bytes[8..12]),
                screen: context.byte_order.u32(&bytes[12..16]),
                share: (share != 0).then(|| XResourceId::new(u64::from(share), 1)),
                direct: bytes[20] != 0,
            })
        }
        X_GLX_CREATE_WINDOW_MINOR_OPCODE => {
            require_len(X_GLX_MAJOR_OPCODE, 24, bytes.len())?;
            let count = context.byte_order.u32(&bytes[20..24]) as usize;
            require_exact_len(
                X_GLX_MAJOR_OPCODE,
                24usize.saturating_add(count.saturating_mul(8)),
                bytes.len(),
            )?;
            let glx = context.byte_order.u32(&bytes[16..20]);
            context.validate_new_resource_id(glx)?;
            Ok(XWireRequest::GlxCreateWindow {
                screen: context.byte_order.u32(&bytes[4..8]),
                fbconfig: context.byte_order.u32(&bytes[8..12]),
                window: id(12),
                glx_window: XResourceId::new(u64::from(glx), 1),
            })
        }
        X_GLX_DELETE_WINDOW_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxDeleteWindow { glx_window: id(4) })
        }
        X_GLX_GET_DRAWABLE_ATTRIBUTES_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxGetDrawableAttributes { drawable: id(4) })
        }
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}

fn decode_xfixes(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_XFIXES_QUERY_VERSION_MINOR_OPCODE => decode_extension_query_version(
            context,
            bytes,
            X_XFIXES_MAJOR_OPCODE,
            X_XFIXES_QUERY_VERSION_MINOR_OPCODE,
            |major_version, minor_version| XWireRequest::XfixesQueryVersion {
                major_version,
                minor_version,
            },
        ),
        X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE => {
            require_exact_len(X_XFIXES_MAJOR_OPCODE, 16, bytes.len())?;
            Ok(XWireRequest::XfixesSelectSelectionInput {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                selection: context.byte_order.u32(&bytes[8..12]),
                event_mask: context.byte_order.u32(&bytes[12..16]),
            })
        }
        X_XFIXES_CREATE_REGION_MINOR_OPCODE => {
            require_len(X_XFIXES_MAJOR_OPCODE, 8, bytes.len())?;
            if (bytes.len() - 8) % 8 != 0 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_XFIXES_MAJOR_OPCODE,
                    expected_at_least: 8,
                    actual: bytes.len(),
                });
            }
            let region = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(region)?;
            let rectangles = bytes[8..]
                .chunks_exact(8)
                .map(|rectangle| Rect {
                    x: i32::from(context.byte_order.i16(&rectangle[0..2])),
                    y: i32::from(context.byte_order.i16(&rectangle[2..4])),
                    width: i32::from(context.byte_order.u16(&rectangle[4..6])),
                    height: i32::from(context.byte_order.u16(&rectangle[6..8])),
                })
                .collect();
            Ok(XWireRequest::XfixesCreateRegion {
                region: XResourceId::new(u64::from(region), 1),
                rectangles,
            })
        }
        X_XFIXES_SET_REGION_MINOR_OPCODE => {
            require_len(X_XFIXES_MAJOR_OPCODE, 8, bytes.len())?;
            if (bytes.len() - 8) % 8 != 0 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_XFIXES_MAJOR_OPCODE,
                    expected_at_least: 8,
                    actual: bytes.len(),
                });
            }
            let rectangles = bytes[8..]
                .chunks_exact(8)
                .map(|rectangle| Rect {
                    x: i32::from(context.byte_order.i16(&rectangle[0..2])),
                    y: i32::from(context.byte_order.i16(&rectangle[2..4])),
                    width: i32::from(context.byte_order.u16(&rectangle[4..6])),
                    height: i32::from(context.byte_order.u16(&rectangle[6..8])),
                })
                .collect();
            Ok(XWireRequest::XfixesSetRegion {
                region: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                rectangles,
            })
        }
        X_XFIXES_DESTROY_REGION_MINOR_OPCODE => {
            require_exact_len(X_XFIXES_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::XfixesDestroyRegion {
                region: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[1])),
    }
}

fn decode_dri3(context: XWireClientContext, bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_DRI3_QUERY_VERSION_MINOR_OPCODE => decode_extension_query_version(
            context,
            bytes,
            X_DRI3_MAJOR_OPCODE,
            X_DRI3_QUERY_VERSION_MINOR_OPCODE,
            |major_version, minor_version| XWireRequest::Dri3QueryVersion {
                major_version,
                minor_version,
            },
        ),
        X_DRI3_OPEN_MINOR_OPCODE => {
            require_exact_len(X_DRI3_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::Dri3Open {
                drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                provider: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_DRI3_PIXMAP_FROM_BUFFER_MINOR_OPCODE => {
            require_exact_len(X_DRI3_MAJOR_OPCODE, 24, bytes.len())?;
            let pixmap = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(pixmap)?;
            Ok(XWireRequest::Dri3PixmapFromBuffer {
                pixmap: XResourceId::new(u64::from(pixmap), 1),
                drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                size_bytes: context.byte_order.u32(&bytes[12..16]),
                width: context.byte_order.u16(&bytes[16..18]),
                height: context.byte_order.u16(&bytes[18..20]),
                stride: context.byte_order.u16(&bytes[20..22]),
                depth: bytes[22],
                bits_per_pixel: bytes[23],
            })
        }
        X_DRI3_FENCE_FROM_FD_MINOR_OPCODE => {
            require_exact_len(X_DRI3_MAJOR_OPCODE, 16, bytes.len())?;
            let fence = context.byte_order.u32(&bytes[8..12]);
            context.validate_new_resource_id(fence)?;
            Ok(XWireRequest::Dri3FenceFromFd {
                drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                fence: XResourceId::new(u64::from(fence), 1),
                initially_triggered: bytes[12] != 0,
            })
        }
        X_DRI3_GET_SUPPORTED_MODIFIERS_MINOR_OPCODE => {
            require_exact_len(X_DRI3_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::Dri3GetSupportedModifiers {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                depth: bytes[8],
                bits_per_pixel: bytes[9],
            })
        }
        X_DRI3_PIXMAP_FROM_BUFFERS_MINOR_OPCODE => {
            require_exact_len(X_DRI3_MAJOR_OPCODE, 64, bytes.len())?;
            let pixmap = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(pixmap)?;
            let num_buffers = bytes[12];
            if num_buffers == 0 || usize::from(num_buffers) > sophia_protocol::DMA_BUF_MAX_PLANES {
                return Err(XWireParseError::InvalidValue(u32::from(num_buffers)));
            }
            Ok(XWireRequest::Dri3PixmapFromBuffers {
                pixmap: XResourceId::new(u64::from(pixmap), 1),
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                num_buffers,
                width: context.byte_order.u16(&bytes[16..18]),
                height: context.byte_order.u16(&bytes[18..20]),
                strides: [
                    context.byte_order.u32(&bytes[20..24]),
                    context.byte_order.u32(&bytes[28..32]),
                    context.byte_order.u32(&bytes[36..40]),
                    context.byte_order.u32(&bytes[44..48]),
                ],
                offsets: [
                    context.byte_order.u32(&bytes[24..28]),
                    context.byte_order.u32(&bytes[32..36]),
                    context.byte_order.u32(&bytes[40..44]),
                    context.byte_order.u32(&bytes[48..52]),
                ],
                depth: bytes[52],
                bits_per_pixel: bytes[53],
                modifier: context.byte_order.u64(&bytes[56..64]),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[1])),
    }
}

fn decode_present(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_PRESENT_QUERY_VERSION_MINOR_OPCODE => decode_extension_query_version(
            context,
            bytes,
            X_PRESENT_MAJOR_OPCODE,
            X_PRESENT_QUERY_VERSION_MINOR_OPCODE,
            |major_version, minor_version| XWireRequest::PresentQueryVersion {
                major_version,
                minor_version,
            },
        ),
        X_PRESENT_PIXMAP_MINOR_OPCODE => {
            require_len(X_PRESENT_MAJOR_OPCODE, 72, bytes.len())?;
            if (bytes.len() - 72) % 8 != 0 {
                return Err(XWireParseError::InvalidLength {
                    opcode: X_PRESENT_MAJOR_OPCODE,
                    expected_at_least: 72,
                    actual: bytes.len(),
                });
            }
            let raw_resource = |offset: usize| context.byte_order.u32(&bytes[offset..offset + 4]);
            let resource = |offset: usize| XResourceId::new(u64::from(raw_resource(offset)), 1);
            let optional_resource = |offset: usize| {
                let raw = raw_resource(offset);
                (raw != 0).then(|| XResourceId::new(u64::from(raw), 1))
            };
            let notifies = bytes[72..]
                .chunks_exact(8)
                .map(|notify| {
                    (
                        XResourceId::new(u64::from(context.byte_order.u32(&notify[..4])), 1),
                        context.byte_order.u32(&notify[4..]),
                    )
                })
                .collect();
            Ok(XWireRequest::PresentPixmap {
                transaction: context.transaction,
                window: resource(4),
                pixmap: resource(8),
                serial: raw_resource(12),
                valid_region: raw_resource(16),
                update_region: raw_resource(20),
                x_offset: context.byte_order.i16(&bytes[24..26]),
                y_offset: context.byte_order.i16(&bytes[26..28]),
                target_crtc: raw_resource(28),
                wait_fence: optional_resource(32),
                idle_fence: optional_resource(36),
                options: raw_resource(40),
                target_msc: context.byte_order.u64(&bytes[48..56]),
                divisor: context.byte_order.u64(&bytes[56..64]),
                remainder: context.byte_order.u64(&bytes[64..72]),
                notifies,
            })
        }
        X_PRESENT_SELECT_INPUT_MINOR_OPCODE => {
            require_exact_len(X_PRESENT_MAJOR_OPCODE, 16, bytes.len())?;
            let event_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(event_id)?;
            Ok(XWireRequest::PresentSelectInput {
                event_id: XResourceId::new(u64::from(event_id), 1),
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                event_mask: context.byte_order.u32(&bytes[12..16]),
            })
        }
        X_PRESENT_QUERY_CAPABILITIES_MINOR_OPCODE => {
            require_exact_len(X_PRESENT_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::PresentQueryCapabilities {
                target: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[1])),
    }
}

impl XWireRequest {
    pub const fn required_fd_count(&self) -> usize {
        match self {
            Self::Dri3PixmapFromBuffer { .. } | Self::Dri3FenceFromFd { .. } => 1,
            Self::Dri3PixmapFromBuffers { num_buffers, .. } => *num_buffers as usize,
            _ => 0,
        }
    }
}

fn decode_extension_query_version(
    context: XWireClientContext,
    bytes: &[u8],
    major_opcode: u8,
    minor_opcode: u8,
    request: impl FnOnce(u32, u32) -> XWireRequest,
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(major_opcode, 12, bytes.len())?;
    if bytes[1] != minor_opcode {
        return Err(XWireParseError::UnknownOpcode(bytes[1]));
    }
    Ok(request(
        context.byte_order.u32(&bytes[4..8]),
        context.byte_order.u32(&bytes[8..12]),
    ))
}

fn decode_x_input(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_INPUT_DEVICE_BELL_MINOR_OPCODE => {
            require_exact_len(X_INPUT_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::XiDeviceBell)
        }
        X_INPUT_QUERY_POINTER_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_QUERY_POINTER_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XiQueryPointer {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                device_id: context.byte_order.u16(&bytes[8..10]),
            })
        }
        X_INPUT_CHANGE_CURSOR_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_CHANGE_CURSOR_REQ_LEN,
                bytes.len(),
            )?;
            let cursor = context.byte_order.u32(&bytes[8..12]);
            Ok(XWireRequest::XiChangeCursor {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                cursor: (cursor != 0).then(|| XResourceId::new(u64::from(cursor), 1)),
            })
        }
        X_INPUT_GET_CLIENT_POINTER_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_GET_CLIENT_POINTER_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XiGetClientPointer)
        }
        X_INPUT_UNGRAB_DEVICE_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_UNGRAB_DEVICE_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XiUngrabDevice {
                device_id: context.byte_order.u16(&bytes[8..10]),
                time: context.byte_order.u32(&bytes[4..8]),
            })
        }
        X_INPUT_GET_EXTENSION_VERSION_MINOR_OPCODE => {
            require_len(X_INPUT_MAJOR_OPCODE, 8, bytes.len())?;
            let name_len = usize::from(context.byte_order.u16(&bytes[4..6]));
            let expected = 8usize.saturating_add(padded_len(name_len));
            require_exact_len(X_INPUT_MAJOR_OPCODE, expected, bytes.len())?;
            Ok(XWireRequest::XiGetExtensionVersion)
        }
        X_INPUT_QUERY_VERSION_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XiQueryVersion {
                major_version: context.byte_order.u16(&bytes[4..6]),
                minor_version: context.byte_order.u16(&bytes[6..8]),
            })
        }
        X_INPUT_QUERY_DEVICE_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_QUERY_DEVICE_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XiQueryDevice {
                device_id: context.byte_order.u16(&bytes[4..6]),
            })
        }
        X_INPUT_GET_FOCUS_MINOR_OPCODE => {
            require_exact_len(X_INPUT_MAJOR_OPCODE, X_INPUT_GET_FOCUS_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::XiGetFocus {
                device_id: context.byte_order.u16(&bytes[4..6]),
            })
        }
        X_INPUT_GET_PROPERTY_MINOR_OPCODE => {
            require_exact_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_GET_PROPERTY_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XiGetProperty)
        }
        X_INPUT_SELECT_EVENTS_MINOR_OPCODE => {
            require_len(
                X_INPUT_MAJOR_OPCODE,
                X_INPUT_SELECT_EVENTS_REQ_LEN,
                bytes.len(),
            )?;
            let window = XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1);
            let count = usize::from(context.byte_order.u16(&bytes[8..10]));
            if count > 16 {
                return Err(XWireParseError::InvalidValue(count as u32));
            }
            let mut offset = X_INPUT_SELECT_EVENTS_REQ_LEN;
            let mut masks = Vec::with_capacity(count);
            for _ in 0..count {
                if offset.checked_add(4).is_none_or(|end| end > bytes.len()) {
                    return Err(XWireParseError::InvalidLength {
                        opcode: X_INPUT_MAJOR_OPCODE,
                        expected_at_least: offset.saturating_add(4),
                        actual: bytes.len(),
                    });
                }
                let device_id = context.byte_order.u16(&bytes[offset..offset + 2]);
                let words = usize::from(context.byte_order.u16(&bytes[offset + 2..offset + 4]));
                if words > 8 {
                    return Err(XWireParseError::InvalidValue(words as u32));
                }
                offset += 4;
                let end = offset.saturating_add(words.saturating_mul(4));
                if end > bytes.len() {
                    return Err(XWireParseError::InvalidLength {
                        opcode: X_INPUT_MAJOR_OPCODE,
                        expected_at_least: end,
                        actual: bytes.len(),
                    });
                }
                let mask = bytes[offset..end]
                    .chunks_exact(4)
                    .map(|word| context.byte_order.u32(word))
                    .collect();
                masks.push((device_id, mask));
                offset = end;
            }
            if offset != bytes.len() {
                return Err(XWireParseError::TrailingBytes(bytes.len() - offset));
            }
            Ok(XWireRequest::XiSelectEvents { window, masks })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[0])),
    }
}

fn decode_big_requests(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_BIG_REQUESTS_ENABLE_MINOR_OPCODE => {
            require_exact_len(
                X_BIG_REQUESTS_MAJOR_OPCODE,
                X_BIG_REQUESTS_ENABLE_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::BigRequestsEnable)
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[0])),
    }
}

fn decode_x_keyboard(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    match bytes[1] {
        X_KEYBOARD_USE_EXTENSION_MINOR_OPCODE => {
            require_exact_len(
                X_KEYBOARD_MAJOR_OPCODE,
                X_KEYBOARD_USE_EXTENSION_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XkbUseExtension {
                wanted_major: context.byte_order.u16(&bytes[4..6]),
                wanted_minor: context.byte_order.u16(&bytes[6..8]),
            })
        }
        X_KEYBOARD_GET_MAP_MINOR_OPCODE => {
            require_exact_len(
                X_KEYBOARD_MAJOR_OPCODE,
                X_KEYBOARD_GET_MAP_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XkbGetMap {
                full: context.byte_order.u16(&bytes[6..8]),
                partial: context.byte_order.u16(&bytes[8..10]),
            })
        }
        X_KEYBOARD_GET_COMPAT_MAP_MINOR_OPCODE => {
            require_exact_len(X_KEYBOARD_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::XkbGetCompatMap {
                device_spec: context.byte_order.u16(&bytes[4..6]),
            })
        }
        X_KEYBOARD_GET_INDICATOR_MAP_MINOR_OPCODE => {
            require_exact_len(X_KEYBOARD_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::XkbGetIndicatorMap {
                device_spec: context.byte_order.u16(&bytes[4..6]),
            })
        }
        X_KEYBOARD_GET_STATE_MINOR_OPCODE => {
            require_exact_len(X_KEYBOARD_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::XkbGetState)
        }
        X_KEYBOARD_GET_CONTROLS_MINOR_OPCODE => {
            require_exact_len(
                X_KEYBOARD_MAJOR_OPCODE,
                X_KEYBOARD_GET_CONTROLS_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XkbGetControls)
        }
        X_KEYBOARD_GET_NAMES_MINOR_OPCODE => {
            require_exact_len(X_KEYBOARD_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::XkbGetNames {
                which: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_KEYBOARD_GET_DEVICE_INFO_MINOR_OPCODE => {
            require_exact_len(X_KEYBOARD_MAJOR_OPCODE, 16, bytes.len())?;
            Ok(XWireRequest::XkbGetDeviceInfo {
                device_spec: context.byte_order.u16(&bytes[4..6]),
                wanted: context.byte_order.u16(&bytes[6..8]),
            })
        }
        X_KEYBOARD_SELECT_EVENTS_MINOR_OPCODE => {
            require_len(
                X_KEYBOARD_MAJOR_OPCODE,
                X_KEYBOARD_SELECT_EVENTS_REQ_LEN,
                bytes.len(),
            )?;
            let affect_which = context.byte_order.u16(&bytes[6..8]);
            let state_details = if affect_which & 4 != 0 {
                let offset = 16 + if affect_which & 1 != 0 { 4 } else { 0 };
                (bytes.len() >= offset + 4).then(|| {
                    (
                        context.byte_order.u16(&bytes[offset..offset + 2]),
                        context.byte_order.u16(&bytes[offset + 2..offset + 4]),
                    )
                })
            } else {
                None
            };
            Ok(XWireRequest::XkbSelectEvents {
                affect_which,
                clear: context.byte_order.u16(&bytes[8..10]),
                select_all: context.byte_order.u16(&bytes[10..12]),
                state_details,
            })
        }
        X_KEYBOARD_PER_CLIENT_FLAGS_MINOR_OPCODE => {
            require_exact_len(
                X_KEYBOARD_MAJOR_OPCODE,
                X_KEYBOARD_PER_CLIENT_FLAGS_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::XkbPerClientFlags {
                change: context.byte_order.u32(&bytes[8..12]),
                value: context.byte_order.u32(&bytes[12..16]),
            })
        }
        _ => Err(XWireParseError::UnknownOpcode(bytes[0])),
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
        X_RANDR_SELECT_INPUT_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_SELECT_INPUT_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrSelectInput {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                enable: context.byte_order.u16(&bytes[8..10]),
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
        X_RANDR_GET_OUTPUT_INFO_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_OUTPUT_INFO_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetOutputInfo {
                output: context.byte_order.u32(&bytes[4..8]),
                config_timestamp: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_RANDR_GET_OUTPUT_PROPERTY_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_OUTPUT_PROPERTY_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetOutputProperty {
                output: context.byte_order.u32(&bytes[4..8]),
                property: context.byte_order.u32(&bytes[8..12]),
                property_type: context.byte_order.u32(&bytes[12..16]),
                long_offset: context.byte_order.u32(&bytes[16..20]),
                long_length: context.byte_order.u32(&bytes[20..24]),
                delete: bytes[24] != 0,
                pending: bytes[25] != 0,
            })
        }
        X_RANDR_GET_CRTC_INFO_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_CRTC_INFO_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetCrtcInfo {
                crtc: context.byte_order.u32(&bytes[4..8]),
                config_timestamp: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_RANDR_GET_CRTC_GAMMA_SIZE_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_CRTC_GAMMA_SIZE_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetCrtcGammaSize {
                crtc: context.byte_order.u32(&bytes[4..8]),
            })
        }
        X_RANDR_GET_OUTPUT_PRIMARY_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_OUTPUT_PRIMARY_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetOutputPrimary {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_RANDR_GET_PROVIDERS_MINOR_OPCODE => {
            require_exact_len(X_RANDR_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::RandrGetProviders {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_RANDR_GET_MONITORS_MINOR_OPCODE => {
            require_exact_len(
                X_RANDR_MAJOR_OPCODE,
                X_RANDR_GET_MONITORS_REQ_LEN,
                bytes.len(),
            )?;
            Ok(XWireRequest::RandrGetMonitors {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                get_active: bytes[8] != 0,
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
            let segment = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(segment)?;
            Ok(XWireRequest::ShmAttach {
                segment: XResourceId::new(u64::from(segment), 1),
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
        X_MIT_SHM_CREATE_PIXMAP_MINOR_OPCODE => {
            require_exact_len(
                X_MIT_SHM_MAJOR_OPCODE,
                X_MIT_SHM_CREATE_PIXMAP_REQ_LEN,
                bytes.len(),
            )?;
            let pixmap = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(pixmap)?;
            Ok(XWireRequest::ShmCreatePixmap {
                pixmap: XResourceId::new(u64::from(pixmap), 1),
                drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
                width: context.byte_order.u16(&bytes[12..14]),
                height: context.byte_order.u16(&bytes[14..16]),
                depth: bytes[16],
                segment: XResourceId::new(u64::from(context.byte_order.u32(&bytes[20..24])), 1),
                offset: context.byte_order.u32(&bytes[24..28]),
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
        data: bytes[X_PUT_IMAGE_REQ_LEN..].to_vec(),
    })
}

fn decode_get_image(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_IMAGE, X_GET_IMAGE_REQ_LEN, bytes.len())?;
    validate_wire_image_format(bytes[1])?;
    let width = context.byte_order.u16(&bytes[12..14]);
    let height = context.byte_order.u16(&bytes[14..16]);
    let byte_len = usize::from(width)
        .checked_mul(usize::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(XWireParseError::PropertyValueTooLarge {
            len: usize::MAX,
            max: X_PUT_IMAGE_MAX_DATA_BYTES,
        })?;
    if byte_len > X_PUT_IMAGE_MAX_DATA_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: byte_len,
            max: X_PUT_IMAGE_MAX_DATA_BYTES,
        });
    }
    Ok(XWireRequest::GetImage {
        format: bytes[1],
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        x: context.byte_order.i16(&bytes[8..10]),
        y: context.byte_order.i16(&bytes[10..12]),
        width,
        height,
        plane_mask: context.byte_order.u32(&bytes[16..20]),
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
    let mut text = Vec::new();
    while offset < item_bytes.len() {
        let len = item_bytes[offset];
        offset += 1;
        if len == 0 && item_bytes[offset..].iter().all(|byte| *byte == 0) {
            break;
        }
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

        let remaining = item_bytes.len().saturating_sub(offset);
        let glyph_len = usize::from(len);
        if remaining >= 1 + glyph_len {
            offset += 1;
            text.extend_from_slice(&item_bytes[offset..offset + glyph_len]);
            offset += glyph_len;
            continue;
        }
        if remaining == glyph_len && glyph_len > 0 {
            offset += 1;
            text.extend_from_slice(&item_bytes[offset..offset + glyph_len - 1]);
            offset += glyph_len - 1;
            continue;
        }
        let item_len = 1usize + glyph_len;
        if remaining < item_len {
            return Err(XWireParseError::InvalidLength {
                opcode: X_POLY_TEXT8,
                expected_at_least: X_POLY_TEXT8_REQ_LEN + offset + item_len,
                actual: bytes.len(),
            });
        }
    }

    Ok(XWireRequest::PolyText8 {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        x: context.byte_order.i16(&bytes[12..14]),
        y: context.byte_order.i16(&bytes[14..16]),
        text,
    })
}

fn decode_image_text8(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_IMAGE_TEXT8, X_IMAGE_TEXT8_REQ_LEN, bytes.len())?;
    let text_len = usize::from(bytes[1]);
    if text_len > X_IMAGE_TEXT8_MAX_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: text_len,
            max: X_IMAGE_TEXT8_MAX_BYTES,
        });
    }
    let expected_len = X_IMAGE_TEXT8_REQ_LEN + padded_len(text_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_IMAGE_TEXT8,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }

    Ok(XWireRequest::ImageText8 {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        x: context.byte_order.i16(&bytes[12..14]),
        y: context.byte_order.i16(&bytes[14..16]),
        text: bytes[X_IMAGE_TEXT8_REQ_LEN..X_IMAGE_TEXT8_REQ_LEN + text_len].to_vec(),
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

fn decode_unmap_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNMAP_WINDOW, X_UNMAP_WINDOW_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UnmapWindow {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
    })
}

fn decode_configure_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_CONFIGURE_WINDOW, X_CONFIGURE_WINDOW_REQ_LEN, bytes.len())?;
    let value_mask = context.byte_order.u16(&bytes[8..10]);
    let value_count = usize::try_from(value_mask.count_ones()).unwrap_or(usize::MAX);
    let expected_len = X_CONFIGURE_WINDOW_REQ_LEN + value_count.saturating_mul(4);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CONFIGURE_WINDOW,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let mut cursor = X_CONFIGURE_WINDOW_REQ_LEN;
    let mut next_value = || {
        let value = context.byte_order.u32(&bytes[cursor..cursor + 4]);
        cursor += 4;
        value
    };
    let x = (value_mask & 0x0001 != 0).then(|| next_value() as i16);
    let y = (value_mask & 0x0002 != 0).then(|| next_value() as i16);
    let width = (value_mask & 0x0004 != 0).then(|| next_value() as u16);
    let height = (value_mask & 0x0008 != 0).then(|| next_value() as u16);
    if value_mask & 0x0010 != 0 {
        let _ = next_value();
    }
    let sibling = (value_mask & 0x0020 != 0).then(|| XResourceId::new(u64::from(next_value()), 1));
    let stack_mode = (value_mask & 0x0040 != 0).then(|| next_value() as u8);

    Ok(XWireRequest::ConfigureWindow {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        value_mask,
        x,
        y,
        width,
        height,
        sibling,
        stack_mode,
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

fn decode_create_colormap(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_CREATE_COLORMAP, X_CREATE_COLORMAP_REQ_LEN, bytes.len())?;
    let colormap = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(colormap)?;
    Ok(XWireRequest::CreateColormap {
        colormap: XResourceId::new(u64::from(colormap), 1),
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        visual: context.byte_order.u32(&bytes[12..16]),
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

    let mut points = Vec::with_capacity(point_bytes.len() / 4);
    let mut previous = XPoint { x: 0, y: 0 };
    for point in point_bytes.chunks_exact(4) {
        let mut decoded = XPoint {
            x: context.byte_order.i16(&point[0..2]),
            y: context.byte_order.i16(&point[2..4]),
        };
        if bytes[1] == 1 && !points.is_empty() {
            decoded.x = previous.x.saturating_add(decoded.x);
            decoded.y = previous.y.saturating_add(decoded.y);
        }
        previous = decoded;
        points.push(decoded);
    }
    Ok(XWireRequest::PolyLine {
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        points,
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

fn decode_alloc_named_color(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(
        X_ALLOC_NAMED_COLOR,
        X_ALLOC_NAMED_COLOR_REQ_LEN,
        bytes.len(),
    )?;
    let name_len = usize::from(context.byte_order.u16(&bytes[8..10]));
    if name_len > X_ALLOC_NAMED_COLOR_MAX_NAME_BYTES {
        return Err(XWireParseError::PropertyValueTooLarge {
            len: name_len,
            max: X_ALLOC_NAMED_COLOR_MAX_NAME_BYTES,
        });
    }
    let expected_len = X_ALLOC_NAMED_COLOR_REQ_LEN + padded_len(name_len);
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_ALLOC_NAMED_COLOR,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let name = core::str::from_utf8(
        &bytes[X_ALLOC_NAMED_COLOR_REQ_LEN..X_ALLOC_NAMED_COLOR_REQ_LEN + name_len],
    )
    .map_err(|_| XWireParseError::InvalidLength {
        opcode: X_ALLOC_NAMED_COLOR,
        expected_at_least: expected_len,
        actual: bytes.len(),
    })?;
    Ok(XWireRequest::AllocNamedColor {
        colormap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        name: name.to_owned(),
    })
}

fn decode_alloc_color(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_ALLOC_COLOR, X_ALLOC_COLOR_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::AllocColor {
        colormap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        red: context.byte_order.u16(&bytes[8..10]),
        green: context.byte_order.u16(&bytes[10..12]),
        blue: context.byte_order.u16(&bytes[12..14]),
    })
}

fn decode_create_cursor(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_CREATE_CURSOR, X_CREATE_CURSOR_REQ_LEN, bytes.len())?;
    let cursor = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(cursor)?;
    let mask = context.byte_order.u32(&bytes[12..16]);
    Ok(XWireRequest::CreateCursor {
        cursor: XResourceId::new(u64::from(cursor), 1),
        source: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        mask: (mask != 0).then(|| XResourceId::new(u64::from(mask), 1)),
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
    let cursor = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(cursor)?;
    let mask_font = context.byte_order.u32(&bytes[12..16]);
    Ok(XWireRequest::CreateGlyphCursor {
        cursor: XResourceId::new(u64::from(cursor), 1),
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

fn decode_recolor_cursor(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_RECOLOR_CURSOR, X_RECOLOR_CURSOR_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::RecolorCursor {
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
    let pixmap = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(pixmap)?;
    Ok(XWireRequest::CreatePixmap {
        depth: bytes[1],
        pixmap: XResourceId::new(u64::from(pixmap), 1),
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
    let font = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(font)?;
    Ok(XWireRequest::OpenFont {
        font: XResourceId::new(u64::from(font), 1),
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
    let gc = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(gc)?;
    let value_mask = context.byte_order.u32(&bytes[12..16]);
    if value_mask & !0x007f_ffff != 0 {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CREATE_GC,
            expected_at_least: X_CREATE_GC_REQ_LEN,
            actual: bytes.len(),
        });
    }
    let value_count = usize::try_from(value_mask.count_ones()).unwrap_or(usize::MAX);
    let expected_len = X_CREATE_GC_REQ_LEN.saturating_add(value_count.saturating_mul(4));
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CREATE_GC,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let mut values = XGraphicsContextValues::default();
    let mut cursor = X_CREATE_GC_REQ_LEN;
    let mut next_value = || {
        let value = context.byte_order.u32(&bytes[cursor..cursor + 4]);
        cursor += 4;
        value
    };
    for bit in 0..23 {
        if value_mask & (1 << bit) == 0 {
            continue;
        }
        let value = next_value();
        match bit {
            0 => values.function = u8::try_from(value).unwrap_or(u8::MAX),
            1 => values.plane_mask = value,
            2 => values.foreground = value,
            3 => values.background = value,
            4 => values.line_width = u16::try_from(value).unwrap_or(u16::MAX),
            8 => values.fill_style = u8::try_from(value).unwrap_or(u8::MAX),
            14 => values.font = (value != 0).then(|| XResourceId::new(u64::from(value), 1)),
            17 => values.clip_x_origin = value as i16,
            18 => values.clip_y_origin = value as i16,
            _ => {}
        }
    }
    Ok(XWireRequest::CreateGraphicsContext {
        gc: XResourceId::new(u64::from(gc), 1),
        drawable: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        values,
    })
}

fn decode_change_gc(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_CHANGE_GC, X_CHANGE_GC_REQ_LEN, bytes.len())?;
    let value_mask = context.byte_order.u32(&bytes[8..12]);
    if value_mask & !0x007f_ffff != 0 {
        return Err(XWireParseError::InvalidValue(value_mask));
    }
    let value_count = usize::try_from(value_mask.count_ones()).unwrap_or(usize::MAX);
    let expected_len = X_CHANGE_GC_REQ_LEN.saturating_add(value_count.saturating_mul(4));
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CHANGE_GC,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let mut values = XGraphicsContextValues::default();
    let mut cursor = X_CHANGE_GC_REQ_LEN;
    for bit in 0..23 {
        if value_mask & (1 << bit) == 0 {
            continue;
        }
        let value = context.byte_order.u32(&bytes[cursor..cursor + 4]);
        cursor += 4;
        match bit {
            0 => values.function = u8::try_from(value).unwrap_or(u8::MAX),
            1 => values.plane_mask = value,
            2 => values.foreground = value,
            3 => values.background = value,
            4 => values.line_width = u16::try_from(value).unwrap_or(u16::MAX),
            8 => values.fill_style = u8::try_from(value).unwrap_or(u8::MAX),
            14 => values.font = (value != 0).then(|| XResourceId::new(u64::from(value), 1)),
            17 => values.clip_x_origin = value as i16,
            18 => values.clip_y_origin = value as i16,
            _ => {}
        }
    }
    Ok(XWireRequest::ChangeGraphicsContext {
        gc: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        value_mask,
        values,
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
    let value_mask = context.byte_order.u32(&bytes[8..12]);
    let value_count = usize::try_from(value_mask.count_ones()).unwrap_or(usize::MAX);
    let expected_len =
        X_CHANGE_WINDOW_ATTRIBUTES_REQ_LEN.saturating_add(value_count.saturating_mul(4));
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CHANGE_WINDOW_ATTRIBUTES,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let mut event_mask = None;
    let mut do_not_propagate_mask = None;
    let mut value_cursor = X_CHANGE_WINDOW_ATTRIBUTES_REQ_LEN;
    for bit in 0..15 {
        if value_mask & (1 << bit) == 0 {
            continue;
        }
        let value = context
            .byte_order
            .u32(&bytes[value_cursor..value_cursor + 4]);
        value_cursor += 4;
        match bit {
            11 => event_mask = Some(value),
            12 => do_not_propagate_mask = Some(value),
            _ => {}
        }
    }
    Ok(XWireRequest::ChangeWindowAttributes {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        event_mask,
        do_not_propagate_mask,
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

fn decode_set_input_focus(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_SET_INPUT_FOCUS, X_SET_INPUT_FOCUS_REQ_LEN, bytes.len())?;
    if bytes[1] > 2 {
        return Err(XWireParseError::InvalidValue(u32::from(bytes[1])));
    }
    Ok(XWireRequest::SetInputFocus {
        focus: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        revert_to: bytes[1],
        time: context.byte_order.u32(&bytes[8..12]),
    })
}

fn decode_get_modifier_mapping(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_GET_MODIFIER_MAPPING,
        X_GET_MODIFIER_MAPPING_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::GetModifierMapping)
}

fn decode_get_keyboard_mapping(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_GET_KEYBOARD_MAPPING,
        X_GET_KEYBOARD_MAPPING_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::GetKeyboardMapping {
        first_keycode: bytes[4],
        count: bytes[5],
    })
}

fn decode_create_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_len(X_CREATE_WINDOW, X_CREATE_WINDOW_REQ_LEN, bytes.len())?;
    let value_mask = context.byte_order.u32(&bytes[28..32]);
    let value_count = usize::try_from(value_mask.count_ones()).unwrap_or(usize::MAX);
    let expected_len = X_CREATE_WINDOW_REQ_LEN.saturating_add(value_count.saturating_mul(4));
    if bytes.len() != expected_len {
        return Err(XWireParseError::InvalidLength {
            opcode: X_CREATE_WINDOW,
            expected_at_least: expected_len,
            actual: bytes.len(),
        });
    }
    let mut value_cursor = X_CREATE_WINDOW_REQ_LEN;
    let mut background_pixel = None;
    let mut event_mask = None;
    let mut do_not_propagate_mask = None;
    let mut colormap = None;
    for bit in 0..15 {
        if value_mask & (1 << bit) == 0 {
            continue;
        }
        let value = context
            .byte_order
            .u32(&bytes[value_cursor..value_cursor + 4]);
        value_cursor += 4;
        match bit {
            1 => background_pixel = Some(value),
            11 => event_mask = Some(value),
            12 => do_not_propagate_mask = Some(value),
            13 => colormap = Some(XResourceId::new(u64::from(value), 1)),
            _ => {}
        }
    }
    let window_raw = context.byte_order.u32(&bytes[4..8]);
    context.validate_new_resource_id(window_raw)?;
    let window = XResourceId::new(u64::from(window_raw), 1);
    Ok(XWireRequest::CreateWindow {
        packet: XAuthorityRequestPacket {
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
        },
        parent: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        depth: bytes[1],
        visual: context.byte_order.u32(&bytes[24..28]),
        colormap,
        background_pixel,
        event_mask,
        do_not_propagate_mask,
    })
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

fn decode_reparent_window(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_REPARENT_WINDOW, X_REPARENT_WINDOW_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::ReparentWindow {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        parent: XResourceId::new(u64::from(context.byte_order.u32(&bytes[8..12])), 1),
        x: context.byte_order.i16(&bytes[12..14]),
        y: context.byte_order.i16(&bytes[14..16]),
    })
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

fn decode_get_selection_owner(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_GET_SELECTION_OWNER,
        X_GET_SELECTION_OWNER_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::GetSelectionOwner {
        selection: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_grab_button(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_BUTTON, X_GRAB_BUTTON_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabButton {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        event_mask: context.byte_order.u16(&bytes[8..10]),
        button: bytes[20],
        modifiers: context.byte_order.u16(&bytes[22..24]),
        owner_events: bytes[1] != 0,
        pointer_mode: bytes[10],
        keyboard_mode: bytes[11],
    })
}

fn decode_grab_pointer(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_POINTER, X_GRAB_POINTER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabPointer {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        event_mask: context.byte_order.u16(&bytes[8..10]),
        owner_events: bytes[1] != 0,
        pointer_mode: bytes[10],
        keyboard_mode: bytes[11],
        time: context.byte_order.u32(&bytes[20..24]),
    })
}

fn decode_ungrab_pointer(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_POINTER, X_UNGRAB_POINTER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabPointer {
        time: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_ungrab_button(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_BUTTON, X_UNGRAB_BUTTON_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabButton {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        button: bytes[1],
        modifiers: context.byte_order.u16(&bytes[8..10]),
    })
}

fn decode_grab_keyboard(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_KEYBOARD, X_GRAB_KEYBOARD_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabKeyboard {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        owner_events: bytes[1] != 0,
        time: context.byte_order.u32(&bytes[8..12]),
        pointer_mode: bytes[12],
        keyboard_mode: bytes[13],
    })
}

fn decode_ungrab_keyboard(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_KEYBOARD, X_UNGRAB_KEYBOARD_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabKeyboard {
        time: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_grab_key(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_KEY, X_GRAB_KEY_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabKey {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        modifiers: context.byte_order.u16(&bytes[8..10]),
        key: bytes[10],
        pointer_mode: bytes[11],
        keyboard_mode: bytes[12],
        owner_events: bytes[1] != 0,
    })
}

fn decode_ungrab_key(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_KEY, X_UNGRAB_KEY_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabKey {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        key: bytes[1],
        modifiers: context.byte_order.u16(&bytes[8..10]),
    })
}

fn decode_allow_events(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_ALLOW_EVENTS, X_ALLOW_EVENTS_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::AllowEvents {
        mode: bytes[1],
        time: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_grab_server(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_SERVER, X_GRAB_SERVER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabServer)
}

fn decode_ungrab_server(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_SERVER, X_UNGRAB_SERVER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabServer)
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

fn decode_send_event(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_SEND_EVENT, X_SEND_EVENT_REQ_LEN, bytes.len())?;
    let event_type = bytes[12] & 0x7f;
    if event_type < 9 {
        return Err(XWireParseError::InvalidEventType(event_type));
    }
    let destination = XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1);
    if event_type != 31 {
        let mut event = [0; 32];
        event.copy_from_slice(&bytes[12..44]);
        return Ok(XWireRequest::SendSelectionNotify {
            destination,
            event_mask: context.byte_order.u32(&bytes[8..12]),
            event: XClientEvent::ClientMessage {
                sequence: 0,
                bytes: event,
            },
        });
    }
    let requestor = XResourceId::new(u64::from(context.byte_order.u32(&bytes[20..24])), 1);
    Ok(XWireRequest::SendSelectionNotify {
        destination,
        event_mask: context.byte_order.u32(&bytes[8..12]),
        event: XClientEvent::SelectionNotify {
            sequence: 0,
            time: context.byte_order.u32(&bytes[16..20]),
            requestor,
            selection: context.byte_order.u32(&bytes[24..28]),
            target: context.byte_order.u32(&bytes[28..32]),
            property: context.byte_order.u32(&bytes[32..36]),
        },
    })
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

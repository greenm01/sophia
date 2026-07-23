use crate::{
    X_ATOM_NONE, XAuthorityRuntimeError, XByteOrder, XResourceId, XTimestamp, XWireParseError,
    padded_len,
};
use sophia_protocol::Rect;

pub const X_CLIENT_OUTPUT_RECORD_LEN: usize = 32;

const X_ERROR: u8 = 0;
const X_KEY_PRESS: u8 = 2;
const X_KEY_RELEASE: u8 = 3;
const X_BUTTON_PRESS: u8 = 4;
const X_BUTTON_RELEASE: u8 = 5;
const X_MOTION_NOTIFY: u8 = 6;
const X_FOCUS_IN: u8 = 9;
const X_FOCUS_OUT: u8 = 10;
const X_EXPOSE: u8 = 12;
const X_MAP_NOTIFY: u8 = 19;
const X_CONFIGURE_NOTIFY: u8 = 22;
const X_PROPERTY_NOTIFY: u8 = 28;
const X_SELECTION_NOTIFY: u8 = 31;

const PROPERTY_NEW_VALUE: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XErrorCode {
    BadRequest,
    BadValue,
    BadWindow,
    BadAtom,
    BadAccess,
    BadIdChoice,
    BadLength,
    BadImplementation,
}

impl XErrorCode {
    pub const fn wire_code(self) -> u8 {
        match self {
            Self::BadRequest => 1,
            Self::BadValue => 2,
            Self::BadWindow => 3,
            Self::BadAtom => 5,
            Self::BadAccess => 10,
            Self::BadIdChoice => 14,
            Self::BadLength => 16,
            Self::BadImplementation => 17,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XClientError {
    pub code: XErrorCode,
    pub sequence: u16,
    pub resource_id: u32,
    pub minor_code: u16,
    pub major_code: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XClientEvent {
    Key {
        sequence: u16,
        pressed: bool,
        keycode: u8,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        state: u16,
    },
    Focus {
        sequence: u16,
        focused: bool,
        detail: u8,
        event: XResourceId,
        mode: u8,
    },
    XkbStateNotify {
        sequence: u16,
        time: XTimestamp,
        modifiers: u8,
        changed: u16,
        keycode: u8,
        event_type: u8,
    },
    PointerMotion {
        sequence: u16,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
    },
    PointerButton {
        sequence: u16,
        pressed: bool,
        button: u8,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
    },
    Expose {
        sequence: u16,
        window: XResourceId,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        count: u16,
    },
    MapNotify {
        sequence: u16,
        event: XResourceId,
        window: XResourceId,
        override_redirect: bool,
    },
    ConfigureNotify {
        sequence: u16,
        event: XResourceId,
        window: XResourceId,
        above_sibling: Option<XResourceId>,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        override_redirect: bool,
    },
    PropertyNotify {
        sequence: u16,
        window: XResourceId,
        atom: u32,
        time: XTimestamp,
        new_value: bool,
    },
    SelectionClear {
        sequence: u16,
        time: XTimestamp,
        owner: XResourceId,
        selection: u32,
    },
    SelectionRequest {
        sequence: u16,
        time: XTimestamp,
        owner: XResourceId,
        requestor: XResourceId,
        selection: u32,
        target: u32,
        property: u32,
    },
    SelectionNotify {
        sequence: u16,
        time: XTimestamp,
        requestor: XResourceId,
        selection: u32,
        target: u32,
        property: u32,
    },
    ClientMessage {
        sequence: u16,
        bytes: [u8; X_CLIENT_OUTPUT_RECORD_LEN],
    },
    ShmCompletion {
        sequence: u16,
        drawable: XResourceId,
        segment: XResourceId,
        offset: u32,
    },
    PresentCompleteNotify {
        sequence: u16,
        event_id: XResourceId,
        window: XResourceId,
        serial: u32,
        ust: u64,
        msc: u64,
        mode: u8,
    },
    PresentIdleNotify {
        sequence: u16,
        event_id: XResourceId,
        window: XResourceId,
        serial: u32,
        pixmap: XResourceId,
        idle_fence: Option<XResourceId>,
    },
    RandrScreenChange {
        sequence: u16,
        timestamp: u32,
        config_timestamp: u32,
        root: XResourceId,
        request_window: XResourceId,
        width: u16,
        height: u16,
        mm_width: u16,
        mm_height: u16,
    },
    RandrCrtcChange {
        sequence: u16,
        timestamp: u32,
        window: XResourceId,
        crtc: u32,
        mode: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    },
    RandrOutputChange {
        sequence: u16,
        timestamp: u32,
        window: XResourceId,
        output: u32,
        crtc: u32,
        mode: u32,
    },
    RandrResourceChange {
        sequence: u16,
        timestamp: u32,
        window: XResourceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XClientReply {
    GrabStatus {
        sequence: u16,
        status: u8,
    },
    InternAtom {
        sequence: u16,
        atom: u32,
    },
    GetAtomName {
        sequence: u16,
        name: String,
    },
    GetGeometry {
        sequence: u16,
        depth: u8,
        root: XResourceId,
        geometry: Rect,
        border_width: u16,
    },
    GetImage {
        sequence: u16,
        depth: u8,
        visual: u32,
        data: Vec<u8>,
    },
    QueryTree {
        sequence: u16,
        root: XResourceId,
        parent: XResourceId,
        children: Vec<XResourceId>,
    },
    GetWindowAttributes {
        sequence: u16,
        visual: u32,
        colormap: XResourceId,
        map_state: u8,
        override_redirect: bool,
    },
    QueryExtension {
        sequence: u16,
        present: bool,
        major_opcode: u8,
        first_event: u8,
        first_error: u8,
    },
    ListExtensions {
        sequence: u16,
    },
    ListFonts {
        sequence: u16,
        names: Vec<String>,
    },
    ListFontsWithInfo {
        sequence: u16,
        names: Vec<String>,
    },
    QueryBestSize {
        sequence: u16,
        width: u16,
        height: u16,
    },
    ShmQueryVersion {
        sequence: u16,
        major_version: u16,
        minor_version: u16,
        shared_pixmaps: bool,
        pixmap_format: u8,
    },
    Dri3QueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    Dri3Open {
        sequence: u16,
    },
    Dri3GetSupportedModifiers {
        sequence: u16,
        window_modifiers: Vec<u64>,
        screen_modifiers: Vec<u64>,
    },
    XfixesQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    PresentQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    PresentQueryCapabilities {
        sequence: u16,
        capabilities: u32,
    },
    RandrQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    RandrGetScreenSizeRange {
        sequence: u16,
        min_width: u16,
        min_height: u16,
        max_width: u16,
        max_height: u16,
    },
    RandrGetScreenResources {
        sequence: u16,
        timestamp: u32,
        crtcs: Vec<u32>,
        outputs: Vec<u32>,
        modes: Vec<XRandrModeInfo>,
    },
    RandrGetOutputInfo {
        sequence: u16,
        timestamp: u32,
        crtc: u32,
        mm_width: u32,
        mm_height: u32,
        crtcs: Vec<u32>,
        modes: Vec<u32>,
        name: Vec<u8>,
    },
    RandrGetOutputProperty {
        sequence: u16,
        property_type: u32,
        bytes_after: u32,
        format: u8,
        data: Vec<u8>,
    },
    RandrGetCrtcInfo {
        sequence: u16,
        timestamp: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        mode: u32,
        outputs: Vec<u32>,
    },
    RandrGetCrtcGammaSize {
        sequence: u16,
        size: u16,
    },
    RandrGetOutputPrimary {
        sequence: u16,
        output: u32,
    },
    RandrGetProviders {
        sequence: u16,
        timestamp: u32,
    },
    RandrGetMonitors {
        sequence: u16,
        timestamp: u32,
        monitors: Vec<XRandrMonitorInfo>,
    },
    XkbUseExtension {
        sequence: u16,
        supported: bool,
        server_major: u16,
        server_minor: u16,
    },
    GlxQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    GlxString {
        sequence: u16,
        value: String,
    },
    GlxVisualConfigs {
        sequence: u16,
        configs: Vec<[u32; 18]>,
    },
    GlxFbConfigs {
        sequence: u16,
        configs: Vec<Vec<(u32, u32)>>,
    },
    GlxIsDirect {
        sequence: u16,
        direct: bool,
    },
    GlxDrawableAttributes {
        sequence: u16,
        attributes: Vec<(u32, u32)>,
    },
    SyncInitialize {
        sequence: u16,
        major_version: u8,
        minor_version: u8,
    },
    XkbGetMap {
        sequence: u16,
        present: u16,
        keysyms: Vec<[u32; 2]>,
        modifier_map: Vec<(u8, u8)>,
    },
    XkbGetCompatMap {
        sequence: u16,
        device_id: u8,
    },
    XkbGetIndicatorMap {
        sequence: u16,
        device_id: u8,
    },
    XkbGetState {
        sequence: u16,
        modifiers: u8,
    },
    XkbGetControls {
        sequence: u16,
    },
    XkbGetNames {
        sequence: u16,
        which: u32,
        min_keycode: u8,
        max_keycode: u8,
        component_atoms: Vec<u32>,
        type_atoms: Vec<u32>,
        key_names: Vec<[u8; 4]>,
    },
    XkbGetDeviceInfo {
        sequence: u16,
        device_id: u8,
        supported: u16,
        unsupported: u16,
    },
    XkbPerClientFlags {
        sequence: u16,
        supported: u32,
        value: u32,
    },
    XiQueryVersion {
        sequence: u16,
        major_version: u16,
        minor_version: u16,
    },
    GeQueryVersion {
        sequence: u16,
        major_version: u16,
        minor_version: u16,
    },
    XiGetClientPointer {
        sequence: u16,
        device_id: u16,
    },
    XiGetExtensionVersion {
        sequence: u16,
        server_major: u16,
        server_minor: u16,
    },
    XiQueryDevice {
        sequence: u16,
        devices: Vec<XXiDeviceInfo>,
    },
    XiQueryPointer {
        sequence: u16,
        root: XResourceId,
        child: XResourceId,
    },
    XiGetFocus {
        sequence: u16,
        focus: XResourceId,
    },
    XiGetProperty {
        sequence: u16,
    },
    BigRequestsEnable {
        sequence: u16,
        maximum_request_length: u32,
    },
    GetInputFocus {
        sequence: u16,
        focus: XResourceId,
        revert_to: u8,
    },
    QueryPointer {
        sequence: u16,
        root: XResourceId,
        child: XResourceId,
        root_x: i16,
        root_y: i16,
        win_x: i16,
        win_y: i16,
        mask: u16,
    },
    GetModifierMapping {
        sequence: u16,
        keycodes_per_modifier: u8,
        keycodes: Vec<u8>,
    },
    GetKeyboardMapping {
        sequence: u16,
        keysyms_per_keycode: u8,
        keysyms: Vec<u32>,
    },
    GetKeyboardControl {
        sequence: u16,
    },
    TranslateCoordinates {
        sequence: u16,
        same_screen: bool,
        child: Option<XResourceId>,
        dst_x: i16,
        dst_y: i16,
    },
    QueryFont {
        sequence: u16,
        font_ascent: i16,
        font_descent: i16,
    },
    GetProperty {
        sequence: u16,
        property_type: u32,
        format: u8,
        bytes_after: u32,
        item_count: u32,
        bytes: Vec<u8>,
    },
    GetSelectionOwner {
        sequence: u16,
        owner: Option<XResourceId>,
    },
    AllocNamedColor {
        sequence: u16,
        pixel: u32,
        red: u16,
        green: u16,
        blue: u16,
    },
    AllocColor {
        sequence: u16,
        pixel: u32,
        red: u16,
        green: u16,
        blue: u16,
    },
    ListProperties {
        sequence: u16,
        atoms: Vec<u32>,
    },
    QueryColors {
        sequence: u16,
        pixels: Vec<u32>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XXiDeviceInfo {
    pub device_id: u16,
    pub device_type: u16,
    pub attachment: u16,
    pub name: String,
    pub classes: Vec<XXiDeviceClass>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XXiDeviceClass {
    Key {
        source_id: u16,
        keys: Vec<u32>,
    },
    Button {
        source_id: u16,
        button_count: u16,
    },
    Valuator {
        source_id: u16,
        number: u16,
        min: i64,
        max: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRandrModeInfo {
    pub id: u32,
    pub width: u16,
    pub height: u16,
    pub refresh_millihz: u32,
    pub name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRandrMonitorInfo {
    pub name: u32,
    pub primary: bool,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub mm_width: u32,
    pub mm_height: u32,
    pub outputs: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XClientOutput {
    Error(XClientError),
    Event(XClientEvent),
    Reply(XClientReply),
}

pub fn encode_x_client_output(byte_order: XByteOrder, output: XClientOutput) -> Vec<u8> {
    match output {
        XClientOutput::Error(error) => encode_x_client_error(byte_order, error).to_vec(),
        XClientOutput::Event(event) => encode_x_client_event(byte_order, event).to_vec(),
        XClientOutput::Reply(reply) => encode_x_client_reply(byte_order, reply),
    }
}

pub fn encode_x_client_reply(byte_order: XByteOrder, reply: XClientReply) -> Vec<u8> {
    match reply {
        XClientReply::GrabStatus { sequence, status } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = status;
            out
        }
        XClientReply::InternAtom { sequence, atom } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], atom);
            out
        }
        XClientReply::GetAtomName { sequence, name } => {
            let bytes = name.as_bytes();
            let padded_name_len = padded_len(bytes.len());
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_name_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(padded_name_len / 4).unwrap_or(0),
            );
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::try_from(bytes.len()).unwrap_or(0),
            );
            out[X_CLIENT_OUTPUT_RECORD_LEN..X_CLIENT_OUTPUT_RECORD_LEN + bytes.len()]
                .copy_from_slice(bytes);
            out
        }
        XClientReply::GetGeometry {
            sequence,
            depth,
            root,
            geometry,
            border_width,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = depth;
            put_resource(byte_order, &mut out[8..12], root);
            put_i16(
                byte_order,
                &mut out[12..14],
                i16::try_from(geometry.x).unwrap_or(0),
            );
            put_i16(
                byte_order,
                &mut out[14..16],
                i16::try_from(geometry.y).unwrap_or(0),
            );
            put_u16(
                byte_order,
                &mut out[16..18],
                u16::try_from(geometry.width).unwrap_or(0),
            );
            put_u16(
                byte_order,
                &mut out[18..20],
                u16::try_from(geometry.height).unwrap_or(0),
            );
            put_u16(byte_order, &mut out[20..22], border_width);
            out
        }
        XClientReply::GetImage {
            sequence,
            depth,
            visual,
            data,
        } => {
            let padded_len = (data.len() + 3) & !3;
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(padded_len / 4).unwrap_or(u32::MAX),
            );
            out[1] = depth;
            put_u32(byte_order, &mut out[8..12], visual);
            out[X_CLIENT_OUTPUT_RECORD_LEN..X_CLIENT_OUTPUT_RECORD_LEN + data.len()]
                .copy_from_slice(&data);
            out
        }
        XClientReply::QueryTree {
            sequence,
            root,
            parent,
            children,
        } => {
            let children_len = children.len().saturating_mul(4);
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + children_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(children_len / 4).unwrap_or(0),
            );
            put_resource(byte_order, &mut out[8..12], root);
            put_resource(byte_order, &mut out[12..16], parent);
            put_u16(
                byte_order,
                &mut out[16..18],
                u16::try_from(children.len()).unwrap_or(0),
            );
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for child in children {
                put_resource(byte_order, &mut out[offset..offset + 4], child);
                offset += 4;
            }
            out
        }
        XClientReply::GetWindowAttributes {
            sequence,
            visual,
            colormap,
            map_state,
            override_redirect,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + 12];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                3,
            );
            out[1] = 0;
            put_u32(byte_order, &mut out[8..12], visual);
            put_u16(byte_order, &mut out[12..14], 1);
            out[14] = 0;
            out[15] = 1;
            put_u32(byte_order, &mut out[16..20], 0);
            put_u32(byte_order, &mut out[20..24], 0);
            out[24] = 0;
            out[25] = 1;
            out[26] = map_state;
            out[27] = u8::from(override_redirect);
            put_resource(byte_order, &mut out[28..32], colormap);
            out
        }
        XClientReply::QueryExtension {
            sequence,
            present,
            major_opcode,
            first_event,
            first_error,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[8] = u8::from(present);
            out[9] = major_opcode;
            out[10] = first_event;
            out[11] = first_error;
            out
        }
        XClientReply::ListExtensions { sequence } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = 0;
            out
        }
        XClientReply::ListFonts { sequence, names } => {
            let names_len = names.iter().map(|name| 1 + name.len()).sum::<usize>();
            let padded_names_len = padded_len(names_len);
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_names_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(padded_names_len / 4).unwrap_or(0),
            );
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::try_from(names.len()).unwrap_or(0),
            );
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for name in names {
                let bytes = name.as_bytes();
                out[offset] = u8::try_from(bytes.len()).unwrap_or(0);
                offset += 1;
                out[offset..offset + bytes.len()].copy_from_slice(bytes);
                offset += bytes.len();
            }
            out
        }
        XClientReply::ListFontsWithInfo { sequence, names } => {
            let mut out = Vec::new();
            for name in names {
                out.extend(encode_font_info_reply(
                    byte_order,
                    sequence,
                    8,
                    2,
                    Some(name.as_bytes()),
                ));
            }
            out.extend(encode_font_info_reply(byte_order, sequence, 0, 0, None));
            out
        }
        XClientReply::QueryBestSize {
            sequence,
            width,
            height,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u16(byte_order, &mut out[8..10], width);
            put_u16(byte_order, &mut out[10..12], height);
            out
        }
        XClientReply::ShmQueryVersion {
            sequence,
            major_version,
            minor_version,
            shared_pixmaps,
            pixmap_format,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = u8::from(shared_pixmaps);
            put_u16(byte_order, &mut out[8..10], major_version);
            put_u16(byte_order, &mut out[10..12], minor_version);
            out[16] = pixmap_format;
            out
        }
        XClientReply::Dri3QueryVersion {
            sequence,
            major_version,
            minor_version,
        }
        | XClientReply::XfixesQueryVersion {
            sequence,
            major_version,
            minor_version,
        }
        | XClientReply::PresentQueryVersion {
            sequence,
            major_version,
            minor_version,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], major_version);
            put_u32(byte_order, &mut out[12..16], minor_version);
            out
        }
        XClientReply::Dri3Open { sequence } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = 1;
            out
        }
        XClientReply::Dri3GetSupportedModifiers {
            sequence,
            window_modifiers,
            screen_modifiers,
        } => {
            let modifier_count = window_modifiers
                .len()
                .saturating_add(screen_modifiers.len());
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + modifier_count.saturating_mul(8)];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                u32::try_from(modifier_count.saturating_mul(2)).unwrap_or(u32::MAX),
            );
            put_u32(
                byte_order,
                &mut out[8..12],
                u32::try_from(window_modifiers.len()).unwrap_or(u32::MAX),
            );
            put_u32(
                byte_order,
                &mut out[12..16],
                u32::try_from(screen_modifiers.len()).unwrap_or(u32::MAX),
            );
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for modifier in window_modifiers.into_iter().chain(screen_modifiers) {
                put_u64(byte_order, &mut out[offset..offset + 8], modifier);
                offset += 8;
            }
            out
        }
        XClientReply::PresentQueryCapabilities {
            sequence,
            capabilities,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], capabilities);
            out
        }
        XClientReply::RandrQueryVersion {
            sequence,
            major_version,
            minor_version,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], major_version);
            put_u32(byte_order, &mut out[12..16], minor_version);
            out
        }
        XClientReply::RandrGetScreenSizeRange {
            sequence,
            min_width,
            min_height,
            max_width,
            max_height,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u16(byte_order, &mut out[8..10], min_width);
            put_u16(byte_order, &mut out[10..12], min_height);
            put_u16(byte_order, &mut out[12..14], max_width);
            put_u16(byte_order, &mut out[14..16], max_height);
            out
        }
        XClientReply::RandrGetCrtcGammaSize { sequence, size } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u16(byte_order, &mut out[8..10], size);
            out
        }
        XClientReply::RandrGetScreenResources {
            sequence,
            timestamp,
            crtcs,
            outputs,
            modes,
        } => {
            let names_len = modes.iter().map(|mode| mode.name.len()).sum::<usize>();
            let payload_len = crtcs.len() * 4 + outputs.len() * 4 + modes.len() * 32 + names_len;
            let padded_payload_len = (payload_len + 3) & !3;
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_payload_len];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                u32::try_from(padded_payload_len / 4).unwrap_or(u32::MAX),
            );
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_u32(byte_order, &mut out[12..16], timestamp);
            put_u16(byte_order, &mut out[16..18], crtcs.len() as u16);
            put_u16(byte_order, &mut out[18..20], outputs.len() as u16);
            put_u16(byte_order, &mut out[20..22], modes.len() as u16);
            put_u16(byte_order, &mut out[22..24], names_len as u16);
            let mut offset = 32;
            for id in crtcs.iter().chain(outputs.iter()) {
                put_u32(byte_order, &mut out[offset..offset + 4], *id);
                offset += 4;
            }
            for mode in &modes {
                put_u32(byte_order, &mut out[offset..offset + 4], mode.id);
                put_u16(byte_order, &mut out[offset + 4..offset + 6], mode.width);
                put_u16(byte_order, &mut out[offset + 6..offset + 8], mode.height);
                let dot_clock = u64::from(mode.width)
                    .saturating_mul(u64::from(mode.height))
                    .saturating_mul(u64::from(mode.refresh_millihz))
                    / 1_000;
                put_u32(
                    byte_order,
                    &mut out[offset + 8..offset + 12],
                    u32::try_from(dot_clock).unwrap_or(u32::MAX),
                );
                put_u16(byte_order, &mut out[offset + 12..offset + 14], mode.width);
                put_u16(byte_order, &mut out[offset + 14..offset + 16], mode.width);
                put_u16(byte_order, &mut out[offset + 16..offset + 18], mode.width);
                put_u16(byte_order, &mut out[offset + 20..offset + 22], mode.height);
                put_u16(byte_order, &mut out[offset + 22..offset + 24], mode.height);
                put_u16(byte_order, &mut out[offset + 24..offset + 26], mode.height);
                put_u16(
                    byte_order,
                    &mut out[offset + 26..offset + 28],
                    mode.name.len() as u16,
                );
                offset += 32;
            }
            for mode in modes {
                let end = offset + mode.name.len();
                out[offset..end].copy_from_slice(&mode.name);
                offset = end;
            }
            out
        }
        XClientReply::RandrGetOutputInfo {
            sequence,
            timestamp,
            crtc,
            mm_width,
            mm_height,
            crtcs,
            modes,
            name,
        } => {
            let payload_len = crtcs.len() * 4 + modes.len() * 4 + name.len();
            let padded_payload_len = (payload_len + 3) & !3;
            let mut out = vec![0; 32 + padded_payload_len];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                (padded_payload_len / 4) as u32,
            );
            out[1] = 0;
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_u32(byte_order, &mut out[12..16], crtc);
            put_u32(byte_order, &mut out[16..20], mm_width);
            put_u32(byte_order, &mut out[20..24], mm_height);
            out[24] = 0;
            out[25] = 0;
            put_u16(byte_order, &mut out[26..28], crtcs.len() as u16);
            put_u16(byte_order, &mut out[28..30], modes.len() as u16);
            put_u16(byte_order, &mut out[30..32], u16::from(!modes.is_empty()));
            out.extend_from_slice(&[0; 4]);
            put_u16(byte_order, &mut out[34..36], name.len() as u16);
            let mut payload = Vec::with_capacity(padded_payload_len);
            for id in crtcs.iter().chain(modes.iter()) {
                push_u32(byte_order, &mut payload, *id);
            }
            payload.extend_from_slice(&name);
            payload.resize(padded_payload_len, 0);
            out.truncate(36);
            out.extend_from_slice(&payload);
            let reply_units = (out.len().saturating_sub(32) + 3) / 4;
            out.resize(32 + reply_units * 4, 0);
            put_u32(byte_order, &mut out[4..8], reply_units as u32);
            out
        }
        XClientReply::RandrGetOutputProperty {
            sequence,
            property_type,
            bytes_after,
            format,
            data,
        } => {
            let padded_len = (data.len() + 3) & !3;
            let mut out = vec![0; 32 + padded_len];
            write_reply_header(byte_order, &mut out, sequence, (padded_len / 4) as u32);
            out[1] = format;
            put_u32(byte_order, &mut out[8..12], property_type);
            put_u32(byte_order, &mut out[12..16], bytes_after);
            let item_width = usize::from(format).checked_div(8).unwrap_or(0);
            let items = if item_width == 0 {
                0
            } else {
                data.len() / item_width
            };
            put_u32(
                byte_order,
                &mut out[16..20],
                u32::try_from(items).unwrap_or(u32::MAX),
            );
            out[32..32 + data.len()].copy_from_slice(&data);
            out
        }
        XClientReply::RandrGetCrtcInfo {
            sequence,
            timestamp,
            x,
            y,
            width,
            height,
            mode,
            outputs,
        } => {
            let payload_len = outputs.len() * 8;
            let mut out = vec![0; 32 + payload_len];
            write_reply_header(byte_order, &mut out, sequence, (payload_len / 4) as u32);
            out[1] = 0;
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_i16(byte_order, &mut out[12..14], x);
            put_i16(byte_order, &mut out[14..16], y);
            put_u16(byte_order, &mut out[16..18], width);
            put_u16(byte_order, &mut out[18..20], height);
            put_u32(byte_order, &mut out[20..24], mode);
            put_u16(byte_order, &mut out[24..26], 1);
            put_u16(byte_order, &mut out[26..28], 1);
            put_u16(byte_order, &mut out[28..30], outputs.len() as u16);
            put_u16(byte_order, &mut out[30..32], outputs.len() as u16);
            let mut offset = 32;
            for id in outputs.iter().chain(outputs.iter()) {
                put_u32(byte_order, &mut out[offset..offset + 4], *id);
                offset += 4;
            }
            out
        }
        XClientReply::RandrGetOutputPrimary { sequence, output } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], output);
            out
        }
        XClientReply::RandrGetProviders {
            sequence,
            timestamp,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_u16(byte_order, &mut out[12..14], 0);
            out
        }
        XClientReply::RandrGetMonitors {
            sequence,
            timestamp,
            monitors,
        } => {
            let payload_len: usize = monitors
                .iter()
                .map(|monitor| 24 + monitor.outputs.len() * 4)
                .sum();
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + payload_len];
            write_reply_header(byte_order, &mut out, sequence, (payload_len / 4) as u32);
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_u32(byte_order, &mut out[12..16], monitors.len() as u32);
            put_u32(
                byte_order,
                &mut out[16..20],
                monitors.iter().map(|m| m.outputs.len() as u32).sum(),
            );
            let mut offset = 32;
            for monitor in monitors {
                put_u32(byte_order, &mut out[offset..offset + 4], monitor.name);
                out[offset + 4] = u8::from(monitor.primary);
                out[offset + 5] = 1;
                put_u16(
                    byte_order,
                    &mut out[offset + 6..offset + 8],
                    monitor.outputs.len() as u16,
                );
                put_i16(byte_order, &mut out[offset + 8..offset + 10], monitor.x);
                put_i16(byte_order, &mut out[offset + 10..offset + 12], monitor.y);
                put_u16(
                    byte_order,
                    &mut out[offset + 12..offset + 14],
                    monitor.width,
                );
                put_u16(
                    byte_order,
                    &mut out[offset + 14..offset + 16],
                    monitor.height,
                );
                put_u32(
                    byte_order,
                    &mut out[offset + 16..offset + 20],
                    monitor.mm_width,
                );
                put_u32(
                    byte_order,
                    &mut out[offset + 20..offset + 24],
                    monitor.mm_height,
                );
                offset += 24;
                for output in monitor.outputs {
                    put_u32(byte_order, &mut out[offset..offset + 4], output);
                    offset += 4;
                }
            }
            out
        }
        XClientReply::XkbUseExtension {
            sequence,
            supported,
            server_major,
            server_minor,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = u8::from(supported);
            put_u16(byte_order, &mut out[8..10], server_major);
            put_u16(byte_order, &mut out[10..12], server_minor);
            out
        }
        XClientReply::GlxQueryVersion {
            sequence,
            major_version,
            minor_version,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], major_version);
            put_u32(byte_order, &mut out[12..16], minor_version);
            out
        }
        XClientReply::GlxString { sequence, value } => {
            let mut bytes = value.into_bytes();
            bytes.push(0);
            let padded = padded_len(bytes.len());
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                u32::try_from(padded / 4).unwrap_or(u32::MAX),
            );
            put_u32(
                byte_order,
                &mut out[12..16],
                u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            );
            out[32..32 + bytes.len()].copy_from_slice(&bytes);
            out
        }
        XClientReply::GlxVisualConfigs { sequence, configs } => {
            let body_len = configs.len() * 18 * 4;
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + body_len];
            write_reply_header(byte_order, &mut out, sequence, (body_len / 4) as u32);
            put_u32(byte_order, &mut out[8..12], configs.len() as u32);
            put_u32(byte_order, &mut out[12..16], 18);
            for (index, value) in configs.into_iter().flatten().enumerate() {
                put_u32(byte_order, &mut out[32 + index * 4..36 + index * 4], value);
            }
            out
        }
        XClientReply::GlxFbConfigs { sequence, configs } => {
            let attributes = configs.first().map_or(0, Vec::len);
            let body_len = configs.len() * attributes * 8;
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + body_len];
            write_reply_header(byte_order, &mut out, sequence, (body_len / 4) as u32);
            put_u32(byte_order, &mut out[8..12], configs.len() as u32);
            put_u32(byte_order, &mut out[12..16], attributes as u32);
            let mut offset = 32;
            for (name, value) in configs.into_iter().flatten() {
                put_u32(byte_order, &mut out[offset..offset + 4], name);
                put_u32(byte_order, &mut out[offset + 4..offset + 8], value);
                offset += 8;
            }
            out
        }
        XClientReply::GlxIsDirect { sequence, direct } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[8] = u8::from(direct);
            out
        }
        XClientReply::GlxDrawableAttributes {
            sequence,
            attributes,
        } => {
            let body_len = attributes.len() * 8;
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + body_len];
            write_reply_header(byte_order, &mut out, sequence, (body_len / 4) as u32);
            put_u32(byte_order, &mut out[8..12], attributes.len() as u32);
            let mut offset = 32;
            for (name, value) in attributes {
                put_u32(byte_order, &mut out[offset..offset + 4], name);
                put_u32(byte_order, &mut out[offset + 4..offset + 8], value);
                offset += 8;
            }
            out
        }
        XClientReply::SyncInitialize {
            sequence,
            major_version,
            minor_version,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[8] = major_version;
            out[9] = minor_version;
            out
        }
        XClientReply::XkbGetMap {
            sequence,
            present,
            keysyms,
            modifier_map,
        } => {
            let include_types = present & 1 != 0;
            let include_syms = present & 2 != 0;
            let include_modmap = present & 0x04 != 0;
            let mut body = Vec::new();
            if include_types {
                for _ in 0..4 {
                    body.extend_from_slice(&[1, 1]);
                    push_u16(byte_order, &mut body, 0);
                    body.extend_from_slice(&[2, 1, 0, 0]);
                    body.extend_from_slice(&[1, 1, 1, 1]);
                    push_u16(byte_order, &mut body, 0);
                    body.extend_from_slice(&[0, 0]);
                }
            }
            if include_syms {
                for syms in &keysyms {
                    // One group with two levels. XKB encodes the group count
                    // directly in the low nibble of groupInfo.
                    body.extend_from_slice(&[0, 0, 0, 0, 1, 2]);
                    push_u16(byte_order, &mut body, 2);
                    push_u32(byte_order, &mut body, syms[0]);
                    push_u32(byte_order, &mut body, syms[1]);
                }
            }
            if present & 0x10 != 0 {
                // One zero action-count byte for every key in the advertised
                // keycode range. No action records follow those counts.
                body.resize(body.len().saturating_add(keysyms.len()), 0);
                body.resize(padded_len(body.len()), 0);
            }
            if include_modmap {
                for (keycode, modifiers) in &modifier_map {
                    body.extend_from_slice(&[*keycode, *modifiers]);
                }
                body.resize(padded_len(body.len()), 0);
            }
            let fixed_extra_len = 8usize;
            let reply_units = u32::try_from((fixed_extra_len + body.len()) / 4)
                .expect("bounded XKB map reply length");
            let mut out = vec![0; 40];
            out[0] = 1;
            out[1] = 3;
            put_u16(byte_order, &mut out[2..4], sequence);
            put_u32(byte_order, &mut out[4..8], reply_units);
            out[10] = 8;
            out[11] = u8::MAX;
            put_u16(byte_order, &mut out[12..14], present);
            out[14] = 0;
            out[15] = if include_types { 4 } else { 0 };
            out[16] = if include_types { 4 } else { 0 };
            out[17] = 8;
            put_u16(
                byte_order,
                &mut out[18..20],
                if include_syms {
                    u16::try_from(keysyms.len().saturating_mul(2)).unwrap_or(u16::MAX)
                } else {
                    0
                },
            );
            out[20] = if include_syms {
                u8::try_from(keysyms.len()).unwrap_or(u8::MAX)
            } else {
                0
            };
            out[21] = if present & 0x10 != 0 { 8 } else { 0 };
            out[24] = if present & 0x10 != 0 {
                u8::try_from(keysyms.len()).unwrap_or(u8::MAX)
            } else {
                0
            };
            out[25] = if present & 0x20 != 0 { 8 } else { 0 };
            out[28] = if present & 0x08 != 0 { 8 } else { 0 };
            out[31] = if include_modmap { 8 } else { 0 };
            out[32] = if include_modmap { 248 } else { 0 };
            out[33] = if include_modmap {
                u8::try_from(modifier_map.len()).unwrap_or(u8::MAX)
            } else {
                0
            };
            out[34] = if present & 0x80 != 0 { 8 } else { 0 };
            out.extend_from_slice(&body);
            out
        }
        XClientReply::XkbGetCompatMap {
            sequence,
            device_id,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = device_id;
            out
        }
        XClientReply::XkbGetIndicatorMap {
            sequence,
            device_id,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = device_id;
            out
        }
        XClientReply::XkbGetState {
            sequence,
            modifiers,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = 3;
            out[8] = modifiers;
            out[9] = modifiers;
            out[18] = modifiers;
            out[20] = modifiers;
            out
        }
        XClientReply::XkbGetControls { sequence } => {
            let mut out = vec![0; 92];
            write_reply_header(byte_order, &mut out[..32], sequence, 15);
            out[1] = 3;
            out[9] = 1;
            put_u16(byte_order, &mut out[20..22], 660);
            put_u16(byte_order, &mut out[22..24], 40);
            out
        }
        XClientReply::XkbGetNames {
            sequence,
            which,
            min_keycode,
            max_keycode,
            component_atoms,
            type_atoms,
            key_names,
        } => {
            let mut body = Vec::new();
            for atom in component_atoms {
                push_u32(byte_order, &mut body, atom);
            }
            if which & 0x40 != 0 {
                for atom in &type_atoms {
                    push_u32(byte_order, &mut body, *atom);
                }
            }
            if which & 0x80 != 0 {
                // Level names are optional. A zero count for every type asks
                // libxkbcommon to install its normal unnamed-level fallback.
                body.resize(body.len().saturating_add(type_atoms.len()), 0);
                body.resize(padded_len(body.len()), 0);
            }
            if which & 0x200 != 0 {
                for name in &key_names {
                    body.extend_from_slice(name);
                }
            }
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                u32::try_from(body.len() / 4).unwrap_or(u32::MAX),
            );
            out[1] = 3;
            put_u32(byte_order, &mut out[8..12], which);
            out[12] = min_keycode;
            out[13] = max_keycode;
            out[14] = u8::try_from(type_atoms.len()).unwrap_or(u8::MAX);
            out[18] = min_keycode;
            out[19] = u8::try_from(key_names.len()).unwrap_or(u8::MAX);
            out.extend_from_slice(&body);
            out
        }
        XClientReply::XkbGetDeviceInfo {
            sequence,
            device_id,
            supported,
            unsupported,
        } => {
            let mut out = vec![0; 36];
            write_reply_header(byte_order, &mut out[..32], sequence, 1);
            out[1] = device_id;
            put_u16(byte_order, &mut out[10..12], supported);
            put_u16(byte_order, &mut out[12..14], unsupported);
            out[21] = 1;
            out
        }
        XClientReply::XkbPerClientFlags {
            sequence,
            supported,
            value,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = 3;
            put_u32(byte_order, &mut out[8..12], supported);
            put_u32(byte_order, &mut out[12..16], value);
            out
        }
        XClientReply::XiQueryVersion {
            sequence,
            major_version,
            minor_version,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = crate::X_INPUT_QUERY_VERSION_MINOR_OPCODE;
            put_u16(byte_order, &mut out[8..10], major_version);
            put_u16(byte_order, &mut out[10..12], minor_version);
            out
        }
        XClientReply::GeQueryVersion {
            sequence,
            major_version,
            minor_version,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u16(byte_order, &mut out[8..10], major_version);
            put_u16(byte_order, &mut out[10..12], minor_version);
            out
        }
        XClientReply::XiGetClientPointer {
            sequence,
            device_id,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[8] = 1;
            put_u16(byte_order, &mut out[10..12], device_id);
            out
        }
        XClientReply::XiGetExtensionVersion {
            sequence,
            server_major,
            server_minor,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = crate::X_INPUT_GET_EXTENSION_VERSION_MINOR_OPCODE;
            put_u16(byte_order, &mut out[8..10], server_major);
            put_u16(byte_order, &mut out[10..12], server_minor);
            out[12] = 1;
            out
        }
        XClientReply::XiQueryDevice { sequence, devices } => {
            let mut body = Vec::new();
            for device in &devices {
                push_u16(byte_order, &mut body, device.device_id);
                push_u16(byte_order, &mut body, device.device_type);
                push_u16(byte_order, &mut body, device.attachment);
                push_u16(
                    byte_order,
                    &mut body,
                    u16::try_from(device.classes.len()).unwrap_or(0),
                );
                push_u16(
                    byte_order,
                    &mut body,
                    u16::try_from(device.name.len()).unwrap_or(0),
                );
                body.extend_from_slice(&[1, 0]);
                body.extend_from_slice(device.name.as_bytes());
                body.resize(padded_len(body.len()), 0);
                for class in &device.classes {
                    match class {
                        XXiDeviceClass::Key { source_id, keys } => {
                            push_u16(byte_order, &mut body, 0);
                            push_u16(
                                byte_order,
                                &mut body,
                                u16::try_from(2 + keys.len()).unwrap_or(u16::MAX),
                            );
                            push_u16(byte_order, &mut body, *source_id);
                            push_u16(
                                byte_order,
                                &mut body,
                                u16::try_from(keys.len()).unwrap_or(0),
                            );
                            for key in keys {
                                push_u32(byte_order, &mut body, *key);
                            }
                        }
                        XXiDeviceClass::Button {
                            source_id,
                            button_count,
                        } => {
                            push_u16(byte_order, &mut body, 1);
                            push_u16(byte_order, &mut body, 2 + 1 + *button_count * 1);
                            push_u16(byte_order, &mut body, *source_id);
                            push_u16(byte_order, &mut body, *button_count);
                            push_u32(byte_order, &mut body, 0);
                            for _ in 0..*button_count {
                                push_u32(byte_order, &mut body, 0);
                            }
                        }
                        XXiDeviceClass::Valuator {
                            source_id,
                            number,
                            min,
                            max,
                        } => {
                            push_u16(byte_order, &mut body, 2);
                            push_u16(byte_order, &mut body, 11);
                            push_u16(byte_order, &mut body, *source_id);
                            push_u16(byte_order, &mut body, *number);
                            push_u32(byte_order, &mut body, 0);
                            push_i64(byte_order, &mut body, *min);
                            push_i64(byte_order, &mut body, *max);
                            push_i64(byte_order, &mut body, 0);
                            push_u32(byte_order, &mut body, 1);
                            body.extend_from_slice(&[0; 4]);
                        }
                    }
                }
            }
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(
                byte_order,
                &mut out,
                sequence,
                u32::try_from(body.len() / 4).unwrap_or(0),
            );
            out[1] = crate::X_INPUT_QUERY_DEVICE_MINOR_OPCODE;
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::try_from(devices.len()).unwrap_or(0),
            );
            out.extend_from_slice(&body);
            out
        }
        XClientReply::XiQueryPointer {
            sequence,
            root,
            child,
        } => {
            let mut out = vec![0; 56];
            write_reply_header(byte_order, &mut out, sequence, 6);
            put_resource(byte_order, &mut out[8..12], root);
            put_resource(byte_order, &mut out[12..16], child);
            out[32] = 1;
            out
        }
        XClientReply::XiGetFocus { sequence, focus } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = crate::X_INPUT_GET_FOCUS_MINOR_OPCODE;
            put_resource(byte_order, &mut out[8..12], focus);
            out
        }
        XClientReply::XiGetProperty { sequence } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = crate::X_INPUT_GET_PROPERTY_MINOR_OPCODE;
            out
        }
        XClientReply::BigRequestsEnable {
            sequence,
            maximum_request_length,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], maximum_request_length);
            out
        }
        XClientReply::GetInputFocus {
            sequence,
            focus,
            revert_to,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = revert_to;
            put_resource(byte_order, &mut out[8..12], focus);
            out
        }
        XClientReply::QueryPointer {
            sequence,
            root,
            child,
            root_x,
            root_y,
            win_x,
            win_y,
            mask,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = 1;
            put_resource(byte_order, &mut out[8..12], root);
            put_resource(byte_order, &mut out[12..16], child);
            put_i16(byte_order, &mut out[16..18], root_x);
            put_i16(byte_order, &mut out[18..20], root_y);
            put_i16(byte_order, &mut out[20..22], win_x);
            put_i16(byte_order, &mut out[22..24], win_y);
            put_u16(byte_order, &mut out[24..26], mask);
            out
        }
        XClientReply::GetModifierMapping {
            sequence,
            keycodes_per_modifier,
            keycodes,
        } => {
            let padded_keycodes_len = padded_len(keycodes.len());
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_keycodes_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(padded_keycodes_len / 4).unwrap_or(0),
            );
            out[1] = keycodes_per_modifier;
            out[X_CLIENT_OUTPUT_RECORD_LEN..X_CLIENT_OUTPUT_RECORD_LEN + keycodes.len()]
                .copy_from_slice(&keycodes);
            out
        }
        XClientReply::GetKeyboardMapping {
            sequence,
            keysyms_per_keycode,
            keysyms,
        } => {
            let keysyms_len = keysyms.len().saturating_mul(4);
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + keysyms_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(keysyms_len / 4).unwrap_or(0),
            );
            out[1] = keysyms_per_keycode;
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for keysym in keysyms {
                put_u32(byte_order, &mut out[offset..offset + 4], keysym);
                offset += 4;
            }
            out
        }
        XClientReply::GetKeyboardControl { sequence } => {
            let mut out = vec![0; 52];
            write_reply_header(byte_order, &mut out, sequence, 5);
            out[1] = 1;
            out[13] = 50;
            put_u16(byte_order, &mut out[14..16], 400);
            put_u16(byte_order, &mut out[16..18], 100);
            out[20..52].fill(0xff);
            out
        }
        XClientReply::TranslateCoordinates {
            sequence,
            same_screen,
            child,
            dst_x,
            dst_y,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            out[1] = u8::from(same_screen);
            put_resource(
                byte_order,
                &mut out[8..12],
                child.unwrap_or(XResourceId::NONE),
            );
            put_i16(byte_order, &mut out[12..14], dst_x);
            put_i16(byte_order, &mut out[14..16], dst_y);
            out
        }
        XClientReply::QueryFont {
            sequence,
            font_ascent,
            font_descent,
        } => encode_font_info_reply(byte_order, sequence, font_ascent, font_descent, None),
        XClientReply::GetProperty {
            sequence,
            property_type,
            format,
            bytes_after,
            item_count,
            bytes,
        } => {
            let padded_value_len = padded_len(bytes.len());
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + padded_value_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(padded_value_len / 4).unwrap_or(0),
            );
            out[1] = format;
            put_u32(byte_order, &mut out[8..12], property_type);
            put_u32(byte_order, &mut out[12..16], bytes_after);
            put_u32(byte_order, &mut out[16..20], item_count);
            out[X_CLIENT_OUTPUT_RECORD_LEN..X_CLIENT_OUTPUT_RECORD_LEN + bytes.len()]
                .copy_from_slice(&bytes);
            out
        }
        XClientReply::GetSelectionOwner { sequence, owner } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(
                byte_order,
                &mut out[8..12],
                owner
                    .map(|resource| u32::try_from(resource.local.raw()).unwrap_or(0))
                    .unwrap_or(0),
            );
            out
        }
        XClientReply::AllocNamedColor {
            sequence,
            pixel,
            red,
            green,
            blue,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], pixel);
            put_u16(byte_order, &mut out[12..14], red);
            put_u16(byte_order, &mut out[14..16], green);
            put_u16(byte_order, &mut out[16..18], blue);
            put_u16(byte_order, &mut out[18..20], red);
            put_u16(byte_order, &mut out[20..22], green);
            put_u16(byte_order, &mut out[22..24], blue);
            out
        }
        XClientReply::AllocColor {
            sequence,
            pixel,
            red,
            green,
            blue,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u16(byte_order, &mut out[8..10], red);
            put_u16(byte_order, &mut out[10..12], green);
            put_u16(byte_order, &mut out[12..14], blue);
            put_u32(byte_order, &mut out[16..20], pixel);
            out
        }
        XClientReply::ListProperties { sequence, atoms } => {
            let atoms_len = atoms.len().saturating_mul(4);
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + atoms_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(atoms.len()).unwrap_or(0),
            );
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::try_from(atoms.len()).unwrap_or(0),
            );
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for atom in atoms {
                put_u32(byte_order, &mut out[offset..offset + 4], atom);
                offset += 4;
            }
            out
        }
        XClientReply::QueryColors { sequence, pixels } => {
            let colors_len = pixels.len().saturating_mul(8);
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN + colors_len];
            write_reply_header(
                byte_order,
                &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
                sequence,
                u32::try_from(colors_len / 4).unwrap_or(0),
            );
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::try_from(pixels.len()).unwrap_or(0),
            );
            let mut offset = X_CLIENT_OUTPUT_RECORD_LEN;
            for pixel in pixels {
                let intensity = if pixel == 0 { 0 } else { u16::MAX };
                put_u16(byte_order, &mut out[offset..offset + 2], intensity);
                put_u16(byte_order, &mut out[offset + 2..offset + 4], intensity);
                put_u16(byte_order, &mut out[offset + 4..offset + 6], intensity);
                offset += 8;
            }
            out
        }
    }
}

pub fn encode_x_client_error(
    byte_order: XByteOrder,
    error: XClientError,
) -> [u8; X_CLIENT_OUTPUT_RECORD_LEN] {
    let mut out = [0; X_CLIENT_OUTPUT_RECORD_LEN];
    out[0] = X_ERROR;
    out[1] = error.code.wire_code();
    put_u16(byte_order, &mut out[2..4], error.sequence);
    put_u32(byte_order, &mut out[4..8], error.resource_id);
    put_u16(byte_order, &mut out[8..10], error.minor_code);
    out[10] = error.major_code;
    out
}

pub fn encode_x_client_event(byte_order: XByteOrder, event: XClientEvent) -> Vec<u8> {
    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
    match event {
        XClientEvent::Key {
            sequence,
            pressed,
            keycode,
            time,
            root,
            event,
            state,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                if pressed { X_KEY_PRESS } else { X_KEY_RELEASE },
                keycode,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], root);
            put_resource(byte_order, &mut out[12..16], event);
            put_resource(byte_order, &mut out[16..20], XResourceId::NONE);
            put_i16(byte_order, &mut out[20..22], 0);
            put_i16(byte_order, &mut out[22..24], 0);
            put_i16(byte_order, &mut out[24..26], 0);
            put_i16(byte_order, &mut out[26..28], 0);
            put_u16(byte_order, &mut out[28..30], state);
            out[30] = 1;
        }
        XClientEvent::Focus {
            sequence,
            focused,
            detail,
            event,
            mode,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                if focused { X_FOCUS_IN } else { X_FOCUS_OUT },
                detail,
                sequence,
            );
            put_resource(byte_order, &mut out[4..8], event);
            out[8] = mode;
        }
        XClientEvent::XkbStateNotify {
            sequence,
            time,
            modifiers,
            changed,
            keycode,
            event_type,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_KEYBOARD_FIRST_EVENT,
                2,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], time);
            out[8] = 3;
            out[9] = modifiers;
            out[10] = modifiers;
            out[16] = modifiers;
            out[18] = modifiers;
            put_u16(byte_order, &mut out[24..26], changed);
            out[26] = keycode;
            out[27] = event_type;
        }
        XClientEvent::PointerMotion {
            sequence,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        } => write_pointer_event(
            byte_order,
            &mut out,
            X_MOTION_NOTIFY,
            0,
            sequence,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        ),
        XClientEvent::PointerButton {
            sequence,
            pressed,
            button,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        } => write_pointer_event(
            byte_order,
            &mut out,
            if pressed {
                X_BUTTON_PRESS
            } else {
                X_BUTTON_RELEASE
            },
            button,
            sequence,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        ),
        XClientEvent::Expose {
            sequence,
            window,
            x,
            y,
            width,
            height,
            count,
        } => {
            write_event_header(byte_order, &mut out, X_EXPOSE, 0, sequence);
            put_resource(byte_order, &mut out[4..8], window);
            put_u16(byte_order, &mut out[8..10], x);
            put_u16(byte_order, &mut out[10..12], y);
            put_u16(byte_order, &mut out[12..14], width);
            put_u16(byte_order, &mut out[14..16], height);
            put_u16(byte_order, &mut out[16..18], count);
        }
        XClientEvent::MapNotify {
            sequence,
            event,
            window,
            override_redirect,
        } => {
            write_event_header(byte_order, &mut out, X_MAP_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], event);
            put_resource(byte_order, &mut out[8..12], window);
            out[12] = u8::from(override_redirect);
        }
        XClientEvent::ConfigureNotify {
            sequence,
            event,
            window,
            above_sibling,
            x,
            y,
            width,
            height,
            border_width,
            override_redirect,
        } => {
            write_event_header(byte_order, &mut out, X_CONFIGURE_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], event);
            put_resource(byte_order, &mut out[8..12], window);
            put_u32(
                byte_order,
                &mut out[12..16],
                above_sibling.map(raw_xid).unwrap_or(0),
            );
            put_i16(byte_order, &mut out[16..18], x);
            put_i16(byte_order, &mut out[18..20], y);
            put_u16(byte_order, &mut out[20..22], width);
            put_u16(byte_order, &mut out[22..24], height);
            put_u16(byte_order, &mut out[24..26], border_width);
            out[26] = u8::from(override_redirect);
        }
        XClientEvent::PropertyNotify {
            sequence,
            window,
            atom,
            time,
            new_value,
        } => {
            write_event_header(byte_order, &mut out, X_PROPERTY_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], window);
            put_u32(byte_order, &mut out[8..12], atom);
            put_u32(byte_order, &mut out[12..16], time);
            out[16] = if new_value { PROPERTY_NEW_VALUE } else { 1 };
        }
        XClientEvent::SelectionClear {
            sequence,
            time,
            owner,
            selection,
        } => {
            write_event_header(byte_order, &mut out, 29, 0, sequence);
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], owner);
            put_u32(byte_order, &mut out[12..16], selection);
        }
        XClientEvent::SelectionRequest {
            sequence,
            time,
            owner,
            requestor,
            selection,
            target,
            property,
        } => {
            write_event_header(byte_order, &mut out, 30, 0, sequence);
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], owner);
            put_resource(byte_order, &mut out[12..16], requestor);
            put_u32(byte_order, &mut out[16..20], selection);
            put_u32(byte_order, &mut out[20..24], target);
            put_u32(byte_order, &mut out[24..28], property);
        }
        XClientEvent::SelectionNotify {
            sequence,
            time,
            requestor,
            selection,
            target,
            property,
        } => {
            write_event_header(byte_order, &mut out, X_SELECTION_NOTIFY, 0, sequence);
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], requestor);
            put_u32(byte_order, &mut out[12..16], selection);
            put_u32(byte_order, &mut out[16..20], target);
            put_u32(byte_order, &mut out[20..24], property);
        }
        XClientEvent::ClientMessage { sequence, bytes } => {
            out = bytes.to_vec();
            put_u16(byte_order, &mut out[2..4], sequence);
        }
        XClientEvent::ShmCompletion {
            sequence,
            drawable,
            segment,
            offset,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_MIT_SHM_FIRST_EVENT,
                0,
                sequence,
            );
            put_resource(byte_order, &mut out[4..8], drawable);
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::from(crate::X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE),
            );
            out[10] = crate::X_MIT_SHM_MAJOR_OPCODE;
            put_resource(byte_order, &mut out[12..16], segment);
            put_u32(byte_order, &mut out[16..20], offset);
        }
        XClientEvent::PresentCompleteNotify {
            sequence,
            event_id,
            window,
            serial,
            ust,
            msc,
            mode,
        } => {
            out.resize(44, 0);
            out[0] = 35;
            out[1] = crate::X_PRESENT_MAJOR_OPCODE;
            put_u16(byte_order, &mut out[2..4], sequence);
            put_u32(byte_order, &mut out[4..8], 3);
            put_u16(byte_order, &mut out[8..10], 1);
            out[10] = 0;
            out[11] = mode;
            put_resource(byte_order, &mut out[12..16], event_id);
            put_resource(byte_order, &mut out[16..20], window);
            put_u32(byte_order, &mut out[20..24], serial);
            put_u64(byte_order, &mut out[24..32], ust);
            put_u32(byte_order, &mut out[32..36], u32::from(sequence));
            put_u64(byte_order, &mut out[36..44], msc);
        }
        XClientEvent::PresentIdleNotify {
            sequence,
            event_id,
            window,
            serial,
            pixmap,
            idle_fence,
        } => {
            out.resize(36, 0);
            out[0] = 35;
            out[1] = crate::X_PRESENT_MAJOR_OPCODE;
            put_u16(byte_order, &mut out[2..4], sequence);
            put_u32(byte_order, &mut out[4..8], 1);
            put_u16(byte_order, &mut out[8..10], 2);
            put_resource(byte_order, &mut out[12..16], event_id);
            put_resource(byte_order, &mut out[16..20], window);
            put_u32(byte_order, &mut out[20..24], serial);
            put_resource(byte_order, &mut out[24..28], pixmap);
            put_resource(
                byte_order,
                &mut out[28..32],
                idle_fence.unwrap_or(XResourceId::NONE),
            );
            put_u32(byte_order, &mut out[32..36], u32::from(sequence));
        }
        XClientEvent::RandrScreenChange {
            sequence,
            timestamp,
            config_timestamp,
            root,
            request_window,
            width,
            height,
            mm_width,
            mm_height,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT,
                1,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_u32(byte_order, &mut out[8..12], config_timestamp);
            put_resource(byte_order, &mut out[12..16], root);
            put_resource(byte_order, &mut out[16..20], request_window);
            put_u16(byte_order, &mut out[20..22], 0);
            put_u16(byte_order, &mut out[22..24], 0);
            put_u16(byte_order, &mut out[24..26], width);
            put_u16(byte_order, &mut out[26..28], height);
            put_u16(byte_order, &mut out[28..30], mm_width);
            put_u16(byte_order, &mut out[30..32], mm_height);
        }
        XClientEvent::RandrCrtcChange {
            sequence,
            timestamp,
            window,
            crtc,
            mode,
            x,
            y,
            width,
            height,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT + 1,
                0,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_resource(byte_order, &mut out[8..12], window);
            put_u32(byte_order, &mut out[12..16], crtc);
            put_u32(byte_order, &mut out[16..20], mode);
            put_u16(byte_order, &mut out[20..22], 1);
            put_i16(byte_order, &mut out[24..26], x);
            put_i16(byte_order, &mut out[26..28], y);
            put_u16(byte_order, &mut out[28..30], width);
            put_u16(byte_order, &mut out[30..32], height);
        }
        XClientEvent::RandrOutputChange {
            sequence,
            timestamp,
            window,
            output,
            crtc,
            mode,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT + 1,
                1,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_resource(byte_order, &mut out[12..16], window);
            put_u32(byte_order, &mut out[16..20], output);
            put_u32(byte_order, &mut out[20..24], crtc);
            put_u32(byte_order, &mut out[24..28], mode);
            put_u16(byte_order, &mut out[28..30], 1);
            out[30] = 0;
            out[31] = 0;
        }
        XClientEvent::RandrResourceChange {
            sequence,
            timestamp,
            window,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT + 1,
                5,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_resource(byte_order, &mut out[8..12], window);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn write_pointer_event(
    byte_order: XByteOrder,
    out: &mut [u8],
    event_type: u8,
    detail: u8,
    sequence: u16,
    time: XTimestamp,
    root: XResourceId,
    event: XResourceId,
    root_x: i16,
    root_y: i16,
    event_x: i16,
    event_y: i16,
    state: u16,
) {
    write_event_header(byte_order, out, event_type, detail, sequence);
    put_u32(byte_order, &mut out[4..8], time);
    put_resource(byte_order, &mut out[8..12], root);
    put_resource(byte_order, &mut out[12..16], event);
    put_resource(byte_order, &mut out[16..20], XResourceId::NONE);
    put_i16(byte_order, &mut out[20..22], root_x);
    put_i16(byte_order, &mut out[22..24], root_y);
    put_i16(byte_order, &mut out[24..26], event_x);
    put_i16(byte_order, &mut out[26..28], event_y);
    put_u16(byte_order, &mut out[28..30], state);
    out[30] = 1;
}

pub fn x_error_from_wire_parse(
    error: &XWireParseError,
    sequence: u16,
    major_code: u8,
    minor_code: u16,
) -> XClientError {
    let code = match error {
        XWireParseError::Truncated { .. }
        | XWireParseError::InvalidLength { .. }
        | XWireParseError::TrailingBytes(_) => XErrorCode::BadLength,
        XWireParseError::UnknownOpcode(_) => XErrorCode::BadRequest,
        XWireParseError::InvalidPropertyMode(_)
        | XWireParseError::InvalidPropertyFormat(_)
        | XWireParseError::InvalidEventType(_)
        | XWireParseError::InvalidValue(_)
        | XWireParseError::PropertyValueTooLarge { .. } => XErrorCode::BadValue,
        XWireParseError::ResourceIdOutsideClientRange { .. } => XErrorCode::BadIdChoice,
    };

    XClientError {
        code,
        sequence,
        resource_id: 0,
        minor_code,
        major_code,
    }
}

pub fn x_error_from_runtime(
    error: XAuthorityRuntimeError,
    sequence: u16,
    major_code: u8,
    resource_id: u32,
) -> XClientError {
    let code = match error {
        XAuthorityRuntimeError::InvalidResource
        | XAuthorityRuntimeError::UnknownResource
        | XAuthorityRuntimeError::WrongResourceKind
        | XAuthorityRuntimeError::InvalidSurface => XErrorCode::BadWindow,
        XAuthorityRuntimeError::InvalidNamespace
        | XAuthorityRuntimeError::CrossNamespaceDenied
        | XAuthorityRuntimeError::StaleGeneration
        | XAuthorityRuntimeError::UnknownRequestorNamespace
        | XAuthorityRuntimeError::MissingSourceNamespace
        | XAuthorityRuntimeError::SameNamespace
        | XAuthorityRuntimeError::PortalRejected => XErrorCode::BadAccess,
        XAuthorityRuntimeError::UnknownSourceOwner => XErrorCode::BadAtom,
    };

    XClientError {
        code,
        sequence,
        resource_id,
        minor_code: 0,
        major_code,
    }
}

pub fn x_selection_failure_event(
    sequence: u16,
    time: XTimestamp,
    requestor: XResourceId,
    selection: u32,
    target: u32,
) -> XClientEvent {
    XClientEvent::SelectionNotify {
        sequence,
        time,
        requestor,
        selection,
        target,
        property: X_ATOM_NONE,
    }
}

fn encode_font_info_reply(
    byte_order: XByteOrder,
    sequence: u16,
    font_ascent: i16,
    font_descent: i16,
    name: Option<&[u8]>,
) -> Vec<u8> {
    let name = name.unwrap_or_default();
    let padded_name_len = padded_len(name.len());
    let mut out = vec![0; 60 + padded_name_len];
    write_reply_header(
        byte_order,
        &mut out[..X_CLIENT_OUTPUT_RECORD_LEN],
        sequence,
        u32::try_from(7 + (padded_name_len / 4)).unwrap_or(7),
    );
    out[1] = u8::try_from(name.len()).unwrap_or(0);
    // min_bounds charinfo
    put_i16(byte_order, &mut out[8..10], 0);
    put_i16(byte_order, &mut out[10..12], 8);
    put_i16(byte_order, &mut out[12..14], 8);
    put_i16(byte_order, &mut out[14..16], 8);
    put_i16(byte_order, &mut out[16..18], 2);
    put_u16(byte_order, &mut out[18..20], 0);
    // max_bounds charinfo
    put_i16(byte_order, &mut out[24..26], 0);
    put_i16(byte_order, &mut out[26..28], 8);
    put_i16(byte_order, &mut out[28..30], 8);
    put_i16(byte_order, &mut out[30..32], 8);
    put_i16(byte_order, &mut out[32..34], 2);
    put_u16(byte_order, &mut out[34..36], 0);
    put_u16(byte_order, &mut out[40..42], 0);
    put_u16(byte_order, &mut out[42..44], 255);
    put_u16(byte_order, &mut out[44..46], 0);
    put_u16(byte_order, &mut out[46..48], 0);
    out[48] = 0;
    out[49] = 0;
    out[50] = 0;
    out[51] = 1;
    put_i16(byte_order, &mut out[52..54], font_ascent);
    put_i16(byte_order, &mut out[54..56], font_descent);
    put_u32(byte_order, &mut out[56..60], 0);
    out[60..60 + name.len()].copy_from_slice(name);
    out
}

fn write_event_header(
    byte_order: XByteOrder,
    out: &mut [u8],
    event_type: u8,
    detail: u8,
    sequence: u16,
) {
    out[0] = event_type;
    out[1] = detail;
    put_u16(byte_order, &mut out[2..4], sequence);
}

fn write_reply_header(byte_order: XByteOrder, out: &mut [u8], sequence: u16, length_units: u32) {
    out[0] = 1;
    put_u16(byte_order, &mut out[2..4], sequence);
    put_u32(byte_order, &mut out[4..8], length_units);
}

fn put_resource(byte_order: XByteOrder, out: &mut [u8], resource: XResourceId) {
    put_u32(byte_order, out, raw_xid(resource));
}

fn raw_xid(resource: XResourceId) -> u32 {
    u32::try_from(resource.local.raw()).unwrap_or(0)
}

fn put_u16(byte_order: XByteOrder, out: &mut [u8], value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

fn put_i16(byte_order: XByteOrder, out: &mut [u8], value: i16) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

fn push_u32(byte_order: XByteOrder, out: &mut Vec<u8>, value: u32) {
    let mut bytes = [0; 4];
    put_u32(byte_order, &mut bytes, value);
    out.extend_from_slice(&bytes);
}

fn push_u16(byte_order: XByteOrder, out: &mut Vec<u8>, value: u16) {
    let mut bytes = [0; 2];
    put_u16(byte_order, &mut bytes, value);
    out.extend_from_slice(&bytes);
}

fn push_i64(byte_order: XByteOrder, out: &mut Vec<u8>, value: i64) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn put_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

fn put_u64(byte_order: XByteOrder, out: &mut [u8], value: u64) {
    let bytes = match byte_order {
        XByteOrder::LittleEndian => value.to_le_bytes(),
        XByteOrder::BigEndian => value.to_be_bytes(),
    };
    out.copy_from_slice(&bytes);
}

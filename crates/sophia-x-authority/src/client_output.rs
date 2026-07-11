use crate::{
    X_ATOM_NONE, XAuthorityRuntimeError, XByteOrder, XResourceId, XTimestamp, XWireParseError,
    padded_len,
};
use sophia_protocol::Rect;

pub const X_CLIENT_OUTPUT_RECORD_LEN: usize = 32;

const X_ERROR: u8 = 0;
const X_KEY_PRESS: u8 = 2;
const X_KEY_RELEASE: u8 = 3;
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
    SelectionNotify {
        sequence: u16,
        time: XTimestamp,
        requestor: XResourceId,
        selection: u32,
        target: u32,
        property: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XClientReply {
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
    },
    RandrGetOutputPrimary {
        sequence: u16,
        output: u32,
    },
    RandrGetMonitors {
        sequence: u16,
        timestamp: u32,
    },
    XkbUseExtension {
        sequence: u16,
        supported: bool,
        server_major: u16,
        server_minor: u16,
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
        XClientReply::RandrGetScreenResources { sequence } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], 0);
            put_u32(byte_order, &mut out[12..16], 0);
            put_u16(byte_order, &mut out[16..18], 0);
            put_u16(byte_order, &mut out[18..20], 0);
            put_u16(byte_order, &mut out[20..22], 0);
            put_u16(byte_order, &mut out[22..24], 0);
            out
        }
        XClientReply::RandrGetOutputPrimary { sequence, output } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], output);
            out
        }
        XClientReply::RandrGetMonitors {
            sequence,
            timestamp,
        } => {
            let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
            write_reply_header(byte_order, &mut out, sequence, 0);
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_u32(byte_order, &mut out[12..16], 0);
            put_u32(byte_order, &mut out[16..20], 0);
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

pub fn encode_x_client_event(
    byte_order: XByteOrder,
    event: XClientEvent,
) -> [u8; X_CLIENT_OUTPUT_RECORD_LEN] {
    let mut out = [0; X_CLIENT_OUTPUT_RECORD_LEN];
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
    }
    out
}

pub fn x_error_from_wire_parse(
    error: &XWireParseError,
    sequence: u16,
    major_code: u8,
) -> XClientError {
    let code = match error {
        XWireParseError::Truncated { .. }
        | XWireParseError::InvalidLength { .. }
        | XWireParseError::TrailingBytes(_) => XErrorCode::BadLength,
        XWireParseError::UnknownOpcode(_) => XErrorCode::BadRequest,
        XWireParseError::InvalidPropertyMode(_)
        | XWireParseError::InvalidPropertyFormat(_)
        | XWireParseError::PropertyValueTooLarge { .. } => XErrorCode::BadValue,
    };

    XClientError {
        code,
        sequence,
        resource_id: 0,
        minor_code: 0,
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
    out: &mut [u8; X_CLIENT_OUTPUT_RECORD_LEN],
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

fn put_u32(byte_order: XByteOrder, out: &mut [u8], value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.copy_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.copy_from_slice(&value.to_be_bytes()),
    }
}

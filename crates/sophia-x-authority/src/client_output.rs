use crate::{
    X_ATOM_NONE, XAuthorityRuntimeError, XByteOrder, XResourceId, XTimestamp, XWireParseError,
};

pub const X_CLIENT_OUTPUT_RECORD_LEN: usize = 32;

const X_ERROR: u8 = 0;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XClientOutput {
    Error(XClientError),
    Event(XClientEvent),
}

pub fn encode_x_client_output(
    byte_order: XByteOrder,
    output: XClientOutput,
) -> [u8; X_CLIENT_OUTPUT_RECORD_LEN] {
    match output {
        XClientOutput::Error(error) => encode_x_client_error(byte_order, error),
        XClientOutput::Event(event) => encode_x_client_event(byte_order, event),
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

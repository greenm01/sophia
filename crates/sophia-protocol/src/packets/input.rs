use crate::geometry::{Point, Transform};
use crate::ids::{DeviceId, SeatId, SurfaceId, XWindowId};

#[derive(Clone, Debug, PartialEq)]
pub struct InputEventPacket {
    pub serial: u64,
    pub seat: SeatId,
    pub device: DeviceId,
    pub time_msec: u64,
    pub kind: InputEventKind,
    pub global_position: Option<Point>,
    pub target_surface: Option<SurfaceId>,
    pub target_window: Option<XWindowId>,
    pub local_position: Option<Point>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEventKind {
    PointerMotion,
    PointerButton { button: u32, pressed: bool },
    Key { keycode: u32, pressed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputRoute {
    pub input_serial: u64,
    pub target_surface: Option<SurfaceId>,
    pub target_window: Option<XWindowId>,
    pub global_position: Point,
    pub local_position: Option<Point>,
    pub transform: Transform,
    pub outcome: InputRouteOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputRouteOutcome {
    Routed,
    NoTarget,
    StaleTarget,
    Denied,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XLibreRoutedInputRequest {
    pub serial: u64,
    pub seat: SeatId,
    pub device: DeviceId,
    pub time_msec: u64,
    pub target_window: XWindowId,
    pub local_position: Point,
    pub kind: InputEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XLibreRoutedInputDecision {
    pub serial: u64,
    pub target_window: XWindowId,
    pub outcome: XLibreRoutedInputOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XLibreRoutedInputOutcome {
    Accepted,
    RejectedStaleTarget,
    RejectedDeniedNamespace,
    RejectedActiveGrab,
    RejectedFocusPolicy,
    RejectedUnsupportedEvent,
}

pub const XLIBRE_ROUTED_INPUT_EXTENSION_NAME: &str = "SOPHIA-ROUTED-INPUT";
pub const XLIBRE_ROUTED_INPUT_MAJOR_VERSION: u16 = 0;
pub const XLIBRE_ROUTED_INPUT_MINOR_VERSION: u16 = 1;
pub const XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE: u8 = 1;
pub const XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH: u16 = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XLibreRoutedInputWireRequest {
    pub serial_hi: u32,
    pub serial_lo: u32,
    pub target_xid: u32,
    pub seat: u32,
    pub device: u32,
    pub time_msec: u32,
    pub local_x_24_8: i32,
    pub local_y_24_8: i32,
    pub event_code: u16,
    pub detail: u16,
    pub flags: u32,
}

impl XLibreRoutedInputRequest {
    pub fn to_wire_request(&self) -> XLibreRoutedInputWireRequest {
        let (event_code, detail, flags) = encode_routed_input_kind(self.kind);

        XLibreRoutedInputWireRequest {
            serial_hi: (self.serial >> 32) as u32,
            serial_lo: self.serial as u32,
            target_xid: self.target_window.xid(),
            seat: self.seat.raw() as u32,
            device: self.device.raw() as u32,
            time_msec: self.time_msec as u32,
            local_x_24_8: fixed_24_8(self.local_position.x),
            local_y_24_8: fixed_24_8(self.local_position.y),
            event_code,
            detail,
            flags,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XLibreRoutedInputWireError {
    UnsupportedEventCode,
    InvalidTarget,
    InvalidSeat,
    InvalidDevice,
}

impl XLibreRoutedInputWireRequest {
    pub fn to_request(self) -> Result<XLibreRoutedInputRequest, XLibreRoutedInputWireError> {
        let target_window = XWindowId::new(self.target_xid, 1);
        let seat = SeatId::from_raw(u64::from(self.seat));
        let device = DeviceId::from_raw(u64::from(self.device));

        if !target_window.is_valid() {
            return Err(XLibreRoutedInputWireError::InvalidTarget);
        }
        if !seat.is_valid() {
            return Err(XLibreRoutedInputWireError::InvalidSeat);
        }
        if !device.is_valid() {
            return Err(XLibreRoutedInputWireError::InvalidDevice);
        }

        Ok(XLibreRoutedInputRequest {
            serial: (u64::from(self.serial_hi) << 32) | u64::from(self.serial_lo),
            seat,
            device,
            time_msec: u64::from(self.time_msec),
            target_window,
            local_position: Point {
                x: f64::from(self.local_x_24_8) / 256.0,
                y: f64::from(self.local_y_24_8) / 256.0,
            },
            kind: decode_routed_input_kind(self.event_code, self.detail, self.flags)?,
        })
    }
}

fn encode_routed_input_kind(kind: InputEventKind) -> (u16, u16, u32) {
    match kind {
        InputEventKind::PointerMotion => (1, 0, 0),
        InputEventKind::PointerButton { button, pressed } => (2, button as u16, u32::from(pressed)),
        InputEventKind::Key { keycode, pressed } => (3, keycode as u16, u32::from(pressed)),
    }
}

fn decode_routed_input_kind(
    event_code: u16,
    detail: u16,
    flags: u32,
) -> Result<InputEventKind, XLibreRoutedInputWireError> {
    let pressed = (flags & 1) != 0;

    match event_code {
        1 => Ok(InputEventKind::PointerMotion),
        2 => Ok(InputEventKind::PointerButton {
            button: u32::from(detail),
            pressed,
        }),
        3 => Ok(InputEventKind::Key {
            keycode: u32::from(detail),
            pressed,
        }),
        _ => Err(XLibreRoutedInputWireError::UnsupportedEventCode),
    }
}

fn fixed_24_8(value: f64) -> i32 {
    (value * 256.0).round() as i32
}

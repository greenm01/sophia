use sophia_protocol::{DeviceId, InputEventKind, Point, SeatId, XWindowId};

pub const XLIBRE_ROUTED_INPUT_EXTENSION_NAME: &str = "SOPHIA-ROUTED-INPUT";
pub const XLIBRE_ROUTED_INPUT_MAJOR_VERSION: u16 = 0;
pub const XLIBRE_ROUTED_INPUT_MINOR_VERSION: u16 = 1;
pub const XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE: u8 = 1;
pub const XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH: u16 = 11;

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
        let (event_code, detail, flags) = match self.kind {
            InputEventKind::PointerMotion => (1, 0, 0),
            InputEventKind::PointerButton { button, pressed } => {
                (2, button as u16, u32::from(pressed))
            }
            InputEventKind::Key { keycode, pressed } => (3, keycode as u16, u32::from(pressed)),
        };

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

fn fixed_24_8(value: f64) -> i32 {
    (value * 256.0).round() as i32
}

use crate::prelude::*;
use crate::state::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SophiaRoutedInputDispatch {
    pub(super) reply: SophiaRoutedInputRouteReply,
    pub(super) elapsed: Duration,
    pub(super) request_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SophiaRoutedInputRouteReply {
    pub(super) serial: u64,
    pub(super) target_window: XWindowId,
    pub(super) outcome: XLibreRoutedInputOutcome,
}

impl TryParse for SophiaRoutedInputRouteReply {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let initial_value = value;
        let (response_type, value) = u8::try_parse(value)?;
        let value = value.get(1..).ok_or(ParseError::InsufficientData)?;
        let (_sequence, value) = u16::try_parse(value)?;
        let (length, value) = u32::try_parse(value)?;
        let (serial_hi, value) = u32::try_parse(value)?;
        let (serial_lo, value) = u32::try_parse(value)?;
        let (target_xid, value) = u32::try_parse(value)?;
        let (outcome, _value) = u16::try_parse(value)?;

        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }

        let target_window = XWindowId::new(target_xid, 1);
        if !target_window.is_valid() {
            return Err(ParseError::InvalidValue);
        }

        let outcome = match outcome {
            0 => XLibreRoutedInputOutcome::Accepted,
            1 => XLibreRoutedInputOutcome::RejectedStaleTarget,
            2 => XLibreRoutedInputOutcome::RejectedDeniedNamespace,
            3 => XLibreRoutedInputOutcome::RejectedActiveGrab,
            4 => XLibreRoutedInputOutcome::RejectedFocusPolicy,
            5 => XLibreRoutedInputOutcome::RejectedUnsupportedEvent,
            _ => return Err(ParseError::InvalidValue),
        };
        let reply = Self {
            serial: (u64::from(serial_hi) << 32) | u64::from(serial_lo),
            target_window,
            outcome,
        };
        let remaining = initial_value
            .get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;

        Ok((reply, remaining))
    }
}
pub(super) fn send_sophia_routed_input_route<C>(
    connection: &C,
    major_opcode: u8,
    request: &XLibreRoutedInputRequest,
) -> Result<SophiaRoutedInputDispatch, XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let wire = request.to_wire_request();
    let mut bytes = Vec::with_capacity(usize::from(XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH) * 4);
    major_opcode.serialize_into(&mut bytes);
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE.serialize_into(&mut bytes);
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH.serialize_into(&mut bytes);
    serialize_routed_input_wire(&wire, &mut bytes);

    let request_bytes = bytes.len();
    let start = Instant::now();
    let cookie = connection
        .send_request_with_reply::<SophiaRoutedInputRouteReply>(&[IoSlice::new(&bytes)], Vec::new())
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?;
    let reply = cookie.reply().map_err(|error| XBridgeError::RoutedInput {
        message: error.to_string(),
    })?;

    Ok(SophiaRoutedInputDispatch {
        reply,
        elapsed: start.elapsed(),
        request_bytes,
    })
}

pub const fn routed_input_request_wire_len() -> usize {
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH as usize * 4
}

fn serialize_routed_input_wire(wire: &XLibreRoutedInputWireRequest, bytes: &mut Vec<u8>) {
    wire.serial_hi.serialize_into(bytes);
    wire.serial_lo.serialize_into(bytes);
    wire.target_xid.serialize_into(bytes);
    wire.seat.serialize_into(bytes);
    wire.device.serialize_into(bytes);
    wire.time_msec.serialize_into(bytes);
    wire.local_x_24_8.serialize_into(bytes);
    wire.local_y_24_8.serialize_into(bytes);
    wire.event_code.serialize_into(bytes);
    wire.detail.serialize_into(bytes);
    wire.flags.serialize_into(bytes);
}

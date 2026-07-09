use crate::{
    BrokerHealthPacket, BrokerHealthState, BrokerKind, SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
    TransactionId,
};

use super::cursor::{Cursor, push_u16};
use super::frame::{decode_frame, encode_frame};
use super::primitives::{decode_optional_text, encode_optional_text};
use super::types::{IpcCodecError, IpcMessageKind};

pub fn encode_broker_health_frame(packet: &BrokerHealthPacket) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_broker_health_payload(packet, &mut payload)?;
    encode_frame(
        IpcMessageKind::BrokerHealth,
        TransactionId::from_raw(packet.generation),
        &payload,
    )
}

pub fn decode_broker_health_frame(frame: &[u8]) -> Result<BrokerHealthPacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::BrokerHealth {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }
    let mut cursor = Cursor::new(payload);
    let packet = decode_broker_health_payload(header.transaction.raw(), &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}
fn encode_broker_health_payload(
    packet: &BrokerHealthPacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    push_u16(out, encode_broker_kind(packet.broker));
    push_u16(out, encode_broker_health_state(packet.state));
    encode_optional_text(
        out,
        "broker_health_message",
        packet.message.as_deref(),
        SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
    )
}

fn decode_broker_health_payload(
    generation: u64,
    cursor: &mut Cursor<'_>,
) -> Result<BrokerHealthPacket, IpcCodecError> {
    let broker = decode_broker_kind(cursor.u16()?)?;
    let state = decode_broker_health_state(cursor.u16()?)?;
    let message = decode_optional_text(
        cursor,
        "broker_health_message",
        SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
    )?;

    Ok(BrokerHealthPacket {
        broker,
        state,
        generation,
        message,
    })
}
fn encode_broker_kind(kind: BrokerKind) -> u16 {
    match kind {
        BrokerKind::Portal => 1,
        BrokerKind::Metadata => 2,
    }
}

fn decode_broker_kind(value: u16) -> Result<BrokerKind, IpcCodecError> {
    match value {
        1 => Ok(BrokerKind::Portal),
        2 => Ok(BrokerKind::Metadata),
        other => Err(IpcCodecError::InvalidEnum {
            field: "broker_kind",
            value: u32::from(other),
        }),
    }
}

fn encode_broker_health_state(state: BrokerHealthState) -> u16 {
    match state {
        BrokerHealthState::Starting => 1,
        BrokerHealthState::Ready => 2,
        BrokerHealthState::Degraded => 3,
        BrokerHealthState::Stopped => 4,
    }
}

fn decode_broker_health_state(value: u16) -> Result<BrokerHealthState, IpcCodecError> {
    match value {
        1 => Ok(BrokerHealthState::Starting),
        2 => Ok(BrokerHealthState::Ready),
        3 => Ok(BrokerHealthState::Degraded),
        4 => Ok(BrokerHealthState::Stopped),
        other => Err(IpcCodecError::InvalidEnum {
            field: "broker_health_state",
            value: u32::from(other),
        }),
    }
}

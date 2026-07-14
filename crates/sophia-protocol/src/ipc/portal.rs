use crate::{
    NamespaceId, PortalBrokerRequestPacket, PortalBrokerResponseDecision,
    PortalBrokerResponsePacket, PortalDecision, PortalGrant, PortalGrantState, PortalRequest,
    PortalTransfer, PortalTransferId, PortalTransferKind, SOPHIA_PORTAL_MAX_MIME_TYPE_LEN,
    TransactionId,
};

use super::cursor::{Cursor, push_u8, push_u16, push_u64};
use super::frame::{decode_frame, encode_frame};
use super::primitives::{decode_optional_text, encode_optional_text};
use super::{IpcCodecError, IpcMessageKind, SOPHIA_IPC_MAX_PAYLOAD_LEN};

pub fn encode_portal_broker_request_frame(
    packet: &PortalBrokerRequestPacket,
) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_transfer(&packet.request.transfer, &mut payload)?;
    push_u64(&mut payload, packet.request.deadline_msec);
    push_u8(&mut payload, u8::from(packet.source_may_publish));
    push_u8(&mut payload, u8::from(packet.target_may_request));
    encode_frame(
        IpcMessageKind::PortalBrokerRequest,
        TransactionId::from_raw(packet.request.transfer.transfer.raw()),
        &payload,
    )
}

pub fn decode_portal_broker_request_frame(
    frame: &[u8],
) -> Result<PortalBrokerRequestPacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_kind(header.message_kind, IpcMessageKind::PortalBrokerRequest)?;
    let mut cursor = Cursor::new(payload);
    let transfer = decode_transfer(&mut cursor)?;
    if transfer.transfer.raw() != header.transaction.raw() {
        return Err(IpcCodecError::InvalidEnum {
            field: "portal_transfer_correlation",
            value: 0,
        });
    }
    let packet = PortalBrokerRequestPacket {
        request: PortalRequest {
            transfer,
            deadline_msec: cursor.u64()?,
        },
        source_may_publish: decode_bool(&mut cursor, "source_may_publish")?,
        target_may_request: decode_bool(&mut cursor, "target_may_request")?,
    };
    cursor.finish()?;
    Ok(packet)
}

pub fn encode_portal_broker_response_frame(
    packet: &PortalBrokerResponsePacket,
) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    match &packet.decision {
        PortalBrokerResponseDecision::Denied => push_u8(&mut payload, 1),
        PortalBrokerResponseDecision::Allowed(grant) => {
            push_u8(&mut payload, 2);
            encode_grant(grant, &mut payload);
        }
    }
    encode_frame(
        IpcMessageKind::PortalBrokerResponse,
        TransactionId::from_raw(packet.transfer.raw()),
        &payload,
    )
}

pub fn decode_portal_broker_response_frame(
    frame: &[u8],
) -> Result<PortalBrokerResponsePacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_kind(header.message_kind, IpcMessageKind::PortalBrokerResponse)?;
    let transfer = PortalTransferId::from_raw(header.transaction.raw());
    let mut cursor = Cursor::new(payload);
    let decision = match cursor.u8()? {
        1 => PortalBrokerResponseDecision::Denied,
        2 => {
            let grant = decode_grant(&mut cursor)?;
            if grant.transfer != transfer {
                return Err(IpcCodecError::InvalidEnum {
                    field: "portal_grant_correlation",
                    value: 0,
                });
            }
            PortalBrokerResponseDecision::Allowed(grant)
        }
        value => {
            return Err(IpcCodecError::InvalidEnum {
                field: "portal_broker_decision",
                value: u32::from(value),
            });
        }
    };
    cursor.finish()?;
    Ok(PortalBrokerResponsePacket { transfer, decision })
}

pub fn encode_portal_clipboard_payload_frame(
    transfer: PortalTransferId,
    payload: &[u8],
) -> Result<Vec<u8>, IpcCodecError> {
    encode_frame(
        IpcMessageKind::PortalClipboardPayload,
        TransactionId::from_raw(transfer.raw()),
        payload,
    )
}

pub fn decode_portal_clipboard_payload_frame(
    frame: &[u8],
) -> Result<(PortalTransferId, Vec<u8>), IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    expect_kind(header.message_kind, IpcMessageKind::PortalClipboardPayload)?;
    if payload.len() > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(IpcCodecError::PayloadTooLarge(payload.len()));
    }
    Ok((
        PortalTransferId::from_raw(header.transaction.raw()),
        payload.to_vec(),
    ))
}

fn encode_transfer(value: &PortalTransfer, out: &mut Vec<u8>) -> Result<(), IpcCodecError> {
    push_u64(out, value.transfer.raw());
    push_u64(out, value.source_namespace.raw());
    push_u64(out, value.target_namespace.raw());
    push_u16(out, encode_kind(value.kind));
    encode_optional_text(
        out,
        "portal_mime_type",
        value.mime_type.as_deref(),
        SOPHIA_PORTAL_MAX_MIME_TYPE_LEN,
    )?;
    push_u64(out, value.byte_size);
    push_u8(out, encode_decision(value.decision));
    push_u64(out, value.generation);
    Ok(())
}

fn decode_transfer(cursor: &mut Cursor<'_>) -> Result<PortalTransfer, IpcCodecError> {
    Ok(PortalTransfer {
        transfer: PortalTransferId::from_raw(cursor.u64()?),
        source_namespace: NamespaceId::from_raw(cursor.u64()?),
        target_namespace: NamespaceId::from_raw(cursor.u64()?),
        kind: decode_kind(cursor.u16()?)?,
        mime_type: decode_optional_text(
            cursor,
            "portal_mime_type",
            SOPHIA_PORTAL_MAX_MIME_TYPE_LEN,
        )?,
        byte_size: cursor.u64()?,
        decision: decode_decision(cursor.u8()?)?,
        generation: cursor.u64()?,
    })
}

fn encode_grant(value: &PortalGrant, out: &mut Vec<u8>) {
    push_u64(out, value.transfer.raw());
    push_u64(out, value.source_namespace.raw());
    push_u64(out, value.target_namespace.raw());
    push_u16(out, encode_kind(value.kind));
    push_u64(out, value.source_generation);
    push_u64(out, value.broker_generation);
    push_u64(out, value.deadline_msec);
    push_u8(
        out,
        match value.state {
            PortalGrantState::Active => 1,
            PortalGrantState::Completed => 2,
            PortalGrantState::Revoked => 3,
            PortalGrantState::Expired => 4,
        },
    );
}

fn decode_grant(cursor: &mut Cursor<'_>) -> Result<PortalGrant, IpcCodecError> {
    Ok(PortalGrant {
        transfer: PortalTransferId::from_raw(cursor.u64()?),
        source_namespace: NamespaceId::from_raw(cursor.u64()?),
        target_namespace: NamespaceId::from_raw(cursor.u64()?),
        kind: decode_kind(cursor.u16()?)?,
        source_generation: cursor.u64()?,
        broker_generation: cursor.u64()?,
        deadline_msec: cursor.u64()?,
        state: match cursor.u8()? {
            1 => PortalGrantState::Active,
            2 => PortalGrantState::Completed,
            3 => PortalGrantState::Revoked,
            4 => PortalGrantState::Expired,
            value => {
                return Err(IpcCodecError::InvalidEnum {
                    field: "portal_grant_state",
                    value: u32::from(value),
                });
            }
        },
    })
}

fn encode_kind(value: PortalTransferKind) -> u16 {
    match value {
        PortalTransferKind::Clipboard => 1,
        PortalTransferKind::DragAndDrop => 2,
        PortalTransferKind::FileHandoff => 3,
        PortalTransferKind::ScreenCapture => 4,
        PortalTransferKind::ScreenRecording => 5,
        PortalTransferKind::UriOpen => 6,
        PortalTransferKind::Notification => 7,
    }
}
fn decode_kind(value: u16) -> Result<PortalTransferKind, IpcCodecError> {
    match value {
        1 => Ok(PortalTransferKind::Clipboard),
        2 => Ok(PortalTransferKind::DragAndDrop),
        3 => Ok(PortalTransferKind::FileHandoff),
        4 => Ok(PortalTransferKind::ScreenCapture),
        5 => Ok(PortalTransferKind::ScreenRecording),
        6 => Ok(PortalTransferKind::UriOpen),
        7 => Ok(PortalTransferKind::Notification),
        value => Err(IpcCodecError::InvalidEnum {
            field: "portal_transfer_kind",
            value: u32::from(value),
        }),
    }
}
fn encode_decision(value: PortalDecision) -> u8 {
    match value {
        PortalDecision::Pending => 1,
        PortalDecision::Allowed => 2,
        PortalDecision::Denied => 3,
        PortalDecision::Revoked => 4,
    }
}
fn decode_decision(value: u8) -> Result<PortalDecision, IpcCodecError> {
    match value {
        1 => Ok(PortalDecision::Pending),
        2 => Ok(PortalDecision::Allowed),
        3 => Ok(PortalDecision::Denied),
        4 => Ok(PortalDecision::Revoked),
        value => Err(IpcCodecError::InvalidEnum {
            field: "portal_decision",
            value: u32::from(value),
        }),
    }
}
fn decode_bool(cursor: &mut Cursor<'_>, field: &'static str) -> Result<bool, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(IpcCodecError::InvalidBool { field, value }),
    }
}
fn expect_kind(actual: IpcMessageKind, expected: IpcMessageKind) -> Result<(), IpcCodecError> {
    if actual == expected {
        Ok(())
    } else {
        Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: actual as u32,
        })
    }
}

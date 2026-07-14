use sophia_protocol::{
    AuthoritySurface, IpcCodecError, IpcMessageKind, PortalTransfer, SurfaceTransaction,
    TransactionId, decode_frame, encode_frame,
};

use crate::{
    ClipboardSelectionFailure, ClipboardSelectionNotify, X_AUTHORITY_MAX_TARGET_NAME_LEN,
    XAuthorityPortalCommand, XAuthorityRequestKind, XAuthorityRequestPacket,
    XAuthorityResponsePacket, XAuthoritySelectionArtifact,
};

const X_AUTHORITY_MAX_TEXT_LEN: usize = 256;

mod support;

use support::*;

pub fn encode_x_authority_request_frame(
    request: &XAuthorityRequestPacket,
) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_request_payload(request, &mut payload)?;
    encode_frame(
        IpcMessageKind::XAuthorityRequest,
        request.transaction,
        &payload,
    )
}

pub fn decode_x_authority_request_frame(
    frame: &[u8],
) -> Result<XAuthorityRequestPacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::XAuthorityRequest {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }

    let mut cursor = Cursor::new(payload);
    let packet = decode_request_payload(header.transaction, &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}

pub fn encode_x_authority_response_frame(
    response: &XAuthorityResponsePacket,
) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_response_payload(response, &mut payload)?;
    encode_frame(
        IpcMessageKind::XAuthorityResponse,
        response.transaction,
        &payload,
    )
}

pub fn decode_x_authority_response_frame(
    frame: &[u8],
) -> Result<XAuthorityResponsePacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::XAuthorityResponse {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }

    let mut cursor = Cursor::new(payload);
    let packet = decode_response_payload(header.transaction, &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}

fn encode_request_payload(
    request: &XAuthorityRequestPacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    encode_namespace_id(request.namespace, out);
    match &request.kind {
        XAuthorityRequestKind::CreateWindow {
            window,
            surface,
            geometry,
            constraints,
            generation,
        } => {
            push_u16(out, 1);
            encode_x_resource_id(*window, out);
            encode_surface_id(*surface, out);
            encode_rect(*geometry, out);
            encode_constraints(*constraints, out);
            push_u64(out, *generation);
        }
        XAuthorityRequestKind::MapWindow { window, generation } => {
            push_u16(out, 2);
            encode_x_resource_id(*window, out);
            push_u64(out, *generation);
        }
        XAuthorityRequestKind::PresentPixmap {
            window,
            pixmap,
            damage,
            previous_committed_generation,
            timeout_msec,
        } => {
            push_u16(out, 3);
            encode_x_resource_id(*window, out);
            push_u32(out, *pixmap);
            encode_region(damage, out)?;
            push_u64(out, *previous_committed_generation);
            push_u32(out, *timeout_msec);
        }
        XAuthorityRequestKind::SetSelectionOwner {
            selection,
            owner,
            timestamp,
            selection_timestamp,
            kind,
        } => {
            push_u16(out, 4);
            push_u32(out, *selection);
            encode_optional_x_resource_id(*owner, out);
            push_u32(out, *timestamp);
            push_u32(out, *selection_timestamp);
            push_u16(out, encode_selection_change_kind(*kind));
        }
        XAuthorityRequestKind::RequestSelection {
            requestor,
            selection,
            target,
            target_name,
            property,
            time,
            transfer,
        } => {
            push_u16(out, 5);
            encode_x_resource_id(*requestor, out);
            push_u32(out, *selection);
            push_u32(out, *target);
            encode_text(
                out,
                "x_authority_target_name",
                target_name,
                X_AUTHORITY_MAX_TARGET_NAME_LEN,
            )?;
            push_u32(out, *property);
            push_u32(out, *time);
            encode_portal_transfer_id(*transfer, out);
        }
    }
    Ok(())
}

fn decode_request_payload(
    transaction: TransactionId,
    cursor: &mut Cursor<'_>,
) -> Result<XAuthorityRequestPacket, IpcCodecError> {
    let namespace = decode_namespace_id(cursor)?;
    let kind = match cursor.u16()? {
        1 => XAuthorityRequestKind::CreateWindow {
            window: decode_x_resource_id(cursor)?,
            surface: decode_surface_id(cursor)?,
            geometry: decode_rect(cursor)?,
            constraints: decode_constraints(cursor)?,
            generation: cursor.u64()?,
        },
        2 => XAuthorityRequestKind::MapWindow {
            window: decode_x_resource_id(cursor)?,
            generation: cursor.u64()?,
        },
        3 => XAuthorityRequestKind::PresentPixmap {
            window: decode_x_resource_id(cursor)?,
            pixmap: cursor.u32()?,
            damage: decode_region(cursor)?,
            previous_committed_generation: cursor.u64()?,
            timeout_msec: cursor.u32()?,
        },
        4 => XAuthorityRequestKind::SetSelectionOwner {
            selection: cursor.u32()?,
            owner: decode_optional_x_resource_id(cursor)?,
            timestamp: cursor.u32()?,
            selection_timestamp: cursor.u32()?,
            kind: decode_selection_change_kind(cursor.u16()?)?,
        },
        5 => XAuthorityRequestKind::RequestSelection {
            requestor: decode_x_resource_id(cursor)?,
            selection: cursor.u32()?,
            target: cursor.u32()?,
            target_name: decode_text(
                cursor,
                "x_authority_target_name",
                X_AUTHORITY_MAX_TARGET_NAME_LEN,
            )?,
            property: cursor.u32()?,
            time: cursor.u32()?,
            transfer: decode_portal_transfer_id(cursor)?,
        },
        other => {
            return Err(IpcCodecError::InvalidEnum {
                field: "x_authority_request_kind",
                value: u32::from(other),
            });
        }
    };

    Ok(XAuthorityRequestPacket {
        transaction,
        namespace,
        kind,
    })
}

fn encode_response_payload(
    response: &XAuthorityResponsePacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    encode_response_outcome(response.outcome, out);
    encode_count(response.surfaces.len(), out)?;
    for surface in &response.surfaces {
        encode_authority_surface(surface, out);
    }
    encode_count(response.removed_surfaces.len(), out)?;
    for surface in &response.removed_surfaces {
        encode_surface_id(*surface, out);
    }
    encode_count(response.transactions.len(), out)?;
    for transaction in &response.transactions {
        encode_surface_transaction(transaction, out)?;
    }
    encode_count(response.portal_commands.len(), out)?;
    for command in &response.portal_commands {
        encode_portal_command(command, out)?;
    }
    encode_count(response.selection_artifacts.len(), out)?;
    for artifact in &response.selection_artifacts {
        encode_selection_artifact(artifact, out);
    }
    Ok(())
}

fn decode_response_payload(
    transaction: TransactionId,
    cursor: &mut Cursor<'_>,
) -> Result<XAuthorityResponsePacket, IpcCodecError> {
    let outcome = decode_response_outcome(cursor)?;
    let surface_count = decode_count(cursor)?;
    let mut surfaces = Vec::with_capacity(surface_count);
    for _ in 0..surface_count {
        surfaces.push(decode_authority_surface(cursor)?);
    }
    let removal_count = decode_count(cursor)?;
    let mut removed_surfaces = Vec::with_capacity(removal_count);
    for _ in 0..removal_count {
        removed_surfaces.push(decode_surface_id(cursor)?);
    }
    let transaction_count = decode_count(cursor)?;
    let mut transactions = Vec::with_capacity(transaction_count);
    for _ in 0..transaction_count {
        transactions.push(decode_surface_transaction(cursor)?);
    }
    let command_count = decode_count(cursor)?;
    let mut portal_commands = Vec::with_capacity(command_count);
    for _ in 0..command_count {
        portal_commands.push(decode_portal_command(cursor)?);
    }
    let artifact_count = decode_count(cursor)?;
    let mut selection_artifacts = Vec::with_capacity(artifact_count);
    for _ in 0..artifact_count {
        selection_artifacts.push(decode_selection_artifact(cursor)?);
    }

    Ok(XAuthorityResponsePacket {
        transaction,
        outcome,
        surfaces,
        removed_surfaces,
        transactions,
        portal_commands,
        selection_artifacts,
    })
}

fn encode_authority_surface(surface: &AuthoritySurface, out: &mut Vec<u8>) {
    encode_authority_kind(surface.authority, out);
    encode_authority_local_id(surface.local_id, out);
    encode_surface_id(surface.surface, out);
    encode_optional_namespace_id(surface.namespace, out);
    encode_bool(surface.mapped, out);
    encode_rect(surface.geometry, out);
    encode_constraints(surface.constraints, out);
    push_u64(out, surface.generation);
}

fn decode_authority_surface(cursor: &mut Cursor<'_>) -> Result<AuthoritySurface, IpcCodecError> {
    Ok(AuthoritySurface {
        authority: decode_authority_kind(cursor.u16()?)?,
        local_id: decode_authority_local_id(cursor)?,
        surface: decode_surface_id(cursor)?,
        namespace: decode_optional_namespace_id(cursor)?,
        mapped: decode_bool(cursor)?,
        geometry: decode_rect(cursor)?,
        constraints: decode_constraints(cursor)?,
        generation: cursor.u64()?,
    })
}

fn encode_surface_transaction(
    transaction: &SurfaceTransaction,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    encode_transaction_id(transaction.transaction, out);
    encode_authority_kind(transaction.authority, out);
    encode_surface_id(transaction.surface, out);
    encode_optional_namespace_id(transaction.namespace, out);
    encode_rect(transaction.target_geometry, out);
    encode_buffer_source(transaction.target_buffer, out);
    encode_region(&transaction.damage, out)?;
    encode_readiness(transaction.readiness, out);
    push_u32(out, transaction.timeout_msec);
    push_u64(out, transaction.previous_committed_generation);
    Ok(())
}

fn decode_surface_transaction(
    cursor: &mut Cursor<'_>,
) -> Result<SurfaceTransaction, IpcCodecError> {
    Ok(SurfaceTransaction {
        transaction: decode_transaction_id(cursor)?,
        authority: decode_authority_kind(cursor.u16()?)?,
        surface: decode_surface_id(cursor)?,
        namespace: decode_optional_namespace_id(cursor)?,
        target_geometry: decode_rect(cursor)?,
        target_buffer: decode_buffer_source(cursor)?,
        damage: decode_region(cursor)?,
        readiness: decode_readiness(cursor.u16()?)?,
        timeout_msec: cursor.u32()?,
        previous_committed_generation: cursor.u64()?,
    })
}

fn encode_portal_command(
    command: &XAuthorityPortalCommand,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    match command {
        XAuthorityPortalCommand::PromptClipboardTransfer(transfer) => {
            push_u16(out, 1);
            encode_portal_transfer(transfer, out)?;
        }
        XAuthorityPortalCommand::FailSelection { transfer } => {
            push_u16(out, 2);
            encode_portal_transfer_id(*transfer, out);
        }
        XAuthorityPortalCommand::HandoffClipboard { transfer } => {
            push_u16(out, 3);
            encode_portal_transfer_id(*transfer, out);
        }
    }
    Ok(())
}

fn decode_portal_command(
    cursor: &mut Cursor<'_>,
) -> Result<XAuthorityPortalCommand, IpcCodecError> {
    match cursor.u16()? {
        1 => Ok(XAuthorityPortalCommand::PromptClipboardTransfer(
            decode_portal_transfer(cursor)?,
        )),
        2 => Ok(XAuthorityPortalCommand::FailSelection {
            transfer: decode_portal_transfer_id(cursor)?,
        }),
        3 => Ok(XAuthorityPortalCommand::HandoffClipboard {
            transfer: decode_portal_transfer_id(cursor)?,
        }),
        other => Err(IpcCodecError::InvalidEnum {
            field: "x_authority_portal_command",
            value: u32::from(other),
        }),
    }
}

fn encode_selection_artifact(artifact: &XAuthoritySelectionArtifact, out: &mut Vec<u8>) {
    match artifact {
        XAuthoritySelectionArtifact::Failure(failure) => {
            push_u16(out, 1);
            encode_selection_failure(failure, out);
        }
        XAuthoritySelectionArtifact::Request(request) => {
            push_u16(out, 2);
            encode_x_resource_id(request.owner, out);
            encode_x_resource_id(request.requestor, out);
            push_u32(out, request.selection);
            push_u32(out, request.target);
            push_u32(out, request.property);
            push_u32(out, request.time);
        }
        XAuthoritySelectionArtifact::Clear {
            owner,
            selection,
            time,
        } => {
            push_u16(out, 3);
            encode_x_resource_id(*owner, out);
            push_u32(out, *selection);
            push_u32(out, *time);
        }
    }
}

fn decode_selection_artifact(
    cursor: &mut Cursor<'_>,
) -> Result<XAuthoritySelectionArtifact, IpcCodecError> {
    match cursor.u16()? {
        1 => Ok(XAuthoritySelectionArtifact::Failure(
            decode_selection_failure(cursor)?,
        )),
        2 => Ok(XAuthoritySelectionArtifact::Request(
            crate::ClipboardSelectionOwnerRequest {
                owner: decode_x_resource_id(cursor)?,
                requestor: decode_x_resource_id(cursor)?,
                selection: cursor.u32()?,
                target: cursor.u32()?,
                property: cursor.u32()?,
                time: cursor.u32()?,
            },
        )),
        3 => Ok(XAuthoritySelectionArtifact::Clear {
            owner: decode_x_resource_id(cursor)?,
            selection: cursor.u32()?,
            time: cursor.u32()?,
        }),
        other => Err(IpcCodecError::InvalidEnum {
            field: "x_authority_selection_artifact",
            value: u32::from(other),
        }),
    }
}

fn encode_selection_failure(failure: &ClipboardSelectionFailure, out: &mut Vec<u8>) {
    encode_portal_transfer_id(failure.transfer, out);
    encode_selection_notify(failure.notify, out);
}

fn decode_selection_failure(
    cursor: &mut Cursor<'_>,
) -> Result<ClipboardSelectionFailure, IpcCodecError> {
    Ok(ClipboardSelectionFailure {
        transfer: decode_portal_transfer_id(cursor)?,
        notify: decode_selection_notify(cursor)?,
    })
}

fn encode_selection_notify(notify: ClipboardSelectionNotify, out: &mut Vec<u8>) {
    push_u32(out, notify.time);
    encode_x_resource_id(notify.requestor, out);
    push_u32(out, notify.selection);
    push_u32(out, notify.target);
    push_u32(out, notify.property);
}

fn decode_selection_notify(
    cursor: &mut Cursor<'_>,
) -> Result<ClipboardSelectionNotify, IpcCodecError> {
    Ok(ClipboardSelectionNotify {
        time: cursor.u32()?,
        requestor: decode_x_resource_id(cursor)?,
        selection: cursor.u32()?,
        target: cursor.u32()?,
        property: cursor.u32()?,
    })
}

fn encode_portal_transfer(
    transfer: &PortalTransfer,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    encode_portal_transfer_id(transfer.transfer, out);
    encode_namespace_id(transfer.source_namespace, out);
    encode_namespace_id(transfer.target_namespace, out);
    encode_portal_transfer_kind(transfer.kind, out);
    encode_optional_text(
        out,
        "x_authority_portal_mime",
        transfer.mime_type.as_deref(),
        X_AUTHORITY_MAX_TEXT_LEN,
    )?;
    push_u64(out, transfer.byte_size);
    encode_portal_decision(transfer.decision, out);
    push_u64(out, transfer.generation);
    Ok(())
}

fn decode_portal_transfer(cursor: &mut Cursor<'_>) -> Result<PortalTransfer, IpcCodecError> {
    Ok(PortalTransfer {
        transfer: decode_portal_transfer_id(cursor)?,
        source_namespace: decode_namespace_id(cursor)?,
        target_namespace: decode_namespace_id(cursor)?,
        kind: decode_portal_transfer_kind(cursor.u16()?)?,
        mime_type: decode_optional_text(
            cursor,
            "x_authority_portal_mime",
            X_AUTHORITY_MAX_TEXT_LEN,
        )?,
        byte_size: cursor.u64()?,
        decision: decode_portal_decision(cursor.u16()?)?,
        generation: cursor.u64()?,
    })
}

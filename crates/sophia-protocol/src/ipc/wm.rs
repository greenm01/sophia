use crate::{
    SurfacePlacement, SurfaceSizeRequest, TransactionId, WmCommand, WmManageSurface,
    WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WmResponsePacket,
};

use super::cursor::{Cursor, push_i32, push_u16, push_u32};
use super::frame::{decode_frame, encode_frame};
use super::primitives::{
    check_count, decode_count, decode_layout_node, decode_option_rect, decode_output_id,
    decode_rect, decode_size, decode_surface_id, decode_transform, decode_workspace_id,
    encode_layout_node, encode_option_rect, encode_output_id, encode_rect, encode_size,
    encode_surface_id, encode_transform, encode_workspace_id,
};
use super::types::{IpcCodecError, IpcMessageKind};

pub fn encode_wm_request_frame(request: &WmRequestPacket) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_wm_request_payload(request, &mut payload)?;
    encode_frame(IpcMessageKind::WmRequest, request.transaction, &payload)
}

pub fn decode_wm_request_frame(frame: &[u8]) -> Result<WmRequestPacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::WmRequest {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }
    let mut cursor = Cursor::new(payload);
    let packet = decode_wm_request_payload(header.transaction, &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}

pub fn encode_wm_response_frame(response: &WmResponsePacket) -> Result<Vec<u8>, IpcCodecError> {
    let mut payload = Vec::new();
    encode_wm_response_payload(response, &mut payload)?;
    encode_frame(IpcMessageKind::WmResponse, response.transaction, &payload)
}

pub fn decode_wm_response_frame(frame: &[u8]) -> Result<WmResponsePacket, IpcCodecError> {
    let (header, payload) = decode_frame(frame)?;
    if header.message_kind != IpcMessageKind::WmResponse {
        return Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: header.message_kind as u32,
        });
    }
    let mut cursor = Cursor::new(payload);
    let packet = decode_wm_response_payload(header.transaction, &mut cursor)?;
    cursor.finish()?;
    Ok(packet)
}
fn encode_wm_request_payload(
    request: &WmRequestPacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    match &request.kind {
        WmRequestKind::ManageSurface(manage) => {
            push_u16(out, 1);
            encode_output_id(manage.output, out);
            encode_workspace_id(manage.workspace, out);
            encode_rect(manage.bounds, out);
            encode_layout_node(&manage.node, out);
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            check_count(relayout.nodes.len())?;
            push_u16(out, 2);
            encode_output_id(relayout.output, out);
            encode_workspace_id(relayout.workspace, out);
            encode_rect(relayout.bounds, out);
            push_u32(out, relayout.nodes.len() as u32);
            for node in &relayout.nodes {
                encode_layout_node(node, out);
            }
        }
        WmRequestKind::SurfaceRemoved { surface, workspace } => {
            push_u16(out, 3);
            encode_surface_id(*surface, out);
            encode_workspace_id(*workspace, out);
        }
    }
    Ok(())
}

fn decode_wm_request_payload(
    transaction: TransactionId,
    cursor: &mut Cursor<'_>,
) -> Result<WmRequestPacket, IpcCodecError> {
    let kind = match cursor.u16()? {
        1 => WmRequestKind::ManageSurface(WmManageSurface {
            output: decode_output_id(cursor)?,
            workspace: decode_workspace_id(cursor)?,
            bounds: decode_rect(cursor)?,
            node: decode_layout_node(cursor)?,
        }),
        2 => {
            let output = decode_output_id(cursor)?;
            let workspace = decode_workspace_id(cursor)?;
            let bounds = decode_rect(cursor)?;
            let count = decode_count(cursor)?;
            let mut nodes = Vec::with_capacity(count);
            for _ in 0..count {
                nodes.push(decode_layout_node(cursor)?);
            }
            WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output,
                workspace,
                bounds,
                nodes,
            })
        }
        3 => WmRequestKind::SurfaceRemoved {
            surface: decode_surface_id(cursor)?,
            workspace: decode_workspace_id(cursor)?,
        },
        other => {
            return Err(IpcCodecError::InvalidEnum {
                field: "wm_request_kind",
                value: u32::from(other),
            });
        }
    };

    Ok(WmRequestPacket { transaction, kind })
}

fn encode_wm_response_payload(
    response: &WmResponsePacket,
    out: &mut Vec<u8>,
) -> Result<(), IpcCodecError> {
    check_count(response.commands.len())?;
    push_u32(out, response.timeout_msec);
    push_u32(out, response.commands.len() as u32);
    for command in &response.commands {
        match command {
            WmCommand::ConfigureSurface(request) => {
                push_u16(out, 1);
                encode_surface_id(request.surface, out);
                encode_size(request.size, out);
            }
            WmCommand::FocusSurface(surface) => {
                push_u16(out, 2);
                encode_surface_id(*surface, out);
            }
            WmCommand::AssignWorkspace { surface, workspace } => {
                push_u16(out, 3);
                encode_surface_id(*surface, out);
                encode_workspace_id(*workspace, out);
            }
            WmCommand::RenderSurface(placement) => {
                push_u16(out, 4);
                encode_surface_id(placement.surface, out);
                encode_rect(placement.geometry, out);
                push_i32(out, placement.z_index);
                encode_option_rect(placement.crop, out);
                encode_transform(placement.transform, out);
            }
        }
    }
    Ok(())
}

fn decode_wm_response_payload(
    transaction: TransactionId,
    cursor: &mut Cursor<'_>,
) -> Result<WmResponsePacket, IpcCodecError> {
    let timeout_msec = cursor.u32()?;
    let count = decode_count(cursor)?;
    let mut commands = Vec::with_capacity(count);
    for _ in 0..count {
        let command = match cursor.u16()? {
            1 => WmCommand::ConfigureSurface(SurfaceSizeRequest {
                surface: decode_surface_id(cursor)?,
                size: decode_size(cursor)?,
            }),
            2 => WmCommand::FocusSurface(decode_surface_id(cursor)?),
            3 => WmCommand::AssignWorkspace {
                surface: decode_surface_id(cursor)?,
                workspace: decode_workspace_id(cursor)?,
            },
            4 => WmCommand::RenderSurface(SurfacePlacement {
                surface: decode_surface_id(cursor)?,
                geometry: decode_rect(cursor)?,
                z_index: cursor.i32()?,
                crop: decode_option_rect(cursor)?,
                transform: decode_transform(cursor)?,
            }),
            other => {
                return Err(IpcCodecError::InvalidEnum {
                    field: "wm_command",
                    value: u32::from(other),
                });
            }
        };
        commands.push(command);
    }

    Ok(WmResponsePacket {
        transaction,
        commands,
        timeout_msec,
    })
}

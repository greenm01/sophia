use crate::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, OutputId, Rect,
    Size, SurfaceConstraints, SurfaceId, Transform, WorkspaceId,
};

use super::cursor::{Cursor, push_i32, push_u8, push_u16, push_u32, push_u64};
use super::types::{IpcCodecError, SOPHIA_IPC_MAX_ITEMS};

pub(crate) fn encode_layout_node(node: &LayoutNodeSnapshot, out: &mut Vec<u8>) {
    encode_surface_id(node.surface, out);
    encode_workspace_id(node.workspace, out);
    push_u16(out, encode_layout_node_kind(node.kind));
    push_u16(out, encode_capabilities(node.capabilities));
    push_u16(out, encode_node_state(node.state));
    encode_constraints(node.constraints, out);
    encode_rect(node.geometry, out);
    push_u64(out, node.generation);
}

pub(crate) fn decode_layout_node(
    cursor: &mut Cursor<'_>,
) -> Result<LayoutNodeSnapshot, IpcCodecError> {
    Ok(LayoutNodeSnapshot {
        surface: decode_surface_id(cursor)?,
        workspace: decode_workspace_id(cursor)?,
        kind: decode_layout_node_kind(cursor.u16()?)?,
        capabilities: decode_capabilities(cursor.u16()?),
        state: decode_node_state(cursor.u16()?),
        constraints: decode_constraints(cursor)?,
        geometry: decode_rect(cursor)?,
        generation: cursor.u64()?,
    })
}

pub(crate) fn encode_constraints(constraints: SurfaceConstraints, out: &mut Vec<u8>) {
    encode_option_size(constraints.min_size, out);
    encode_option_size(constraints.max_size, out);
}

pub(crate) fn decode_constraints(
    cursor: &mut Cursor<'_>,
) -> Result<SurfaceConstraints, IpcCodecError> {
    Ok(SurfaceConstraints {
        min_size: decode_option_size(cursor)?,
        max_size: decode_option_size(cursor)?,
    })
}

pub(crate) fn encode_surface_id(id: SurfaceId, out: &mut Vec<u8>) {
    push_u32(out, id.index());
    push_u32(out, id.generation());
}

pub(crate) fn decode_surface_id(cursor: &mut Cursor<'_>) -> Result<SurfaceId, IpcCodecError> {
    Ok(SurfaceId::new(cursor.u32()?, cursor.u32()?))
}

pub(crate) fn encode_workspace_id(id: WorkspaceId, out: &mut Vec<u8>) {
    push_u64(out, id.raw());
}

pub(crate) fn decode_workspace_id(cursor: &mut Cursor<'_>) -> Result<WorkspaceId, IpcCodecError> {
    Ok(WorkspaceId::from_raw(cursor.u64()?))
}

pub(crate) fn encode_output_id(id: OutputId, out: &mut Vec<u8>) {
    push_u64(out, id.raw());
}

pub(crate) fn decode_output_id(cursor: &mut Cursor<'_>) -> Result<OutputId, IpcCodecError> {
    Ok(OutputId::from_raw(cursor.u64()?))
}

pub(crate) fn encode_rect(rect: Rect, out: &mut Vec<u8>) {
    push_i32(out, rect.x);
    push_i32(out, rect.y);
    push_i32(out, rect.width);
    push_i32(out, rect.height);
}

pub(crate) fn decode_rect(cursor: &mut Cursor<'_>) -> Result<Rect, IpcCodecError> {
    Ok(Rect {
        x: cursor.i32()?,
        y: cursor.i32()?,
        width: cursor.i32()?,
        height: cursor.i32()?,
    })
}

pub(crate) fn encode_size(size: Size, out: &mut Vec<u8>) {
    push_i32(out, size.width);
    push_i32(out, size.height);
}

pub(crate) fn decode_size(cursor: &mut Cursor<'_>) -> Result<Size, IpcCodecError> {
    Ok(Size {
        width: cursor.i32()?,
        height: cursor.i32()?,
    })
}

pub(crate) fn encode_option_size(size: Option<Size>, out: &mut Vec<u8>) {
    match size {
        Some(size) => {
            push_u8(out, 1);
            encode_size(size, out);
        }
        None => push_u8(out, 0),
    }
}

pub(crate) fn decode_option_size(cursor: &mut Cursor<'_>) -> Result<Option<Size>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_size(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "option_size",
            value: other,
        }),
    }
}

pub(crate) fn encode_option_rect(rect: Option<Rect>, out: &mut Vec<u8>) {
    match rect {
        Some(rect) => {
            push_u8(out, 1);
            encode_rect(rect, out);
        }
        None => push_u8(out, 0),
    }
}

pub(crate) fn decode_option_rect(cursor: &mut Cursor<'_>) -> Result<Option<Rect>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_rect(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "option_rect",
            value: other,
        }),
    }
}

pub(crate) fn encode_transform(transform: Transform, out: &mut Vec<u8>) {
    for value in transform.matrix {
        push_u32(out, value.to_bits());
    }
}

pub(crate) fn decode_transform(cursor: &mut Cursor<'_>) -> Result<Transform, IpcCodecError> {
    let mut matrix = [0.0; 9];
    for value in &mut matrix {
        *value = f32::from_bits(cursor.u32()?);
    }
    Ok(Transform { matrix })
}

pub(crate) fn encode_layout_node_kind(kind: LayoutNodeKind) -> u16 {
    match kind {
        LayoutNodeKind::Toplevel => 1,
        LayoutNodeKind::Dialog => 2,
        LayoutNodeKind::Utility => 3,
        LayoutNodeKind::Unknown => 4,
    }
}

pub(crate) fn decode_layout_node_kind(value: u16) -> Result<LayoutNodeKind, IpcCodecError> {
    match value {
        1 => Ok(LayoutNodeKind::Toplevel),
        2 => Ok(LayoutNodeKind::Dialog),
        3 => Ok(LayoutNodeKind::Utility),
        4 => Ok(LayoutNodeKind::Unknown),
        other => Err(IpcCodecError::InvalidEnum {
            field: "layout_node_kind",
            value: u32::from(other),
        }),
    }
}
pub(crate) fn encode_capabilities(capabilities: LayoutNodeCapabilities) -> u16 {
    u16::from(capabilities.movable)
        | (u16::from(capabilities.resizable) << 1)
        | (u16::from(capabilities.focusable) << 2)
        | (u16::from(capabilities.closable) << 3)
        | (u16::from(capabilities.fullscreenable) << 4)
}

pub(crate) fn decode_capabilities(bits: u16) -> LayoutNodeCapabilities {
    LayoutNodeCapabilities {
        movable: bits & 1 != 0,
        resizable: bits & (1 << 1) != 0,
        focusable: bits & (1 << 2) != 0,
        closable: bits & (1 << 3) != 0,
        fullscreenable: bits & (1 << 4) != 0,
    }
}

pub(crate) fn encode_node_state(state: LayoutNodeState) -> u16 {
    u16::from(state.focused)
        | (u16::from(state.urgent) << 1)
        | (u16::from(state.fullscreen) << 2)
        | (u16::from(state.floating) << 3)
        | (u16::from(state.visible) << 4)
}

pub(crate) fn decode_node_state(bits: u16) -> LayoutNodeState {
    LayoutNodeState {
        focused: bits & 1 != 0,
        urgent: bits & (1 << 1) != 0,
        fullscreen: bits & (1 << 2) != 0,
        floating: bits & (1 << 3) != 0,
        visible: bits & (1 << 4) != 0,
    }
}

pub(crate) fn check_count(count: usize) -> Result<(), IpcCodecError> {
    if count > SOPHIA_IPC_MAX_ITEMS {
        Err(IpcCodecError::CountTooLarge {
            count,
            max: SOPHIA_IPC_MAX_ITEMS,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn decode_count(cursor: &mut Cursor<'_>) -> Result<usize, IpcCodecError> {
    let count = cursor.u32()? as usize;
    check_count(count)?;
    Ok(count)
}

pub(crate) fn encode_optional_text(
    out: &mut Vec<u8>,
    field: &'static str,
    value: Option<&str>,
    max: usize,
) -> Result<(), IpcCodecError> {
    match value {
        Some(value) => {
            if value.len() > max {
                return Err(IpcCodecError::TextTooLarge {
                    field,
                    len: value.len(),
                    max,
                });
            }
            push_u8(out, 1);
            push_u16(out, value.len() as u16);
            out.extend_from_slice(value.as_bytes());
        }
        None => push_u8(out, 0),
    }

    Ok(())
}

pub(crate) fn decode_optional_text(
    cursor: &mut Cursor<'_>,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => {
            let len = cursor.u16()? as usize;
            if len > max {
                return Err(IpcCodecError::TextTooLarge { field, len, max });
            }
            let bytes = cursor.slice(len)?;
            let text = core::str::from_utf8(bytes)
                .map_err(|_| IpcCodecError::InvalidUtf8 { field })?
                .to_owned();
            Ok(Some(text))
        }
        other => Err(IpcCodecError::InvalidBool {
            field,
            value: other,
        }),
    }
}

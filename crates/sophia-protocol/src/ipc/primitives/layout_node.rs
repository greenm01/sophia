use super::*;

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

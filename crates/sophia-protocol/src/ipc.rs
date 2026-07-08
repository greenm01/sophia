use crate::OutputId;
use crate::{
    BrokerHealthPacket, BrokerHealthState, BrokerKind, LayoutNodeCapabilities, LayoutNodeKind,
    LayoutNodeSnapshot, LayoutNodeState, Rect, SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN, Size,
    SurfaceConstraints, SurfaceId, SurfacePlacement, SurfaceSizeRequest, TransactionId, Transform,
    WmCommand, WmManageSurface, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket,
    WmResponsePacket, WorkspaceId,
};

pub const SOPHIA_IPC_MAGIC: u32 = 0x4850_4f53;
pub const SOPHIA_IPC_VERSION: u16 = 1;
pub const SOPHIA_IPC_HEADER_LEN: usize = 24;
pub const SOPHIA_IPC_MAX_PAYLOAD_LEN: usize = 64 * 1024;
pub const SOPHIA_IPC_MAX_ITEMS: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IpcMessageKind {
    WmRequest = 1,
    WmResponse = 2,
    BrokerHealth = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpcFrameHeader {
    pub message_kind: IpcMessageKind,
    pub transaction: TransactionId,
    pub payload_len: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpcCodecError {
    Truncated,
    BadMagic,
    UnsupportedVersion(u16),
    UnknownMessageKind(u16),
    PayloadTooLarge(usize),
    ReservedNonZero(u32),
    TrailingBytes(usize),
    CountTooLarge {
        count: usize,
        max: usize,
    },
    TextTooLarge {
        field: &'static str,
        len: usize,
        max: usize,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    InvalidEnum {
        field: &'static str,
        value: u32,
    },
    InvalidBool {
        field: &'static str,
        value: u8,
    },
}

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

pub fn encode_frame(
    message_kind: IpcMessageKind,
    transaction: TransactionId,
    payload: &[u8],
) -> Result<Vec<u8>, IpcCodecError> {
    if payload.len() > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(IpcCodecError::PayloadTooLarge(payload.len()));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload.len());
    push_u32(&mut frame, SOPHIA_IPC_MAGIC);
    push_u16(&mut frame, SOPHIA_IPC_VERSION);
    push_u16(&mut frame, message_kind as u16);
    push_u64(&mut frame, transaction.raw());
    push_u32(&mut frame, payload.len() as u32);
    push_u32(&mut frame, 0);
    frame.extend_from_slice(payload);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> Result<(IpcFrameHeader, &[u8]), IpcCodecError> {
    if frame.len() < SOPHIA_IPC_HEADER_LEN {
        return Err(IpcCodecError::Truncated);
    }

    let mut cursor = Cursor::new(frame);
    let magic = cursor.u32()?;
    if magic != SOPHIA_IPC_MAGIC {
        return Err(IpcCodecError::BadMagic);
    }

    let version = cursor.u16()?;
    if version != SOPHIA_IPC_VERSION {
        return Err(IpcCodecError::UnsupportedVersion(version));
    }

    let message_kind = match cursor.u16()? {
        1 => IpcMessageKind::WmRequest,
        2 => IpcMessageKind::WmResponse,
        3 => IpcMessageKind::BrokerHealth,
        other => return Err(IpcCodecError::UnknownMessageKind(other)),
    };
    let transaction = TransactionId::from_raw(cursor.u64()?);
    let payload_len = cursor.u32()?;
    let reserved = cursor.u32()?;
    if reserved != 0 {
        return Err(IpcCodecError::ReservedNonZero(reserved));
    }

    let payload_len_usize = payload_len as usize;
    if payload_len_usize > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(IpcCodecError::PayloadTooLarge(payload_len_usize));
    }
    let expected_len = SOPHIA_IPC_HEADER_LEN
        .checked_add(payload_len_usize)
        .ok_or(IpcCodecError::PayloadTooLarge(payload_len_usize))?;
    if frame.len() < expected_len {
        return Err(IpcCodecError::Truncated);
    }
    if frame.len() > expected_len {
        return Err(IpcCodecError::TrailingBytes(frame.len() - expected_len));
    }

    Ok((
        IpcFrameHeader {
            message_kind,
            transaction,
            payload_len,
        },
        &frame[SOPHIA_IPC_HEADER_LEN..expected_len],
    ))
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

fn encode_layout_node(node: &LayoutNodeSnapshot, out: &mut Vec<u8>) {
    encode_surface_id(node.surface, out);
    encode_workspace_id(node.workspace, out);
    push_u16(out, encode_layout_node_kind(node.kind));
    push_u16(out, encode_capabilities(node.capabilities));
    push_u16(out, encode_node_state(node.state));
    encode_constraints(node.constraints, out);
    encode_rect(node.geometry, out);
    push_u64(out, node.generation);
}

fn decode_layout_node(cursor: &mut Cursor<'_>) -> Result<LayoutNodeSnapshot, IpcCodecError> {
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

fn encode_constraints(constraints: SurfaceConstraints, out: &mut Vec<u8>) {
    encode_option_size(constraints.min_size, out);
    encode_option_size(constraints.max_size, out);
}

fn decode_constraints(cursor: &mut Cursor<'_>) -> Result<SurfaceConstraints, IpcCodecError> {
    Ok(SurfaceConstraints {
        min_size: decode_option_size(cursor)?,
        max_size: decode_option_size(cursor)?,
    })
}

fn encode_surface_id(id: SurfaceId, out: &mut Vec<u8>) {
    push_u32(out, id.index());
    push_u32(out, id.generation());
}

fn decode_surface_id(cursor: &mut Cursor<'_>) -> Result<SurfaceId, IpcCodecError> {
    Ok(SurfaceId::new(cursor.u32()?, cursor.u32()?))
}

fn encode_workspace_id(id: WorkspaceId, out: &mut Vec<u8>) {
    push_u64(out, id.raw());
}

fn decode_workspace_id(cursor: &mut Cursor<'_>) -> Result<WorkspaceId, IpcCodecError> {
    Ok(WorkspaceId::from_raw(cursor.u64()?))
}

fn encode_output_id(id: OutputId, out: &mut Vec<u8>) {
    push_u64(out, id.raw());
}

fn decode_output_id(cursor: &mut Cursor<'_>) -> Result<OutputId, IpcCodecError> {
    Ok(OutputId::from_raw(cursor.u64()?))
}

fn encode_rect(rect: Rect, out: &mut Vec<u8>) {
    push_i32(out, rect.x);
    push_i32(out, rect.y);
    push_i32(out, rect.width);
    push_i32(out, rect.height);
}

fn decode_rect(cursor: &mut Cursor<'_>) -> Result<Rect, IpcCodecError> {
    Ok(Rect {
        x: cursor.i32()?,
        y: cursor.i32()?,
        width: cursor.i32()?,
        height: cursor.i32()?,
    })
}

fn encode_size(size: Size, out: &mut Vec<u8>) {
    push_i32(out, size.width);
    push_i32(out, size.height);
}

fn decode_size(cursor: &mut Cursor<'_>) -> Result<Size, IpcCodecError> {
    Ok(Size {
        width: cursor.i32()?,
        height: cursor.i32()?,
    })
}

fn encode_option_size(size: Option<Size>, out: &mut Vec<u8>) {
    match size {
        Some(size) => {
            push_u8(out, 1);
            encode_size(size, out);
        }
        None => push_u8(out, 0),
    }
}

fn decode_option_size(cursor: &mut Cursor<'_>) -> Result<Option<Size>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_size(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "option_size",
            value: other,
        }),
    }
}

fn encode_option_rect(rect: Option<Rect>, out: &mut Vec<u8>) {
    match rect {
        Some(rect) => {
            push_u8(out, 1);
            encode_rect(rect, out);
        }
        None => push_u8(out, 0),
    }
}

fn decode_option_rect(cursor: &mut Cursor<'_>) -> Result<Option<Rect>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_rect(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "option_rect",
            value: other,
        }),
    }
}

fn encode_transform(transform: Transform, out: &mut Vec<u8>) {
    for value in transform.matrix {
        push_u32(out, value.to_bits());
    }
}

fn decode_transform(cursor: &mut Cursor<'_>) -> Result<Transform, IpcCodecError> {
    let mut matrix = [0.0; 9];
    for value in &mut matrix {
        *value = f32::from_bits(cursor.u32()?);
    }
    Ok(Transform { matrix })
}

fn encode_layout_node_kind(kind: LayoutNodeKind) -> u16 {
    match kind {
        LayoutNodeKind::Toplevel => 1,
        LayoutNodeKind::Dialog => 2,
        LayoutNodeKind::Utility => 3,
        LayoutNodeKind::Unknown => 4,
    }
}

fn decode_layout_node_kind(value: u16) -> Result<LayoutNodeKind, IpcCodecError> {
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

fn encode_capabilities(capabilities: LayoutNodeCapabilities) -> u16 {
    u16::from(capabilities.movable)
        | (u16::from(capabilities.resizable) << 1)
        | (u16::from(capabilities.focusable) << 2)
        | (u16::from(capabilities.closable) << 3)
        | (u16::from(capabilities.fullscreenable) << 4)
}

fn decode_capabilities(bits: u16) -> LayoutNodeCapabilities {
    LayoutNodeCapabilities {
        movable: bits & 1 != 0,
        resizable: bits & (1 << 1) != 0,
        focusable: bits & (1 << 2) != 0,
        closable: bits & (1 << 3) != 0,
        fullscreenable: bits & (1 << 4) != 0,
    }
}

fn encode_node_state(state: LayoutNodeState) -> u16 {
    u16::from(state.focused)
        | (u16::from(state.urgent) << 1)
        | (u16::from(state.fullscreen) << 2)
        | (u16::from(state.floating) << 3)
        | (u16::from(state.visible) << 4)
}

fn decode_node_state(bits: u16) -> LayoutNodeState {
    LayoutNodeState {
        focused: bits & 1 != 0,
        urgent: bits & (1 << 1) != 0,
        fullscreen: bits & (1 << 2) != 0,
        floating: bits & (1 << 3) != 0,
        visible: bits & (1 << 4) != 0,
    }
}

fn check_count(count: usize) -> Result<(), IpcCodecError> {
    if count > SOPHIA_IPC_MAX_ITEMS {
        Err(IpcCodecError::CountTooLarge {
            count,
            max: SOPHIA_IPC_MAX_ITEMS,
        })
    } else {
        Ok(())
    }
}

fn decode_count(cursor: &mut Cursor<'_>) -> Result<usize, IpcCodecError> {
    let count = cursor.u32()? as usize;
    check_count(count)?;
    Ok(count)
}

fn encode_optional_text(
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

fn decode_optional_text(
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

fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(&self) -> Result<(), IpcCodecError> {
        let remaining = self.bytes.len().saturating_sub(self.offset);
        if remaining == 0 {
            Ok(())
        } else {
            Err(IpcCodecError::TrailingBytes(remaining))
        }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], IpcCodecError> {
        let end = self.offset.checked_add(N).ok_or(IpcCodecError::Truncated)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(IpcCodecError::Truncated)?;
        self.offset = end;
        let mut out = [0; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    fn slice(&mut self, len: usize) -> Result<&'a [u8], IpcCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(IpcCodecError::Truncated)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(IpcCodecError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, IpcCodecError> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, IpcCodecError> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    fn u32(&mut self) -> Result<u32, IpcCodecError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, IpcCodecError> {
        Ok(u64::from_le_bytes(self.take()?))
    }

    fn i32(&mut self) -> Result<i32, IpcCodecError> {
        Ok(i32::from_le_bytes(self.take()?))
    }
}

use crate::{
    XAuthorityRequestKind, XAuthorityResponseOutcome, XAuthorityResponsePacket, XAuthorityRuntime,
    XByteOrder, XClientEvent, XClientOutput, XPropertyTable, XResourceId, XWireParseError,
    XWireRequest, encode_x_client_output, x_error_from_runtime, x_error_from_wire_parse,
    x_selection_failure_event,
};
use sophia_protocol::NamespaceId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDispatchContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub sequence: u16,
    pub major_opcode: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XDispatchResult {
    pub response: Option<XAuthorityResponsePacket>,
    pub outputs: Vec<XClientOutput>,
}

impl XDispatchResult {
    pub fn encoded_outputs(&self, byte_order: XByteOrder) -> Vec<[u8; 32]> {
        self.outputs
            .iter()
            .map(|output| encode_x_client_output(byte_order, *output))
            .collect()
    }
}

pub fn dispatch_x11_wire_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    properties: &mut XPropertyTable,
) -> XDispatchResult {
    match request {
        XWireRequest::Authority(packet) => {
            let kind = packet.kind.clone();
            let response = runtime.apply(packet);
            let outputs = outputs_from_authority_response(context, &kind, &response);
            XDispatchResult {
                response: Some(response),
                outputs,
            }
        }
        XWireRequest::ChangeProperty(change) => {
            let output = match properties.apply_change(context.namespace, change.clone()) {
                Ok(record) => XClientOutput::Event(XClientEvent::PropertyNotify {
                    sequence: context.sequence,
                    window: record.window,
                    atom: record.property,
                    time: 0,
                    new_value: true,
                }),
                Err(_) => XClientOutput::Error(crate::XClientError {
                    code: crate::XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: u32::try_from(change.window.local.raw()).unwrap_or(0),
                    minor_code: 0,
                    major_code: context.major_opcode,
                }),
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
            }
        }
    }
}

pub fn dispatch_x11_parse_error(
    context: XDispatchContext,
    error: XWireParseError,
) -> XDispatchResult {
    XDispatchResult {
        response: None,
        outputs: vec![XClientOutput::Error(x_error_from_wire_parse(
            &error,
            context.sequence,
            context.major_opcode,
        ))],
    }
}

fn outputs_from_authority_response(
    context: XDispatchContext,
    kind: &XAuthorityRequestKind,
    response: &XAuthorityResponsePacket,
) -> Vec<XClientOutput> {
    if let XAuthorityRequestKind::RequestSelection {
        requestor,
        selection,
        target,
        time,
        ..
    } = kind
    {
        if !response.selection_artifacts.is_empty() {
            return vec![XClientOutput::Event(x_selection_failure_event(
                context.sequence,
                *time,
                *requestor,
                *selection,
                *target,
            ))];
        }
    }

    if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
        return vec![XClientOutput::Error(x_error_from_runtime(
            error,
            context.sequence,
            context.major_opcode,
            resource_from_kind(kind),
        ))];
    }

    match kind {
        XAuthorityRequestKind::CreateWindow {
            window, geometry, ..
        } => vec![XClientOutput::Event(XClientEvent::ConfigureNotify {
            sequence: context.sequence,
            event: *window,
            window: *window,
            above_sibling: None,
            x: clamp_i16(geometry.x),
            y: clamp_i16(geometry.y),
            width: clamp_u16(geometry.width),
            height: clamp_u16(geometry.height),
            border_width: 0,
            override_redirect: false,
        })],
        XAuthorityRequestKind::MapWindow { window, .. } => {
            vec![XClientOutput::Event(XClientEvent::MapNotify {
                sequence: context.sequence,
                event: *window,
                window: *window,
                override_redirect: false,
            })]
        }
        XAuthorityRequestKind::RequestSelection { .. } => Vec::new(),
        XAuthorityRequestKind::SetSelectionOwner { .. }
        | XAuthorityRequestKind::PresentPixmap { .. } => Vec::new(),
    }
}

fn resource_from_kind(kind: &XAuthorityRequestKind) -> u32 {
    let resource = match kind {
        XAuthorityRequestKind::CreateWindow { window, .. }
        | XAuthorityRequestKind::MapWindow { window, .. }
        | XAuthorityRequestKind::PresentPixmap { window, .. } => *window,
        XAuthorityRequestKind::SetSelectionOwner { owner, .. } => {
            owner.unwrap_or(XResourceId::NONE)
        }
        XAuthorityRequestKind::RequestSelection { requestor, .. } => *requestor,
    };
    u32::try_from(resource.local.raw()).unwrap_or(0)
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn clamp_u16(value: i32) -> u16 {
    value.clamp(0, i32::from(u16::MAX)) as u16
}

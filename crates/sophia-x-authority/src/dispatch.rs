use crate::{
    X_SOPHIA_PRESENT_EXTENSION_NAME, X_SOPHIA_PRESENT_MAJOR_OPCODE, XAtomTable,
    XAuthorityRequestKind, XAuthorityResponseOutcome, XAuthorityResponsePacket, XAuthorityRuntime,
    XByteOrder, XClientEvent, XClientOutput, XClientReply, XErrorCode, XMetadataPropertyCandidate,
    XPropertyError, XPropertyTable, XResourceId, XWireParseError, XWireRequest,
    encode_x_client_output, metadata_property_candidate, x_error_from_runtime,
    x_error_from_wire_parse, x_selection_failure_event,
};
use sophia_protocol::{NamespaceId, Rect, Region, TransactionId};

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
    pub metadata_candidates: Vec<XMetadataPropertyCandidate>,
}

impl XDispatchResult {
    pub fn encoded_outputs(&self, byte_order: XByteOrder) -> Vec<Vec<u8>> {
        self.outputs
            .iter()
            .map(|output| encode_x_client_output(byte_order, output.clone()))
            .collect()
    }
}

pub fn dispatch_x11_wire_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    atoms: &mut XAtomTable,
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
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::DestroyWindow { window } => {
            let outputs =
                if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))]
                } else {
                    Vec::new()
                };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::InternAtom {
            only_if_exists,
            name,
        } => {
            let output = match atoms.intern(&name, only_if_exists) {
                Ok(atom) => XClientOutput::Reply(XClientReply::InternAtom {
                    sequence: context.sequence,
                    atom: atom.unwrap_or(0),
                }),
                Err(_) => XClientOutput::Error(crate::XClientError {
                    code: crate::XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: 0,
                    minor_code: 0,
                    major_code: context.major_opcode,
                }),
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GetAtomName { atom } => {
            let output = match atoms.name(atom) {
                Some(name) => XClientOutput::Reply(XClientReply::GetAtomName {
                    sequence: context.sequence,
                    name: name.to_owned(),
                }),
                None => XClientOutput::Error(crate::XClientError {
                    code: crate::XErrorCode::BadAtom,
                    sequence: context.sequence,
                    resource_id: atom,
                    minor_code: 0,
                    major_code: context.major_opcode,
                }),
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ChangeProperty(change) => {
            let (output, metadata_candidates) =
                match properties.apply_change(context.namespace, change.clone()) {
                    Ok(record) => {
                        let candidate = metadata_property_candidate(&record, atoms);
                        (
                            XClientOutput::Event(XClientEvent::PropertyNotify {
                                sequence: context.sequence,
                                window: record.window,
                                atom: record.property,
                                time: 0,
                                new_value: true,
                            }),
                            candidate.into_iter().collect(),
                        )
                    }
                    Err(_) => (
                        XClientOutput::Error(crate::XClientError {
                            code: crate::XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: u32::try_from(change.window.local.raw()).unwrap_or(0),
                            minor_code: 0,
                            major_code: context.major_opcode,
                        }),
                        Vec::new(),
                    ),
                };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates,
            }
        }
        XWireRequest::GetProperty(read) => {
            let output = if read.property == crate::X_PROPERTY_ANY_TYPE
                || atoms.name(read.property).is_none()
                || atom_type_is_unknown(atoms, read.property_type)
            {
                XClientOutput::Error(crate::XClientError {
                    code: crate::XErrorCode::BadAtom,
                    sequence: context.sequence,
                    resource_id: read.property,
                    minor_code: 0,
                    major_code: context.major_opcode,
                })
            } else if read.window.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
                x_client_output_from_property_read(
                    &context,
                    properties.read_property(context.namespace, read),
                )
            } else if let Err(error) =
                runtime.validate_window_access(context.namespace, read.window)
            {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(read.window.local.raw()).unwrap_or(0),
                ))
            } else {
                x_client_output_from_property_read(
                    &context,
                    properties.read_property(context.namespace, read),
                )
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CreateGraphicsContext { .. } | XWireRequest::FreeGraphicsContext { .. } => {
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GetInputFocus => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::GetInputFocus {
                sequence: context.sequence,
                focus: XResourceId::new(u64::from(crate::X_SETUP_DEFAULT_ROOT), 1),
                revert_to: 1,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::QueryExtension { name } => {
            let present = name == X_SOPHIA_PRESENT_EXTENSION_NAME;
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::QueryExtension {
                    sequence: context.sequence,
                    present,
                    major_opcode: if present {
                        X_SOPHIA_PRESENT_MAJOR_OPCODE
                    } else {
                        0
                    },
                    first_event: 0,
                    first_error: 0,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ListExtensions => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::ListExtensions {
                sequence: context.sequence,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::QueryBestSize { width, height, .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::QueryBestSize {
                sequence: context.sequence,
                width,
                height,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::PolyFillRectangle {
            drawable,
            rectangles,
            ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let mut damage = Region::empty();
            for rectangle in rectangles {
                damage.push(rectangle);
            }
            let response =
                runtime.apply_core_draw(transaction, context.namespace, drawable, damage);
            let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(drawable.local.raw()).unwrap_or(0),
                ))]
            } else {
                Vec::new()
            };
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::PutImage {
            drawable,
            width,
            height,
            dst_x,
            dst_y,
            data_len,
            ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let damage = Region::single(Rect {
                x: i32::from(dst_x),
                y: i32::from(dst_y),
                width: i32::from(width),
                height: i32::from(height),
            });
            let response =
                runtime.apply_put_image(transaction, context.namespace, drawable, damage, data_len);
            let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(drawable.local.raw()).unwrap_or(0),
                ))]
            } else {
                Vec::new()
            };
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
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
        metadata_candidates: Vec::new(),
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

fn atom_type_is_unknown(atoms: &XAtomTable, atom: u32) -> bool {
    atom != crate::X_PROPERTY_ANY_TYPE && atoms.name(atom).is_none()
}

fn x_client_output_from_property_read(
    context: &XDispatchContext,
    result: Result<crate::XPropertyReadReply, XPropertyError>,
) -> XClientOutput {
    match result {
        Ok(reply) => XClientOutput::Reply(XClientReply::GetProperty {
            sequence: context.sequence,
            property_type: reply.property_type,
            format: reply.format,
            bytes_after: reply.bytes_after,
            item_count: reply.item_count,
            bytes: reply.bytes,
        }),
        Err(error) => XClientOutput::Error(crate::XClientError {
            code: x_error_from_property_read(error),
            sequence: context.sequence,
            resource_id: 0,
            minor_code: 0,
            major_code: context.major_opcode,
        }),
    }
}

fn x_error_from_property_read(error: XPropertyError) -> XErrorCode {
    match error {
        XPropertyError::InvalidNamespace | XPropertyError::InvalidWindow => XErrorCode::BadWindow,
        XPropertyError::InvalidFormat(_)
        | XPropertyError::ValueTooLarge { .. }
        | XPropertyError::TypeMismatch
        | XPropertyError::InvalidOffset
        | XPropertyError::ReadTooLarge { .. } => XErrorCode::BadValue,
    }
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn clamp_u16(value: i32) -> u16 {
    value.clamp(0, i32::from(u16::MAX)) as u16
}

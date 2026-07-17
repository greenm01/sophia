use crate::{
    X_ATOM_NONE, X_BIG_REQUESTS_EXTENSION_NAME, X_BIG_REQUESTS_MAJOR_OPCODE,
    X_MIT_SHM_EXTENSION_NAME, X_MIT_SHM_MAJOR_OPCODE, X_RANDR_EXTENSION_NAME, X_RANDR_MAJOR_OPCODE,
    X_SETUP_DEFAULT_COLORMAP, X_SETUP_DEFAULT_ROOT, X_SETUP_DEFAULT_VISUAL,
    X_SOPHIA_PRESENT_EXTENSION_NAME, X_SOPHIA_PRESENT_MAJOR_OPCODE, XAtomTable,
    XAuthorityRequestKind, XAuthorityResponseOutcome, XAuthorityResponsePacket, XAuthorityRuntime,
    XAuthorityRuntimeError, XByteOrder, XClientEvent, XClientOutput, XClientReply, XErrorCode,
    XMetadataPropertyCandidate, XPropertyError, XPropertyTable, XRandrModeInfo, XRandrMonitorInfo,
    XResourceId, XWireParseError, XWireRequest, XXiDeviceClass, XXiDeviceInfo,
    encode_x_client_output, metadata_property_candidate, x_error_from_runtime,
    x_error_from_wire_parse, x_selection_failure_event,
};
use sophia_protocol::{NamespaceId, OutputTopologySnapshot, Rect, Region, TransactionId};

const DRM_FORMAT_MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDispatchContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub sequence: u16,
    pub major_opcode: u8,
    pub client_id: u64,
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
    runtime.begin_dispatch();
    match request {
        XWireRequest::CreateWindow {
            packet,
            background_pixel,
            ..
        } => {
            let kind = packet.kind.clone();
            let namespace = packet.namespace;
            let response = runtime.apply(packet);
            if response.outcome == XAuthorityResponseOutcome::Accepted
                && let XAuthorityRequestKind::CreateWindow { window, .. } = &kind
            {
                let _ = runtime.set_window_background_pixel(
                    namespace,
                    *window,
                    background_pixel.unwrap_or(0),
                );
            }
            let outputs = outputs_from_authority_response(context.clone(), &kind, &response);
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::Authority(mut packet) => {
            if let XAuthorityRequestKind::RequestSelection {
                target,
                target_name,
                ..
            } = &mut packet.kind
                && let Some(name) = atoms.name(*target)
            {
                *target_name = name.to_owned();
            }
            let kind = packet.kind.clone();
            let response = runtime.apply(packet);
            if let XAuthorityRequestKind::RequestSelection { transfer, .. } = &kind {
                runtime.set_pending_clipboard_byte_order(*transfer, context.byte_order);
            }
            let outputs = outputs_from_authority_response(context, &kind, &response);
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ChangeWindowAttributes { window, .. } => {
            let outputs =
                if let Err(error) = runtime.validate_drawable_access(context.namespace, window) {
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
        XWireRequest::GetWindowAttributes { window } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::GetWindowAttributes {
                    sequence: context.sequence,
                    visual: X_SETUP_DEFAULT_VISUAL,
                    colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_COLORMAP), 1),
                    map_state: 2,
                    override_redirect: false,
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::GetWindowAttributes {
                    sequence: context.sequence,
                    visual: X_SETUP_DEFAULT_VISUAL,
                    colormap: XResourceId::new(u64::from(X_SETUP_DEFAULT_COLORMAP), 1),
                    map_state: 2,
                    override_redirect: false,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::DestroyWindow { window } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let mut response = XAuthorityResponsePacket::accepted(transaction);
            let outputs = match runtime.destroy_window(context.namespace, window) {
                Ok(surface) => {
                    properties.remove_window(context.namespace, window);
                    response.removed_surfaces.push(surface);
                    Vec::new()
                }
                Err(error) => {
                    response = XAuthorityResponsePacket::rejected(transaction, error);
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))]
                }
            };
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::MapSubwindows { window } => {
            let outputs = if let Err(error) =
                runtime.validate_drawable_access(context.namespace, window)
            {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))]
            } else {
                match runtime.map_namespace_windows(context.namespace, u64::from(context.sequence))
                {
                    Ok(surfaces) => surfaces
                        .into_iter()
                        .flat_map(|surface| {
                            let window = XResourceId {
                                local: surface.local_id,
                            };
                            [
                                XClientOutput::Event(XClientEvent::MapNotify {
                                    sequence: context.sequence,
                                    event: window,
                                    window,
                                    override_redirect: false,
                                }),
                                XClientOutput::Event(XClientEvent::Expose {
                                    sequence: context.sequence,
                                    window,
                                    x: 0,
                                    y: 0,
                                    width: clamp_u16(surface.geometry.width),
                                    height: clamp_u16(surface.geometry.height),
                                    count: 0,
                                }),
                            ]
                        })
                        .collect(),
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))],
                }
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::UnmapWindow { window } => {
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
        XWireRequest::ConfigureWindow {
            window,
            x,
            y,
            width,
            height,
            ..
        } => {
            let outputs = if let Err(error) = runtime.configure_window_geometry(
                context.namespace,
                window,
                x,
                y,
                width,
                height,
                u64::from(context.sequence),
            ) {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))]
            } else {
                match runtime.window_geometry(context.namespace, window) {
                    Ok(geometry) => vec![XClientOutput::Event(XClientEvent::ConfigureNotify {
                        sequence: context.sequence,
                        event: window,
                        window,
                        above_sibling: None,
                        x: clamp_i16(geometry.x),
                        y: clamp_i16(geometry.y),
                        width: clamp_u16(geometry.width),
                        height: clamp_u16(geometry.height),
                        border_width: 0,
                        override_redirect: false,
                    })],
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))],
                }
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GetGeometry { drawable } => {
            let output = if drawable.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::GetGeometry {
                    sequence: context.sequence,
                    depth: 24,
                    root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                    geometry: Rect {
                        x: 0,
                        y: 0,
                        width: runtime
                            .output_topology()
                            .root_size()
                            .expect("validated output topology")
                            .width,
                        height: runtime
                            .output_topology()
                            .root_size()
                            .expect("validated output topology")
                            .height,
                    },
                    border_width: 0,
                })
            } else {
                match runtime.window_geometry(context.namespace, drawable) {
                    Ok(geometry) => XClientOutput::Reply(XClientReply::GetGeometry {
                        sequence: context.sequence,
                        depth: 24,
                        root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                        geometry,
                        border_width: 0,
                    }),
                    Err(error) => XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(drawable.local.raw()).unwrap_or(0),
                    )),
                }
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::QueryTree { window } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::QueryTree {
                    sequence: context.sequence,
                    root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                    parent: XResourceId::NONE,
                    children: Vec::new(),
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::QueryTree {
                    sequence: context.sequence,
                    root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                    parent: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                    children: Vec::new(),
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
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
            let window_access = (change.window.local.raw()
                == u64::from(crate::X_SETUP_DEFAULT_ROOT))
            .then_some(Ok(()))
            .unwrap_or_else(|| runtime.validate_window_access(context.namespace, change.window));
            let (output, metadata_candidates) = match window_access {
                Err(error) => (
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(change.window.local.raw()).unwrap_or(0),
                    )),
                    Vec::new(),
                ),
                Ok(()) => match properties.apply_change(context.namespace, change.clone()) {
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
                },
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates,
            }
        }
        XWireRequest::DeleteProperty { window, property } => {
            let access = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                Ok(())
            } else {
                runtime.validate_window_access(context.namespace, window)
            };
            let outputs = match access {
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))],
                Ok(()) => properties
                    .remove(context.namespace, window, property)
                    .map(|_| {
                        XClientOutput::Event(XClientEvent::PropertyNotify {
                            sequence: context.sequence,
                            window,
                            atom: property,
                            time: 0,
                            new_value: false,
                        })
                    })
                    .into_iter()
                    .collect(),
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
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
        XWireRequest::ListProperties { window } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::ListProperties {
                    sequence: context.sequence,
                    atoms: properties.properties_for_window(context.namespace, window),
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::ListProperties {
                    sequence: context.sequence,
                    atoms: properties.properties_for_window(context.namespace, window),
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GetSelectionOwner { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::GetSelectionOwner {
                sequence: context.sequence,
                owner: None,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::SendSelectionNotify {
            destination,
            event_mask,
            mut event,
        } => {
            let requestor = match &event {
                XClientEvent::SelectionNotify { requestor, .. } => Some(*requestor),
                XClientEvent::ClientMessage { .. } => None,
                _ => unreachable!("wire decoder admits only sendable events"),
            };
            let validation = if destination.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                Ok(())
            } else {
                runtime.validate_window_access(context.namespace, destination)
            }
            .and_then(|()| {
                requestor
                    .map(|requestor| runtime.validate_window_access(context.namespace, requestor))
                    .unwrap_or(Ok(()))
            });
            let outputs = match validation {
                Ok(())
                    if (requestor.is_none() || event_mask == 0)
                        && requestor.is_none_or(|requestor| destination == requestor) =>
                {
                    match &mut event {
                        XClientEvent::SelectionNotify { sequence, .. }
                        | XClientEvent::ClientMessage { sequence, .. } => {
                            *sequence = context.sequence;
                        }
                        _ => unreachable!("wire decoder admits only sendable events"),
                    }
                    vec![XClientOutput::Event(event)]
                }
                Ok(()) => vec![XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: u32::try_from(destination.local.raw()).unwrap_or(0),
                    minor_code: 0,
                    major_code: context.major_opcode,
                })],
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(destination.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GrabPointer {
            window,
            event_mask,
            owner_events,
            pointer_mode,
            keyboard_mode,
            ..
        } => {
            let status = if validate_grab_window(runtime, context.namespace, window).is_err() {
                3
            } else {
                runtime
                    .input_authority_mut()
                    .grab_pointer(
                        context.namespace,
                        crate::XActiveInputGrab {
                            owner: context.client_id,
                            window,
                            owner_events,
                            pointer_mode,
                            keyboard_mode,
                            event_mask,
                        },
                    )
                    .map_or(1, |_| 0)
            };
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::GrabStatus {
                    sequence: context.sequence,
                    status,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::UngrabPointer { .. } => {
            runtime
                .input_authority_mut()
                .ungrab_pointer(context.namespace, context.client_id);
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GrabKeyboard {
            window,
            owner_events,
            pointer_mode,
            keyboard_mode,
            ..
        } => {
            let status = if validate_grab_window(runtime, context.namespace, window).is_err() {
                3
            } else {
                runtime
                    .input_authority_mut()
                    .grab_keyboard(
                        context.namespace,
                        crate::XActiveInputGrab {
                            owner: context.client_id,
                            window,
                            owner_events,
                            pointer_mode,
                            keyboard_mode,
                            event_mask: 0,
                        },
                    )
                    .map_or(1, |_| 0)
            };
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::GrabStatus {
                    sequence: context.sequence,
                    status,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::UngrabKeyboard { .. } => {
            runtime
                .input_authority_mut()
                .ungrab_keyboard(context.namespace, context.client_id);
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GrabButton {
            window,
            event_mask,
            button,
            modifiers,
            owner_events,
            pointer_mode,
            keyboard_mode,
        } => {
            let outputs = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                runtime
                    .input_authority_mut()
                    .grab_button(
                        context.namespace,
                        crate::XPassiveInputGrab {
                            owner: context.client_id,
                            window,
                            detail: button,
                            modifiers,
                            owner_events,
                            pointer_mode,
                            keyboard_mode,
                            event_mask,
                        },
                    )
                    .err()
                    .map(|_| grab_access_error(&context, window))
                    .into_iter()
                    .collect()
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))]
            } else {
                runtime
                    .input_authority_mut()
                    .grab_button(
                        context.namespace,
                        crate::XPassiveInputGrab {
                            owner: context.client_id,
                            window,
                            detail: button,
                            modifiers,
                            owner_events,
                            pointer_mode,
                            keyboard_mode,
                            event_mask,
                        },
                    )
                    .err()
                    .map(|_| grab_access_error(&context, window))
                    .into_iter()
                    .collect()
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::UngrabButton {
            window,
            button,
            modifiers,
        } => {
            runtime.input_authority_mut().ungrab_button(
                context.namespace,
                context.client_id,
                window,
                button,
                modifiers,
            );
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GrabKey {
            window,
            key,
            modifiers,
            owner_events,
            pointer_mode,
            keyboard_mode,
        } => {
            let outputs = match validate_grab_window(runtime, context.namespace, window) {
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))],
                Ok(()) => runtime
                    .input_authority_mut()
                    .grab_key(
                        context.namespace,
                        crate::XPassiveInputGrab {
                            owner: context.client_id,
                            window,
                            detail: key,
                            modifiers,
                            owner_events,
                            pointer_mode,
                            keyboard_mode,
                            event_mask: 0,
                        },
                    )
                    .err()
                    .map(|_| grab_access_error(&context, window))
                    .into_iter()
                    .collect(),
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::UngrabKey {
            window,
            key,
            modifiers,
        } => {
            runtime.input_authority_mut().ungrab_key(
                context.namespace,
                context.client_id,
                window,
                key,
                modifiers,
            );
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::AllowEvents { mode, .. } => {
            let invalid = runtime
                .input_authority_mut()
                .allow_events(context.namespace, context.client_id, mode)
                .is_err();
            XDispatchResult {
                response: None,
                outputs: invalid
                    .then(|| {
                        XClientOutput::Error(crate::XClientError {
                            code: XErrorCode::BadValue,
                            sequence: context.sequence,
                            resource_id: u32::from(mode),
                            minor_code: 0,
                            major_code: context.major_opcode,
                        })
                    })
                    .into_iter()
                    .collect(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GrabServer => {
            let _ = runtime
                .input_authority_mut()
                .grab_server(context.namespace, context.client_id);
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::UngrabServer => {
            runtime
                .input_authority_mut()
                .ungrab_server(context.namespace, context.client_id);
            XDispatchResult {
                response: None,
                outputs: Vec::new(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CreateGraphicsContext {
            gc,
            drawable,
            values,
        } => {
            let outputs = runtime
                .create_graphics_context(context.namespace, gc, drawable, values)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(gc.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::SetClipRectangles { gc, rectangles } => {
            let outputs = runtime
                .set_graphics_context_clip_rectangles(context.namespace, gc, rectangles)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(gc.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::FreeGraphicsContext { gc } => {
            let outputs = runtime
                .free_graphics_context(context.namespace, gc)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(gc.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ClearArea {
            window,
            x,
            y,
            width,
            height,
            ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let geometry = runtime.window_geometry(context.namespace, window).ok();
            let clear_width = if width == 0 {
                geometry
                    .map(|geometry| geometry.width.saturating_sub(i32::from(x)).max(0))
                    .unwrap_or(0)
            } else {
                i32::from(width)
            };
            let clear_height = if height == 0 {
                geometry
                    .map(|geometry| geometry.height.saturating_sub(i32::from(y)).max(0))
                    .unwrap_or(0)
            } else {
                i32::from(height)
            };
            let response = match runtime.window_background_pixel(context.namespace, window) {
                Ok(pixel) => runtime.apply_clear_with_pixel(
                    transaction,
                    context.namespace,
                    window,
                    Region::single(Rect {
                        x: i32::from(x),
                        y: i32::from(y),
                        width: clear_width,
                        height: clear_height,
                    }),
                    pixel,
                ),
                Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
            };
            let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
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
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::OpenFont { font, .. } => {
            let outputs =
                match runtime.open_font(context.namespace, font, u64::from(context.sequence)) {
                    Ok(()) => Vec::new(),
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(font.local.raw()).unwrap_or(0),
                    ))],
                };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CloseFont { font } => {
            let outputs = match runtime.close_font(context.namespace, font) {
                Ok(()) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(font.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::QueryFont { font } => {
            let output = match runtime.validate_font_access(context.namespace, font) {
                Ok(()) => XClientOutput::Reply(XClientReply::QueryFont {
                    sequence: context.sequence,
                    font_ascent: 8,
                    font_descent: 2,
                }),
                Err(error) => XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(font.local.raw()).unwrap_or(0),
                )),
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::CreateGlyphCursor {
            cursor,
            source_font,
            mask_font,
        } => {
            let outputs = if let Err(error) =
                runtime.validate_font_access(context.namespace, source_font)
            {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(source_font.local.raw()).unwrap_or(0),
                ))]
            } else if let Some(mask_font) = mask_font {
                if let Err(error) = runtime.validate_font_access(context.namespace, mask_font) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(mask_font.local.raw()).unwrap_or(0),
                    ))]
                } else {
                    match runtime.create_cursor(
                        context.namespace,
                        cursor,
                        u64::from(context.sequence),
                    ) {
                        Ok(()) => Vec::new(),
                        Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                            error,
                            context.sequence,
                            context.major_opcode,
                            u32::try_from(cursor.local.raw()).unwrap_or(0),
                        ))],
                    }
                }
            } else {
                match runtime.create_cursor(context.namespace, cursor, u64::from(context.sequence))
                {
                    Ok(()) => Vec::new(),
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(cursor.local.raw()).unwrap_or(0),
                    ))],
                }
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::FreeCursor { cursor } => {
            let outputs = match runtime.free_cursor(context.namespace, cursor) {
                Ok(()) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(cursor.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RecolorCursor { cursor } => {
            let outputs = match runtime.validate_cursor_access(context.namespace, cursor) {
                Ok(()) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(cursor.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ListFonts { max_names, .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::ListFonts {
                sequence: context.sequence,
                names: if max_names == 0 {
                    Vec::new()
                } else {
                    vec!["fixed".to_owned()]
                },
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::ListFontsWithInfo { max_names, .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::ListFontsWithInfo {
                sequence: context.sequence,
                names: if max_names == 0 {
                    Vec::new()
                } else {
                    vec!["fixed".to_owned()]
                },
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::CreatePixmap {
            pixmap, drawable, ..
        } => {
            let outputs =
                if let Err(error) = runtime.validate_drawable_access(context.namespace, drawable) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(drawable.local.raw()).unwrap_or(0),
                    ))]
                } else if let Err(error) =
                    runtime.create_pixmap(context.namespace, pixmap, u64::from(context.sequence))
                {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(pixmap.local.raw()).unwrap_or(0),
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
        XWireRequest::FreePixmap { pixmap } => {
            let outputs = match runtime.free_pixmap(context.namespace, pixmap) {
                Ok(_) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(pixmap.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GetInputFocus => {
            let (focus, revert_to) = runtime.input_focus(context.namespace);
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::GetInputFocus {
                    sequence: context.sequence,
                    focus,
                    revert_to,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::SetInputFocus {
            focus, revert_to, ..
        } => {
            let outputs = runtime
                .set_input_focus(context.namespace, focus, revert_to)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(focus.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect();
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::GetModifierMapping => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::GetModifierMapping {
                sequence: context.sequence,
                keycodes_per_modifier: 2,
                keycodes: vec![50, 62, 66, 0, 37, 105, 64, 108, 77, 0, 0, 0, 133, 134, 0, 0],
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::GetKeyboardMapping {
            first_keycode,
            count,
        } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::GetKeyboardMapping {
                sequence: context.sequence,
                keysyms_per_keycode: 2,
                keysyms: runtime.xkb_keymap().core_mapping(first_keycode, count),
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::TranslateCoordinates {
            source,
            destination,
            src_x,
            src_y,
        } => {
            let output =
                if let Err(error) = runtime.validate_drawable_access(context.namespace, source) {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(source.local.raw()).unwrap_or(0),
                    ))
                } else if let Err(error) =
                    runtime.validate_drawable_access(context.namespace, destination)
                {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(destination.local.raw()).unwrap_or(0),
                    ))
                } else {
                    XClientOutput::Reply(XClientReply::TranslateCoordinates {
                        sequence: context.sequence,
                        same_screen: true,
                        child: None,
                        dst_x: src_x,
                        dst_y: src_y,
                    })
                };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::QueryPointer { window } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT)
                || runtime
                    .validate_window_access(context.namespace, window)
                    .is_ok()
            {
                XClientOutput::Reply(XClientReply::QueryPointer {
                    sequence: context.sequence,
                    root: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
                    child: XResourceId::NONE,
                    root_x: 0,
                    root_y: 0,
                    win_x: 0,
                    win_y: 0,
                    mask: 0,
                })
            } else {
                XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadWindow,
                    sequence: context.sequence,
                    resource_id: u32::try_from(window.local.raw()).unwrap_or(0),
                    minor_code: 0,
                    major_code: context.major_opcode,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::QueryExtension { name } => {
            let extension = extension_query_result(&name);
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::QueryExtension {
                    sequence: context.sequence,
                    present: extension.present,
                    major_opcode: extension.major_opcode,
                    first_event: extension.first_event,
                    first_error: extension.first_error,
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
        XWireRequest::QueryColors { pixels, .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::QueryColors {
                sequence: context.sequence,
                pixels,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::CreateColormap { window, .. } => {
            let outputs = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                Vec::new()
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
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
        XWireRequest::AllocNamedColor { name, .. } => {
            let black = name.eq_ignore_ascii_case("black");
            let intensity = if black { 0 } else { u16::MAX };
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::AllocNamedColor {
                    sequence: context.sequence,
                    pixel: if black { 0 } else { 1 },
                    red: intensity,
                    green: intensity,
                    blue: intensity,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::AllocColor {
            red, green, blue, ..
        } => {
            let pixel = true_color_pixel_from_rgb16(red, green, blue);
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::AllocColor {
                    sequence: context.sequence,
                    pixel,
                    red,
                    green,
                    blue,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ShmQueryVersion => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::ShmQueryVersion {
                sequence: context.sequence,
                major_version: 1,
                minor_version: 2,
                shared_pixmaps: false,
                pixmap_format: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::Dri3QueryVersion { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::Dri3QueryVersion {
                sequence: context.sequence,
                major_version: 1,
                minor_version: 2,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XfixesQueryVersion { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XfixesQueryVersion {
                sequence: context.sequence,
                major_version: 6,
                minor_version: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XfixesCreateRegion { region, rectangles } => {
            let output = runtime
                .create_xfixes_region(
                    context.namespace,
                    region,
                    rectangles,
                    u64::from(context.sequence),
                )
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(region.local.raw()).unwrap_or(0),
                    ))
                });
            XDispatchResult {
                response: None,
                outputs: output.into_iter().collect(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XfixesSetRegion { region, rectangles } => {
            let output = runtime
                .set_xfixes_region(context.namespace, region, rectangles)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(region.local.raw()).unwrap_or(0),
                    ))
                });
            XDispatchResult {
                response: None,
                outputs: output.into_iter().collect(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XfixesDestroyRegion { region } => {
            let output = runtime
                .destroy_xfixes_region(context.namespace, region)
                .err()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(region.local.raw()).unwrap_or(0),
                    ))
                });
            XDispatchResult {
                response: None,
                outputs: output.into_iter().collect(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XfixesSelectSelectionInput {
            window,
            selection,
            event_mask,
        } => {
            let output = if event_mask & !0b111 != 0 {
                Some(XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: event_mask,
                    minor_code: crate::X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE.into(),
                    major_code: context.major_opcode,
                }))
            } else if atoms.name(selection).is_none() {
                Some(XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadAtom,
                    sequence: context.sequence,
                    resource_id: selection,
                    minor_code: crate::X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE.into(),
                    major_code: context.major_opcode,
                }))
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                let mut error = x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                );
                error.minor_code = crate::X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE.into();
                Some(XClientOutput::Error(error))
            } else {
                None
            };
            XDispatchResult {
                response: None,
                outputs: output.into_iter().collect(),
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::Dri3Open { drawable, provider } => {
            let outputs = if provider != 0 {
                vec![XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: provider,
                    minor_code: u16::from(crate::X_DRI3_OPEN_MINOR_OPCODE),
                    major_code: context.major_opcode,
                })]
            } else if let Err(error) = runtime.validate_drawable_access(context.namespace, drawable)
            {
                let mut error = x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(drawable.local.raw()).unwrap_or(0),
                );
                error.minor_code = u16::from(crate::X_DRI3_OPEN_MINOR_OPCODE);
                vec![XClientOutput::Error(error)]
            } else {
                vec![XClientOutput::Reply(XClientReply::Dri3Open {
                    sequence: context.sequence,
                })]
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::Dri3PixmapFromBuffer {
            pixmap,
            drawable,
            size_bytes,
            width,
            height,
            stride,
            depth,
            bits_per_pixel,
        } => {
            let outputs =
                if let Err(error) = runtime.validate_drawable_access(context.namespace, drawable) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(drawable.local.raw()).unwrap_or(0),
                    ))]
                } else if let Err(error) = runtime.create_dri3_pixmap(
                    context.namespace,
                    pixmap,
                    u64::from(context.sequence),
                    size_bytes,
                    width,
                    height,
                    stride,
                    depth,
                    bits_per_pixel,
                ) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(pixmap.local.raw()).unwrap_or(0),
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
        XWireRequest::Dri3PixmapFromBuffers {
            pixmap,
            window,
            num_buffers,
            width,
            height,
            strides,
            offsets,
            depth,
            bits_per_pixel,
            modifier,
        } => {
            let outputs =
                if let Err(error) = runtime.validate_drawable_access(context.namespace, window) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))]
                } else if let Err(error) = runtime.create_dri3_pixmap_from_buffers(
                    context.namespace,
                    pixmap,
                    u64::from(context.sequence),
                    num_buffers,
                    width,
                    height,
                    strides,
                    offsets,
                    depth,
                    bits_per_pixel,
                    modifier,
                ) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(pixmap.local.raw()).unwrap_or(0),
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
        XWireRequest::Dri3FenceFromFd {
            drawable, fence, ..
        } => {
            let outputs =
                if let Err(error) = runtime.validate_drawable_access(context.namespace, drawable) {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(drawable.local.raw()).unwrap_or(0),
                    ))]
                } else if let Err(error) =
                    runtime.create_dri3_fence(context.namespace, fence, u64::from(context.sequence))
                {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(fence.local.raw()).unwrap_or(0),
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
        XWireRequest::Dri3GetSupportedModifiers {
            window,
            depth,
            bits_per_pixel,
        } => {
            let outputs = if depth != 24 || bits_per_pixel != 32 {
                vec![XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: u32::from(depth),
                    minor_code: u16::from(crate::X_DRI3_GET_SUPPORTED_MODIFIERS_MINOR_OPCODE),
                    major_code: context.major_opcode,
                })]
            } else if window.local.raw() != u64::from(crate::X_SETUP_DEFAULT_ROOT) {
                match runtime.validate_window_access(context.namespace, window) {
                    Ok(()) => vec![XClientOutput::Reply(
                        XClientReply::Dri3GetSupportedModifiers {
                            sequence: context.sequence,
                            window_modifiers: Vec::new(),
                            screen_modifiers: vec![0, DRM_FORMAT_MOD_INVALID],
                        },
                    )],
                    Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))],
                }
            } else {
                vec![XClientOutput::Reply(
                    XClientReply::Dri3GetSupportedModifiers {
                        sequence: context.sequence,
                        window_modifiers: Vec::new(),
                        screen_modifiers: vec![0, DRM_FORMAT_MOD_INVALID],
                    },
                )]
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::PresentQueryVersion { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::PresentQueryVersion {
                sequence: context.sequence,
                major_version: 1,
                minor_version: 2,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::PresentQueryCapabilities { target } => {
            let outputs = if target.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT)
                || runtime
                    .validate_window_access(context.namespace, target)
                    .is_ok()
            {
                vec![XClientOutput::Reply(
                    XClientReply::PresentQueryCapabilities {
                        sequence: context.sequence,
                        capabilities: 1 << 1,
                    },
                )]
            } else {
                vec![XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadWindow,
                    sequence: context.sequence,
                    resource_id: u32::try_from(target.local.raw()).unwrap_or(0),
                    minor_code: u16::from(crate::X_PRESENT_QUERY_CAPABILITIES_MINOR_OPCODE),
                    major_code: context.major_opcode,
                })]
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::PresentSelectInput {
            window, event_mask, ..
        } => {
            let outputs = if event_mask & !0x0f != 0 {
                vec![XClientOutput::Error(crate::XClientError {
                    code: XErrorCode::BadValue,
                    sequence: context.sequence,
                    resource_id: event_mask,
                    minor_code: u16::from(crate::X_PRESENT_SELECT_INPUT_MINOR_OPCODE),
                    major_code: context.major_opcode,
                })]
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
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
        XWireRequest::PresentPixmap {
            window,
            pixmap,
            valid_region,
            update_region,
            target_crtc,
            wait_fence,
            idle_fence,
            options,
            divisor,
            remainder,
            ..
        } => {
            let invalid_value = target_crtc != 0
                || options & !0x0f != 0
                || (divisor == 0 && remainder != 0)
                || (divisor != 0 && remainder >= divisor);
            let validation = if invalid_value {
                Err(XAuthorityRuntimeError::InvalidResource)
            } else {
                let valid_region = XResourceId::new(u64::from(valid_region), 1);
                let update_region = XResourceId::new(u64::from(update_region), 1);
                runtime
                    .validate_window_access(context.namespace, window)
                    .and_then(|()| {
                        valid_region
                            .is_valid()
                            .then_some(valid_region)
                            .map_or(Ok(()), |region| {
                                runtime.validate_xfixes_region_access(context.namespace, region)
                            })
                    })
                    .and_then(|()| {
                        update_region
                            .is_valid()
                            .then_some(update_region)
                            .map_or(Ok(()), |region| {
                                runtime.validate_xfixes_region_access(context.namespace, region)
                            })
                    })
                    .and_then(|()| runtime.validate_pixmap_access(context.namespace, pixmap))
                    .and_then(|()| {
                        wait_fence.map_or(Ok(()), |fence| {
                            runtime.validate_dri3_fence_access(context.namespace, fence)
                        })
                    })
                    .and_then(|()| {
                        idle_fence.map_or(Ok(()), |fence| {
                            runtime.validate_dri3_fence_access(context.namespace, fence)
                        })
                    })
            };
            if let Err(error) = validation {
                if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
                    eprintln!(
                        "sophia_present_validation schema=1 sequence={} invalid={} target_crtc={:#x} options={:#x} divisor={} remainder={} window={:?} pixmap={:?} valid_region={:?} update_region={:?} wait_fence={:?} idle_fence={:?}",
                        context.sequence,
                        invalid_value,
                        target_crtc,
                        options,
                        divisor,
                        remainder,
                        runtime.validate_window_access(context.namespace, window),
                        runtime.validate_pixmap_access(context.namespace, pixmap),
                        (valid_region != 0).then(|| runtime.validate_xfixes_region_access(
                            context.namespace,
                            XResourceId::new(u64::from(valid_region), 1)
                        )),
                        (update_region != 0).then(|| runtime.validate_xfixes_region_access(
                            context.namespace,
                            XResourceId::new(u64::from(update_region), 1)
                        )),
                        wait_fence
                            .map(|fence| runtime
                                .validate_dri3_fence_access(context.namespace, fence)),
                        idle_fence
                            .map(|fence| runtime
                                .validate_dri3_fence_access(context.namespace, fence)),
                    );
                }
                return XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(pixmap.local.raw()).unwrap_or(0),
                    ))],
                    metadata_candidates: Vec::new(),
                };
            }
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let response =
                runtime.present_standard_pixmap(transaction, context.namespace, window, pixmap);
            let outputs = match response.outcome {
                XAuthorityResponseOutcome::Accepted => Vec::new(),
                XAuthorityResponseOutcome::Rejected(error) => {
                    vec![XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(pixmap.local.raw()).unwrap_or(0),
                    ))]
                }
            };
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrQueryVersion { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::RandrQueryVersion {
                sequence: context.sequence,
                major_version: 1,
                minor_version: 5,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::RandrSelectInput { window, .. } => {
            let outputs = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                Vec::new()
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
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
        XWireRequest::RandrGetScreenSizeRange { window } => {
            let root_size = runtime
                .output_topology()
                .root_size()
                .expect("validated output topology");
            let root_width = u16::try_from(root_size.width).expect("validated output width");
            let root_height = u16::try_from(root_size.height).expect("validated output height");
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetScreenSizeRange {
                    sequence: context.sequence,
                    min_width: root_width,
                    min_height: root_height,
                    max_width: root_width,
                    max_height: root_height,
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::RandrGetScreenSizeRange {
                    sequence: context.sequence,
                    min_width: root_width,
                    min_height: root_height,
                    max_width: root_width,
                    max_height: root_height,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetScreenResources { window, .. } => {
            let resources = randr_resources(runtime.output_topology());
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetScreenResources {
                    sequence: context.sequence,
                    timestamp: resources.timestamp,
                    crtcs: resources.crtcs.clone(),
                    outputs: resources.outputs.clone(),
                    modes: resources.modes.clone(),
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::RandrGetScreenResources {
                    sequence: context.sequence,
                    timestamp: resources.timestamp,
                    crtcs: resources.crtcs,
                    outputs: resources.outputs,
                    modes: resources.modes,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetOutputInfo { output, .. } => {
            let resources = randr_resources(runtime.output_topology());
            let client_output = resources
                .outputs
                .iter()
                .position(|candidate| *candidate == output)
                .map(|index| {
                    let entry = &runtime.output_topology().outputs[index];
                    let mode = resources.modes[index].id;
                    XClientOutput::Reply(XClientReply::RandrGetOutputInfo {
                        sequence: context.sequence,
                        timestamp: resources.timestamp,
                        crtc: resources.crtcs[index],
                        mm_width: logical_pixels_to_millimeters(entry.logical.width),
                        mm_height: logical_pixels_to_millimeters(entry.logical.height),
                        crtcs: vec![resources.crtcs[index]],
                        modes: vec![mode],
                        name: format!("SOPHIA-{}", entry.output.raw()).into_bytes(),
                    })
                })
                .unwrap_or_else(|| {
                    XClientOutput::Error(crate::XClientError {
                        code: XErrorCode::BadValue,
                        sequence: context.sequence,
                        resource_id: output,
                        minor_code: crate::X_RANDR_GET_OUTPUT_INFO_MINOR_OPCODE.into(),
                        major_code: context.major_opcode,
                    })
                });
            XDispatchResult {
                response: None,
                outputs: vec![client_output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetOutputProperty {
            output,
            property,
            property_type: _,
            long_offset: _,
            long_length: _,
            delete: _,
            pending: _,
        } => {
            let resources = randr_resources(runtime.output_topology());
            let client_output =
                if resources.outputs.contains(&output) && atoms.name(property).is_some() {
                    XClientOutput::Reply(XClientReply::RandrGetOutputProperty {
                        sequence: context.sequence,
                        property_type: 0,
                        bytes_after: 0,
                        format: 0,
                        data: Vec::new(),
                    })
                } else {
                    XClientOutput::Error(crate::XClientError {
                        code: XErrorCode::BadValue,
                        sequence: context.sequence,
                        resource_id: output,
                        minor_code: crate::X_RANDR_GET_OUTPUT_PROPERTY_MINOR_OPCODE.into(),
                        major_code: context.major_opcode,
                    })
                };
            XDispatchResult {
                response: None,
                outputs: vec![client_output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetCrtcInfo { crtc, .. } => {
            let resources = randr_resources(runtime.output_topology());
            let client_output = resources
                .crtcs
                .iter()
                .position(|candidate| *candidate == crtc)
                .map(|index| {
                    let entry = &runtime.output_topology().outputs[index];
                    XClientOutput::Reply(XClientReply::RandrGetCrtcInfo {
                        sequence: context.sequence,
                        timestamp: resources.timestamp,
                        x: i16::try_from(entry.logical.x).unwrap_or(i16::MAX),
                        y: i16::try_from(entry.logical.y).unwrap_or(i16::MAX),
                        width: u16::try_from(entry.logical.width).expect("validated output width"),
                        height: u16::try_from(entry.logical.height)
                            .expect("validated output height"),
                        mode: resources.modes[index].id,
                        outputs: vec![resources.outputs[index]],
                    })
                })
                .unwrap_or_else(|| {
                    XClientOutput::Error(crate::XClientError {
                        code: XErrorCode::BadValue,
                        sequence: context.sequence,
                        resource_id: crtc,
                        minor_code: crate::X_RANDR_GET_CRTC_INFO_MINOR_OPCODE.into(),
                        major_code: context.major_opcode,
                    })
                });
            XDispatchResult {
                response: None,
                outputs: vec![client_output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetOutputPrimary { window } => {
            let resources = randr_resources(runtime.output_topology());
            let primary = runtime
                .output_topology()
                .outputs
                .iter()
                .position(|entry| entry.output == runtime.output_topology().primary)
                .map(|index| resources.outputs[index])
                .expect("validated primary output");
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetOutputPrimary {
                    sequence: context.sequence,
                    output: primary,
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::RandrGetOutputPrimary {
                    sequence: context.sequence,
                    output: primary,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetMonitors { window, .. } => {
            let timestamp = u32::try_from(runtime.output_topology().generation)
                .unwrap_or(u32::MAX)
                .max(1);
            let monitors = randr_monitors(runtime.output_topology(), atoms);
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetMonitors {
                    sequence: context.sequence,
                    timestamp,
                    monitors: monitors.clone(),
                })
            } else if let Err(error) = runtime.validate_window_access(context.namespace, window) {
                XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(window.local.raw()).unwrap_or(0),
                ))
            } else {
                XClientOutput::Reply(XClientReply::RandrGetMonitors {
                    sequence: context.sequence,
                    timestamp,
                    monitors,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XkbUseExtension { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XkbUseExtension {
                sequence: context.sequence,
                supported: true,
                server_major: 1,
                server_minor: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XkbGetMap { full, partial } => {
            let present = (full | partial) & 0x0043;
            let keysyms = runtime.xkb_keymap().xkb_keysyms();
            let modifier_map = runtime.xkb_keymap().modifier_map();
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::XkbGetMap {
                    sequence: context.sequence,
                    present,
                    keysyms,
                    modifier_map,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XkbGetState => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XkbGetState {
                sequence: context.sequence,
                modifiers: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XkbGetNames { which } => {
            let config = runtime.xkb_keymap().config();
            let layout = if config.variant.is_empty() {
                config.layout.clone()
            } else {
                format!("{}({})", config.layout, config.variant)
            };
            let components = [
                (1, config.rules.clone()),
                (2, config.model.clone()),
                (4, layout.clone()),
                (8, layout),
                (16, "complete".to_owned()),
                (32, "complete".to_owned()),
            ];
            let present = which & 0x3f;
            let atoms = components
                .iter()
                .filter(|(mask, _)| present & mask != 0)
                .filter_map(|(_, name)| atoms.intern(name, false).ok().flatten())
                .collect();
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::XkbGetNames {
                    sequence: context.sequence,
                    which: present,
                    min_keycode: runtime.xkb_keymap().min_keycode(),
                    max_keycode: runtime.xkb_keymap().max_keycode(),
                    atoms,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XkbSelectEvents { .. } => XDispatchResult {
            response: None,
            outputs: Vec::new(),
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XkbPerClientFlags { change, value } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XkbPerClientFlags {
                sequence: context.sequence,
                supported: change,
                value: value & change,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XiGetExtensionVersion => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XiGetExtensionVersion {
                sequence: context.sequence,
                server_major: 2,
                server_minor: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XiGetClientPointer => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XiGetClientPointer {
                sequence: context.sequence,
                device_id: 2,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::GeQueryVersion { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::GeQueryVersion {
                sequence: context.sequence,
                major_version: 1,
                minor_version: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XiQueryVersion { .. } => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XiQueryVersion {
                sequence: context.sequence,
                major_version: 2,
                minor_version: 0,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::XiQueryDevice { device_id } => {
            let pointer = XXiDeviceInfo {
                device_id: 2,
                device_type: 1,
                attachment: 3,
                name: "Sophia master pointer".to_owned(),
                classes: vec![
                    XXiDeviceClass::Button {
                        source_id: 2,
                        button_count: 5,
                    },
                    XXiDeviceClass::Valuator {
                        source_id: 2,
                        number: 0,
                        min: 0,
                        max: i64::from(u16::MAX) << 32,
                    },
                    XXiDeviceClass::Valuator {
                        source_id: 2,
                        number: 1,
                        min: 0,
                        max: i64::from(u16::MAX) << 32,
                    },
                ],
            };
            let keyboard = XXiDeviceInfo {
                device_id: 3,
                device_type: 2,
                attachment: 2,
                name: "Sophia master keyboard".to_owned(),
                classes: vec![XXiDeviceClass::Key {
                    source_id: 3,
                    keys: (8..=255).collect(),
                }],
            };
            let devices = match device_id {
                0 => vec![pointer, keyboard],
                1 => vec![pointer, keyboard],
                2 => vec![pointer],
                3 => vec![keyboard],
                _ => Vec::new(),
            };
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::XiQueryDevice {
                    sequence: context.sequence,
                    devices,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XiSelectEvents { window, masks } => {
            let outputs = (window.local.raw() != u64::from(X_SETUP_DEFAULT_ROOT))
                .then(|| {
                    runtime
                        .validate_window_access(context.namespace, window)
                        .err()
                })
                .flatten()
                .map(|error| {
                    XClientOutput::Error(x_error_from_runtime(
                        error,
                        context.sequence,
                        context.major_opcode,
                        u32::try_from(window.local.raw()).unwrap_or(0),
                    ))
                })
                .into_iter()
                .collect::<Vec<_>>();
            if outputs.is_empty() {
                runtime.input_authority_mut().select_xi_events(
                    context.namespace,
                    context.client_id,
                    window,
                    &masks,
                );
            }
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XiGetFocus { .. } => {
            let (focus, _) = runtime.input_focus(context.namespace);
            XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Reply(XClientReply::XiGetFocus {
                    sequence: context.sequence,
                    focus,
                })],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::XiGetProperty => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::XiGetProperty {
                sequence: context.sequence,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::BigRequestsEnable => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::BigRequestsEnable {
                sequence: context.sequence,
                maximum_request_length: 4096,
            })],
            metadata_candidates: Vec::new(),
        },
        XWireRequest::ShmAttach {
            segment,
            shmid,
            read_only,
        } => {
            let outputs = match runtime.attach_shm_segment(
                context.namespace,
                segment,
                shmid,
                read_only,
                u64::from(context.sequence),
            ) {
                Ok(()) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(segment.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ShmDetach { segment } => {
            let outputs = match runtime.detach_shm_segment(context.namespace, segment) {
                Ok(()) => Vec::new(),
                Err(
                    XAuthorityRuntimeError::InvalidResource
                    | XAuthorityRuntimeError::UnknownResource,
                ) => Vec::new(),
                Err(error) => vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(segment.local.raw()).unwrap_or(0),
                ))],
            };
            XDispatchResult {
                response: None,
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::ShmPutImage {
            drawable,
            segment,
            total_width,
            total_height,
            src_x,
            src_y,
            src_width,
            src_height,
            dst_x,
            dst_y,
            depth,
            format,
            offset,
            send_event,
            ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if runtime
                .validate_shm_segment_access(context.namespace, segment)
                .is_err()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: vec![XClientOutput::Error(crate::XClientError {
                        code: XErrorCode::BadAccess,
                        sequence: context.sequence,
                        resource_id: u32::try_from(segment.local.raw()).unwrap_or(0),
                        minor_code: 3,
                        major_code: context.major_opcode,
                    })],
                    metadata_candidates: Vec::new(),
                };
            }
            if runtime
                .validate_pixmap_access(context.namespace, drawable)
                .is_ok()
            {
                let outputs = send_event
                    .then_some(XClientOutput::Event(XClientEvent::ShmCompletion {
                        sequence: context.sequence,
                        drawable,
                        segment,
                        offset,
                    }))
                    .into_iter()
                    .collect();
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs,
                    metadata_candidates: Vec::new(),
                };
            }
            let damage = Region::single(Rect {
                x: i32::from(dst_x),
                y: i32::from(dst_y),
                width: i32::from(src_width),
                height: i32::from(src_height),
            });
            let image = runtime
                .shm_segment_shmid(context.namespace, segment)
                .ok()
                .and_then(|shmid| {
                    copy_shm_image_region(
                        shmid,
                        offset,
                        total_width,
                        total_height,
                        src_x,
                        src_y,
                        src_width,
                        src_height,
                        depth,
                        format,
                    )
                });
            let response = runtime.apply_put_image(
                transaction,
                context.namespace,
                drawable,
                damage,
                image.as_deref(),
            );
            let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(drawable.local.raw()).unwrap_or(0),
                ))]
            } else if send_event {
                vec![XClientOutput::Event(XClientEvent::ShmCompletion {
                    sequence: context.sequence,
                    drawable,
                    segment,
                    offset,
                })]
            } else {
                Vec::new()
            };
            XDispatchResult {
                response: Some(response),
                outputs,
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::PolyFillRectangle {
            drawable,
            gc,
            rectangles,
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if runtime
                .validate_pixmap_access(context.namespace, drawable)
                .is_ok()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let mut damage = Region::empty();
            for rectangle in rectangles {
                damage.push(rectangle);
            }
            let response = match runtime.graphics_context_values(context.namespace, gc) {
                Ok(values) => runtime.apply_core_draw_with_gc(
                    transaction,
                    context.namespace,
                    drawable,
                    damage,
                    &values,
                ),
                Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
            };
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
        XWireRequest::CopyArea {
            source,
            destination,
            gc,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            let response = match runtime.graphics_context_values(context.namespace, gc) {
                Ok(values) => runtime.apply_copy_area_with_gc(
                    transaction,
                    context.namespace,
                    source,
                    destination,
                    src_x,
                    src_y,
                    dst_x,
                    dst_y,
                    width,
                    height,
                    &values,
                ),
                Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
            };
            let outputs = if let XAuthorityResponseOutcome::Rejected(error) = response.outcome {
                vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(destination.local.raw()).unwrap_or(0),
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
        XWireRequest::PolyLine {
            drawable,
            gc,
            points,
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if points.len() < 2
                || runtime
                    .validate_pixmap_access(context.namespace, drawable)
                    .is_ok()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let response = match runtime.graphics_context_values(context.namespace, gc) {
                Ok(values) => runtime.apply_line_draw(
                    transaction,
                    context.namespace,
                    drawable,
                    &points,
                    &values,
                ),
                Err(error) => XAuthorityResponsePacket::rejected(transaction, error),
            };
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
        XWireRequest::PolySegment {
            drawable, damage, ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if runtime
                .validate_pixmap_access(context.namespace, drawable)
                .is_ok()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let mut region = Region::empty();
            for rect in damage {
                region.push(rect);
            }
            let response =
                runtime.apply_core_draw(transaction, context.namespace, drawable, region);
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
        XWireRequest::PolyFillArc {
            drawable, damage, ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if runtime
                .validate_pixmap_access(context.namespace, drawable)
                .is_ok()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let mut region = Region::empty();
            for rect in damage {
                region.push(rect);
            }
            let response =
                runtime.apply_core_draw(transaction, context.namespace, drawable, region);
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
        XWireRequest::PolyText8 {
            drawable,
            gc,
            x,
            y,
            text,
        } => dispatch_text_draw(context, runtime, drawable, gc, x, y, text, false),
        XWireRequest::ImageText8 {
            drawable,
            gc,
            x,
            y,
            text,
        } => dispatch_text_draw(context, runtime, drawable, gc, x, y, text, true),
        XWireRequest::FillPoly {
            drawable, damage, ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if damage.is_none()
                || runtime
                    .validate_pixmap_access(context.namespace, drawable)
                    .is_ok()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let response = runtime.apply_core_draw(
                transaction,
                context.namespace,
                drawable,
                Region::single(damage.unwrap()),
            );
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
            data,
            ..
        } => {
            let transaction = TransactionId::from_raw(u64::from(context.sequence));
            if runtime
                .validate_pixmap_access(context.namespace, drawable)
                .is_ok()
            {
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let damage = Region::single(Rect {
                x: i32::from(dst_x),
                y: i32::from(dst_y),
                width: i32::from(width),
                height: i32::from(height),
            });
            let response = runtime.apply_put_image(
                transaction,
                context.namespace,
                drawable,
                damage,
                Some(&data),
            );
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

fn dispatch_text_draw(
    context: XDispatchContext,
    runtime: &mut XAuthorityRuntime,
    drawable: XResourceId,
    gc: XResourceId,
    x: i16,
    y: i16,
    text: Vec<u8>,
    opaque: bool,
) -> XDispatchResult {
    let transaction = TransactionId::from_raw(u64::from(context.sequence));
    if runtime
        .validate_pixmap_access(context.namespace, drawable)
        .is_ok()
    {
        return XDispatchResult {
            response: Some(XAuthorityResponsePacket::accepted(transaction)),
            outputs: Vec::new(),
            metadata_candidates: Vec::new(),
        };
    }
    let gc_values = match runtime.graphics_context_values(context.namespace, gc) {
        Ok(values) => values,
        Err(error) => {
            return XDispatchResult {
                response: None,
                outputs: vec![XClientOutput::Error(x_error_from_runtime(
                    error,
                    context.sequence,
                    context.major_opcode,
                    u32::try_from(gc.local.raw()).unwrap_or(0),
                ))],
                metadata_candidates: Vec::new(),
            };
        }
    };
    let response = runtime.apply_text_draw(
        transaction,
        context.namespace,
        drawable,
        x,
        y,
        &text,
        opaque,
        &gc_values,
    );
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

fn validate_grab_window(
    runtime: &XAuthorityRuntime,
    namespace: NamespaceId,
    window: XResourceId,
) -> Result<(), XAuthorityRuntimeError> {
    if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
        Ok(())
    } else {
        runtime.validate_window_access(namespace, window)
    }
}

fn grab_access_error(context: &XDispatchContext, window: XResourceId) -> XClientOutput {
    XClientOutput::Error(crate::XClientError {
        code: XErrorCode::BadAccess,
        sequence: context.sequence,
        resource_id: u32::try_from(window.local.raw()).unwrap_or(0),
        minor_code: 0,
        major_code: context.major_opcode,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XExtensionQueryResult {
    present: bool,
    major_opcode: u8,
    first_event: u8,
    first_error: u8,
}

fn extension_query_result(name: &str) -> XExtensionQueryResult {
    match name {
        X_SOPHIA_PRESENT_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: X_SOPHIA_PRESENT_MAJOR_OPCODE,
            first_event: 0,
            first_error: 0,
        },
        X_MIT_SHM_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: X_MIT_SHM_MAJOR_OPCODE,
            first_event: crate::X_MIT_SHM_FIRST_EVENT,
            first_error: 0,
        },
        crate::X_DRI3_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: crate::X_DRI3_MAJOR_OPCODE,
            first_event: 0,
            first_error: 0,
        },
        crate::X_PRESENT_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: crate::X_PRESENT_MAJOR_OPCODE,
            first_event: crate::X_PRESENT_FIRST_EVENT,
            first_error: 0,
        },
        crate::X_XFIXES_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: crate::X_XFIXES_MAJOR_OPCODE,
            first_event: 0,
            first_error: 0,
        },
        X_RANDR_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: X_RANDR_MAJOR_OPCODE,
            first_event: crate::X_RANDR_FIRST_EVENT,
            first_error: 0,
        },
        crate::X_KEYBOARD_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: crate::X_KEYBOARD_MAJOR_OPCODE,
            first_event: crate::X_KEYBOARD_FIRST_EVENT,
            first_error: 0,
        },
        crate::X_INPUT_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: crate::X_INPUT_MAJOR_OPCODE,
            first_event: crate::X_INPUT_FIRST_EVENT,
            first_error: crate::X_INPUT_FIRST_ERROR,
        },
        crate::X_GENERIC_EVENT_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: crate::X_GENERIC_EVENT_MAJOR_OPCODE,
            first_event: 0,
            first_error: 0,
        },
        X_BIG_REQUESTS_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: X_BIG_REQUESTS_MAJOR_OPCODE,
            first_event: 0,
            first_error: 0,
        },
        _ => XExtensionQueryResult {
            present: false,
            major_opcode: 0,
            first_event: 0,
            first_error: 0,
        },
    }
}

fn copy_shm_image_region(
    shmid: u32,
    offset: u32,
    total_width: u16,
    total_height: u16,
    src_x: u16,
    src_y: u16,
    src_width: u16,
    src_height: u16,
    depth: u8,
    format: u8,
) -> Option<Vec<u8>> {
    const Z_PIXMAP: u8 = 2;
    const BYTES_PER_PIXEL: usize = 4;
    const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
    if format != Z_PIXMAP || !matches!(depth, 24 | 32) {
        return None;
    }
    let total_width = usize::from(total_width);
    let total_height = usize::from(total_height);
    let src_x = usize::from(src_x);
    let src_y = usize::from(src_y);
    let src_width = usize::from(src_width);
    let src_height = usize::from(src_height);
    if src_x.checked_add(src_width)? > total_width || src_y.checked_add(src_height)? > total_height
    {
        return None;
    }
    let stride = total_width.checked_mul(BYTES_PER_PIXEL)?;
    let total_len = stride.checked_mul(total_height)?;
    if total_len > MAX_IMAGE_BYTES {
        return None;
    }
    let source =
        sophia_sysv_shm::copy_bytes(shmid, usize::try_from(offset).ok()?, total_len).ok()?;
    let row_len = src_width.checked_mul(BYTES_PER_PIXEL)?;
    let mut image = Vec::with_capacity(row_len.checked_mul(src_height)?);
    for row in src_y..src_y.checked_add(src_height)? {
        let start = row
            .checked_mul(stride)?
            .checked_add(src_x.checked_mul(BYTES_PER_PIXEL)?)?;
        image.extend_from_slice(source.get(start..start.checked_add(row_len)?)?);
    }
    Some(image)
}

#[derive(Clone, Debug)]
struct XRandrResources {
    timestamp: u32,
    crtcs: Vec<u32>,
    outputs: Vec<u32>,
    modes: Vec<XRandrModeInfo>,
}

fn randr_resources(snapshot: &OutputTopologySnapshot) -> XRandrResources {
    let timestamp = u32::try_from(snapshot.generation)
        .unwrap_or(u32::MAX)
        .max(1);
    let mut crtcs = Vec::with_capacity(snapshot.outputs.len());
    let mut outputs = Vec::with_capacity(snapshot.outputs.len());
    let mut modes = Vec::with_capacity(snapshot.outputs.len());
    for entry in &snapshot.outputs {
        // Output identity is Engine-owned and survives topology reordering.
        // The protocol caps the topology at 16 entries; folding the opaque ID
        // keeps it outside client resource ranges while remaining stable.
        let identity = stable_randr_identity(entry.output.raw());
        let crtc = 0x1000_0000 | identity;
        let output = 0x2000_0000 | identity;
        let mode = stable_randr_mode_id(
            entry.logical.width,
            entry.logical.height,
            entry.refresh_millihz,
        );
        crtcs.push(crtc);
        outputs.push(output);
        modes.push(XRandrModeInfo {
            id: mode,
            width: u16::try_from(entry.logical.width).expect("validated output width"),
            height: u16::try_from(entry.logical.height).expect("validated output height"),
            refresh_millihz: entry.refresh_millihz,
            name: format!(
                "{}x{}@{}",
                entry.logical.width,
                entry.logical.height,
                entry.refresh_millihz / 1_000
            )
            .into_bytes(),
        });
    }
    XRandrResources {
        timestamp,
        crtcs,
        outputs,
        modes,
    }
}

pub(crate) fn stable_randr_identity(raw: u64) -> u32 {
    let folded = raw ^ (raw >> 32);
    (u32::try_from(folded & 0x0fff_ffff).unwrap_or(0)).max(1)
}

pub(crate) fn stable_randr_mode_id(width: i32, height: i32, refresh_millihz: u32) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for value in [width as u32, height as u32, refresh_millihz] {
        hash ^= value;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    0x3000_0000 | (hash & 0x0fff_ffff).max(1)
}

fn logical_pixels_to_millimeters(pixels: i32) -> u32 {
    u32::try_from(i64::from(pixels).saturating_mul(254).saturating_add(480) / 960)
        .unwrap_or(u32::MAX)
        .max(1)
}

fn randr_monitors(
    snapshot: &OutputTopologySnapshot,
    atoms: &mut XAtomTable,
) -> Vec<XRandrMonitorInfo> {
    snapshot
        .outputs
        .iter()
        .map(|entry| {
            let name = atoms
                .intern(&format!("SOPHIA-{}", entry.output.raw()), false)
                .ok()
                .flatten()
                .unwrap_or(X_ATOM_NONE);
            XRandrMonitorInfo {
                name,
                primary: entry.output == snapshot.primary,
                x: i16::try_from(entry.logical.x).unwrap_or(i16::MAX),
                y: i16::try_from(entry.logical.y).unwrap_or(i16::MAX),
                width: u16::try_from(entry.logical.width).unwrap_or(u16::MAX),
                height: u16::try_from(entry.logical.height).unwrap_or(u16::MAX),
                mm_width: logical_pixels_to_millimeters(entry.logical.width),
                mm_height: logical_pixels_to_millimeters(entry.logical.height),
                outputs: vec![0x2000_0000 | stable_randr_identity(entry.output.raw())],
            }
        })
        .collect()
}

pub fn dispatch_x11_parse_error(
    context: XDispatchContext,
    minor_code: u16,
    error: XWireParseError,
) -> XDispatchResult {
    XDispatchResult {
        response: None,
        outputs: vec![XClientOutput::Error(x_error_from_wire_parse(
            &error,
            context.sequence,
            context.major_opcode,
            minor_code,
        ))],
        metadata_candidates: Vec::new(),
    }
}

fn outputs_from_authority_response(
    context: XDispatchContext,
    kind: &XAuthorityRequestKind,
    response: &XAuthorityResponsePacket,
) -> Vec<XClientOutput> {
    if let Some(crate::XAuthoritySelectionArtifact::Clear {
        owner,
        selection,
        time,
    }) = response.selection_artifacts.first()
    {
        return vec![XClientOutput::Event(XClientEvent::SelectionClear {
            sequence: context.sequence,
            time: *time,
            owner: *owner,
            selection: *selection,
        })];
    }
    if let XAuthorityRequestKind::RequestSelection {
        requestor,
        selection,
        target,
        time,
        ..
    } = kind
    {
        if let Some(artifact) = response.selection_artifacts.first() {
            return vec![XClientOutput::Event(match artifact {
                crate::XAuthoritySelectionArtifact::Failure(_) => x_selection_failure_event(
                    context.sequence,
                    *time,
                    *requestor,
                    *selection,
                    *target,
                ),
                crate::XAuthoritySelectionArtifact::Request(request) => {
                    XClientEvent::SelectionRequest {
                        sequence: context.sequence,
                        time: request.time,
                        owner: request.owner,
                        requestor: request.requestor,
                        selection: request.selection,
                        target: request.target,
                        property: request.property,
                    }
                }
                crate::XAuthoritySelectionArtifact::Clear {
                    owner,
                    selection,
                    time,
                } => XClientEvent::SelectionClear {
                    sequence: context.sequence,
                    time: *time,
                    owner: *owner,
                    selection: *selection,
                },
            })];
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
            let mut outputs = vec![XClientOutput::Event(XClientEvent::MapNotify {
                sequence: context.sequence,
                event: *window,
                window: *window,
                override_redirect: false,
            })];
            if let Some(surface) = response.surfaces.iter().find(|surface| surface.mapped) {
                outputs.push(XClientOutput::Event(XClientEvent::Expose {
                    sequence: context.sequence,
                    window: *window,
                    x: 0,
                    y: 0,
                    width: clamp_u16(surface.geometry.width),
                    height: clamp_u16(surface.geometry.height),
                    count: 0,
                }));
            }
            outputs
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

fn true_color_pixel_from_rgb16(red: u16, green: u16, blue: u16) -> u32 {
    (u32::from(red & 0xff00) << 8) | u32::from(green & 0xff00) | (u32::from(blue) >> 8)
}

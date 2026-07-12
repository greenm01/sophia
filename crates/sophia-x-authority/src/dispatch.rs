use crate::{
    X_BIG_REQUESTS_EXTENSION_NAME, X_BIG_REQUESTS_MAJOR_OPCODE, X_MIT_SHM_EXTENSION_NAME,
    X_MIT_SHM_MAJOR_OPCODE, X_RANDR_EXTENSION_NAME, X_RANDR_MAJOR_OPCODE, X_SETUP_DEFAULT_COLORMAP,
    X_SETUP_DEFAULT_ROOT, X_SETUP_DEFAULT_VISUAL, X_SETUP_ROOT_HEIGHT, X_SETUP_ROOT_WIDTH,
    X_SOPHIA_PRESENT_EXTENSION_NAME, X_SOPHIA_PRESENT_MAJOR_OPCODE, XAtomTable,
    XAuthorityRequestKind, XAuthorityResponseOutcome, XAuthorityResponsePacket, XAuthorityRuntime,
    XAuthorityRuntimeError, XByteOrder, XClientEvent, XClientOutput, XClientReply, XErrorCode,
    XMetadataPropertyCandidate, XPropertyError, XPropertyTable, XResourceId, XWireParseError,
    XWireRequest, encode_x_client_output, metadata_property_candidate, x_error_from_runtime,
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
    runtime.begin_dispatch();
    match request {
        XWireRequest::CreateWindow {
            packet,
            background_pixel,
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
        XWireRequest::ChangeWindowAttributes { window } => {
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
                        width: i32::from(X_SETUP_ROOT_WIDTH),
                        height: i32::from(X_SETUP_ROOT_HEIGHT),
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
        XWireRequest::GrabButton { window, .. } | XWireRequest::UngrabButton { window, .. } => {
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
        XWireRequest::GrabServer | XWireRequest::UngrabServer => XDispatchResult {
            response: None,
            outputs: Vec::new(),
            metadata_candidates: Vec::new(),
        },
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
                Ok(()) => Vec::new(),
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
        XWireRequest::GetInputFocus => XDispatchResult {
            response: None,
            outputs: vec![XClientOutput::Reply(XClientReply::GetInputFocus {
                sequence: context.sequence,
                focus: XResourceId::new(u64::from(crate::X_SETUP_DEFAULT_ROOT), 1),
                revert_to: 1,
            })],
            metadata_candidates: Vec::new(),
        },
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
                keysyms: x11_us_keyboard_mapping(first_keycode, count),
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
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetScreenSizeRange {
                    sequence: context.sequence,
                    min_width: X_SETUP_ROOT_WIDTH,
                    min_height: X_SETUP_ROOT_HEIGHT,
                    max_width: X_SETUP_ROOT_WIDTH,
                    max_height: X_SETUP_ROOT_HEIGHT,
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
                    min_width: X_SETUP_ROOT_WIDTH,
                    min_height: X_SETUP_ROOT_HEIGHT,
                    max_width: X_SETUP_ROOT_WIDTH,
                    max_height: X_SETUP_ROOT_HEIGHT,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetScreenResources { window, .. } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetScreenResources {
                    sequence: context.sequence,
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
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetOutputPrimary { window } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetOutputPrimary {
                    sequence: context.sequence,
                    output: 0,
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
                    output: 0,
                })
            };
            XDispatchResult {
                response: None,
                outputs: vec![output],
                metadata_candidates: Vec::new(),
            }
        }
        XWireRequest::RandrGetMonitors { window, .. } => {
            let output = if window.local.raw() == u64::from(X_SETUP_DEFAULT_ROOT) {
                XClientOutput::Reply(XClientReply::RandrGetMonitors {
                    sequence: context.sequence,
                    timestamp: 0,
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
                    timestamp: 0,
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
            src_width,
            src_height,
            dst_x,
            dst_y,
            offset: _,
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
                return XDispatchResult {
                    response: Some(XAuthorityResponsePacket::accepted(transaction)),
                    outputs: Vec::new(),
                    metadata_candidates: Vec::new(),
                };
            }
            let damage = Region::single(Rect {
                x: i32::from(dst_x),
                y: i32::from(dst_y),
                width: i32::from(src_width),
                height: i32::from(src_height),
            });
            let response =
                runtime.apply_put_image(transaction, context.namespace, drawable, damage, None);
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

fn x11_us_keyboard_mapping(first_keycode: u8, count: u8) -> Vec<u32> {
    let mut keysyms = Vec::with_capacity(usize::from(count) * 2);
    for offset in 0..count {
        let keycode = first_keycode.saturating_add(offset);
        let (base, shifted) = x11_us_keysyms(keycode);
        keysyms.push(base);
        keysyms.push(shifted);
    }
    keysyms
}

fn x11_us_keysyms(keycode: u8) -> (u32, u32) {
    match keycode {
        9 => (0xff1b, 0xff1b),
        10..=18 => {
            const BASE: &[u8; 9] = b"123456789";
            const SHIFTED: &[u8; 9] = b"!@#$%^&*(";
            let index = usize::from(keycode - 10);
            (u32::from(BASE[index]), u32::from(SHIFTED[index]))
        }
        19 => (u32::from(b'0'), u32::from(b')')),
        20 => (u32::from(b'-'), u32::from(b'_')),
        21 => (u32::from(b'='), u32::from(b'+')),
        22 => (0xff08, 0xff08),
        23 => (0xff09, 0xfe20),
        24..=33 => letter_keysyms(keycode, 24, b"qwertyuiop"),
        34 => (u32::from(b'['), u32::from(b'{')),
        35 => (u32::from(b']'), u32::from(b'}')),
        36 => (0xff0d, 0xff0d),
        37 => (0xffe3, 0xffe3),
        38..=46 => letter_keysyms(keycode, 38, b"asdfghjkl"),
        47 => (u32::from(b';'), u32::from(b':')),
        48 => (u32::from(b'\''), u32::from(b'\"')),
        49 => (u32::from(b'`'), u32::from(b'~')),
        50 | 62 => (0xffe1, 0xffe2),
        51 => (u32::from(b'\\'), u32::from(b'|')),
        52..=58 => letter_keysyms(keycode, 52, b"zxcvbnm"),
        59 => (u32::from(b','), u32::from(b'<')),
        60 => (u32::from(b'.'), u32::from(b'>')),
        61 => (u32::from(b'/'), u32::from(b'?')),
        65 => (u32::from(b' '), u32::from(b' ')),
        66 => (0xffe5, 0xffe5),
        110 => (0xff50, 0xff50),
        111 => (0xff52, 0xff52),
        112 => (0xff55, 0xff55),
        113 => (0xff51, 0xff51),
        114 => (0xff53, 0xff53),
        115 => (0xff57, 0xff57),
        116 => (0xff54, 0xff54),
        117 => (0xff56, 0xff56),
        118 => (0xff63, 0xff63),
        119 => (0xffff, 0xffff),
        _ => (0, 0),
    }
}

fn letter_keysyms(keycode: u8, first: u8, letters: &[u8]) -> (u32, u32) {
    let letter = letters[usize::from(keycode - first)];
    (u32::from(letter), u32::from(letter.to_ascii_uppercase()))
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
            first_event: 0,
            first_error: 0,
        },
        X_RANDR_EXTENSION_NAME => XExtensionQueryResult {
            present: true,
            major_opcode: X_RANDR_MAJOR_OPCODE,
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

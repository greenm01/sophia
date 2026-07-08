use sophia_protocol::*;

#[test]
fn simple_ids_start_after_zero() {
    let mut alloc = IdAllocator::<NamespaceId>::new();
    let first = alloc.next_id();
    let second = alloc.next_id();

    assert!(first.is_valid());
    assert_eq!(first.raw(), 1);
    assert_eq!(second.raw(), 2);
}

#[test]
fn foreign_xids_keep_generation() {
    let id = XWindowId::new(0x1200042, 7);

    assert!(id.is_valid());
    assert_eq!(id.xid(), 0x1200042);
    assert_eq!(id.generation(), 7);
}

#[test]
fn region_drops_empty_rectangles() {
    let mut region = Region::empty();
    region.push(Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 10,
    });
    region.push(Rect {
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    });

    assert_eq!(region.rects.len(), 1);
}

#[test]
fn stale_surface_id_fails_closed() {
    let mut table = SurfaceTable::new();
    let first = table.insert("first");

    assert_eq!(table.remove(first), Ok("first"));

    let second = table.insert("second");

    assert_ne!(first, second);
    assert_eq!(table.get(first), None);
    assert_eq!(table.get(second), Some(&"second"));
}

#[test]
fn layer_snapshot_is_cloneable_frame_data() {
    let surface = SurfaceId::new(0, 1);
    let snapshot = LayerSnapshot {
        surface,
        window: Some(XWindowId::new(42, 1)),
        namespace: Some(NamespaceId::from_raw(1)),
        stack_rank: 0,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 640,
            height: 480,
        },
        source: BufferSource::XPixmap { pixmap: 99 },
        damage: Region::single(Rect {
            x: 10,
            y: 20,
            width: 10,
            height: 10,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 3,
    };

    let cloned = snapshot.clone();

    assert_eq!(cloned.surface, surface);
    assert_eq!(cloned.damage.rects.len(), 1);
}

#[test]
fn layout_node_snapshot_carries_only_opaque_policy_data() {
    let node = LayoutNodeSnapshot {
        surface: SurfaceId::new(7, 1),
        workspace: WorkspaceId::from_raw(2),
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 320,
                height: 200,
            }),
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        generation: 3,
    };

    assert_eq!(node.surface, SurfaceId::new(7, 1));
    assert_eq!(node.workspace, WorkspaceId::from_raw(2));
    assert!(node.capabilities.resizable);
    assert!(node.state.visible);
}

#[test]
fn chrome_descriptor_carries_redacted_metadata_separately() {
    let chrome = ChromeDescriptor {
        surface: SurfaceId::new(9, 1),
        label: Some(DisplayLabel {
            text: "Private Window".to_owned(),
            redacted: true,
        }),
        icon: Some(IconTokenId::from_raw(4)),
        trust_level: TrustLevel::Untrusted,
        attention: AttentionState::Notice,
        generation: 1,
    };

    assert_eq!(chrome.surface, SurfaceId::new(9, 1));
    assert_eq!(
        chrome.label.as_ref().map(|label| label.redacted),
        Some(true)
    );
    assert_eq!(chrome.icon, Some(IconTokenId::from_raw(4)));
}

#[test]
fn broker_health_packet_accepts_bounded_status_message() {
    let packet = BrokerHealthPacket::new(
        BrokerKind::Portal,
        BrokerHealthState::Ready,
        3,
        Some("ready".to_owned()),
    )
    .unwrap();

    assert_eq!(packet.broker, BrokerKind::Portal);
    assert_eq!(packet.state, BrokerHealthState::Ready);
    assert_eq!(packet.generation, 3);
    assert_eq!(packet.message.as_deref(), Some("ready"));
}

#[test]
fn broker_health_packet_accepts_empty_status_message() {
    let packet =
        BrokerHealthPacket::new(BrokerKind::Metadata, BrokerHealthState::Starting, 1, None)
            .unwrap();

    assert_eq!(packet.broker, BrokerKind::Metadata);
    assert_eq!(packet.state, BrokerHealthState::Starting);
    assert_eq!(packet.message, None);
}

#[test]
fn broker_health_packet_rejects_unbounded_status_message() {
    let message = "x".repeat(SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1);

    assert_eq!(
        BrokerHealthPacket::new(
            BrokerKind::Portal,
            BrokerHealthState::Degraded,
            4,
            Some(message)
        ),
        Err(BrokerHealthError::MessageTooLong {
            len: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1,
            max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
        })
    );
}

#[test]
fn chrome_action_request_is_surface_scoped() {
    let request = ChromeActionRequest {
        surface: SurfaceId::new(9, 4),
        generation: 12,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    assert_eq!(request.surface, SurfaceId::new(9, 4));
    assert_eq!(request.generation, 12);
    assert_eq!(request.kind, ChromeActionKind::CloseSurfaceRequested);
}

#[test]
fn wm_manage_request_contains_only_blind_policy_data() {
    let surface = SurfaceId::new(2, 1);
    let workspace = WorkspaceId::from_raw(1);
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(5),
        kind: WmRequestKind::ManageSurface(WmManageSurface {
            node: layout_node(surface, workspace),
            output: OutputId::from_raw(1),
            workspace,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
        }),
    };

    assert_eq!(request.transaction, TransactionId::from_raw(5));
    let WmRequestKind::ManageSurface(manage) = request.kind else {
        panic!("expected manage request");
    };
    assert_eq!(manage.node.surface, surface);
    assert_eq!(manage.workspace, workspace);
}

#[test]
fn wm_response_converts_to_layout_transaction() {
    let surface = SurfaceId::new(2, 1);
    let workspace = WorkspaceId::from_raw(1);
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(5),
        commands: vec![
            WmCommand::AssignWorkspace { surface, workspace },
            WmCommand::ConfigureSurface(SurfaceSizeRequest {
                surface,
                size: Size {
                    width: 640,
                    height: 480,
                },
            }),
            WmCommand::FocusSurface(surface),
            WmCommand::RenderSurface(SurfacePlacement {
                surface,
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
                z_index: 3,
                crop: None,
                transform: Transform::IDENTITY,
            }),
        ],
        timeout_msec: 250,
    };

    let transaction = response.into_layout_transaction();

    assert_eq!(transaction.transaction, TransactionId::from_raw(5));
    assert_eq!(transaction.requested_sizes.len(), 1);
    assert_eq!(transaction.focus, Some(surface));
    assert_eq!(transaction.render_positions.len(), 1);
    assert_eq!(transaction.render_positions[0].z_index, 3);
    assert_eq!(transaction.timeout_msec, 250);
}

#[test]
fn xlibre_routed_input_request_is_targeted_but_not_direct_delivery() {
    let request = XLibreRoutedInputRequest {
        serial: 99,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1_000,
        target_window: XWindowId::new(0x42, 1),
        local_position: Point { x: 12.5, y: 9.0 },
        kind: InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
    };

    assert_eq!(request.serial, 99);
    assert_eq!(request.target_window, XWindowId::new(0x42, 1));
    assert_eq!(request.local_position.x, 12.5);
    assert_eq!(request.device, DeviceId::from_raw(2));
    assert_eq!(
        request.kind,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        }
    );
}

#[test]
fn xlibre_routed_input_decision_carries_server_side_rejection() {
    let decision = XLibreRoutedInputDecision {
        serial: 100,
        target_window: XWindowId::new(0x55, 3),
        outcome: XLibreRoutedInputOutcome::RejectedDeniedNamespace,
    };

    assert_eq!(decision.serial, 100);
    assert_eq!(
        decision.outcome,
        XLibreRoutedInputOutcome::RejectedDeniedNamespace
    );
}

#[test]
fn xlibre_routed_input_request_has_stable_wire_shape() {
    let request = XLibreRoutedInputRequest {
        serial: 0x0000_0001_0000_0002,
        seat: SeatId::from_raw(3),
        device: DeviceId::from_raw(4),
        time_msec: 5,
        target_window: XWindowId::new(0x1200_0042, 1),
        local_position: Point { x: 12.5, y: 9.25 },
        kind: InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
    };

    let wire = request.to_wire_request();

    assert_eq!(XLIBRE_ROUTED_INPUT_EXTENSION_NAME, "SOPHIA-ROUTED-INPUT");
    assert_eq!(XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE, 1);
    assert_eq!(XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH, 11);
    assert_eq!(wire.serial_hi, 1);
    assert_eq!(wire.serial_lo, 2);
    assert_eq!(wire.target_xid, 0x1200_0042);
    assert_eq!(wire.seat, 3);
    assert_eq!(wire.device, 4);
    assert_eq!(wire.local_x_24_8, 3200);
    assert_eq!(wire.local_y_24_8, 2368);
    assert_eq!(wire.event_code, 2);
    assert_eq!(wire.detail, 1);
    assert_eq!(wire.flags, 1);
}

#[test]
fn xlibre_routed_input_wire_request_decodes_to_packet() {
    let wire = XLibreRoutedInputWireRequest {
        serial_hi: 7,
        serial_lo: 8,
        target_xid: 0x44,
        seat: 1,
        device: 2,
        time_msec: 10,
        local_x_24_8: 512,
        local_y_24_8: 768,
        event_code: 1,
        detail: 0,
        flags: 0,
    };

    let request = wire.to_request().unwrap();

    assert_eq!(request.serial, 0x0000_0007_0000_0008);
    assert_eq!(request.target_window, XWindowId::new(0x44, 1));
    assert_eq!(request.local_position, Point { x: 2.0, y: 3.0 });
    assert_eq!(request.kind, InputEventKind::PointerMotion);
}

#[test]
fn xlibre_routed_input_wire_request_rejects_unknown_event_code() {
    let wire = XLibreRoutedInputWireRequest {
        serial_hi: 0,
        serial_lo: 1,
        target_xid: 0x44,
        seat: 1,
        device: 2,
        time_msec: 10,
        local_x_24_8: 0,
        local_y_24_8: 0,
        event_code: 99,
        detail: 0,
        flags: 0,
    };

    assert_eq!(
        wire.to_request(),
        Err(XLibreRoutedInputWireError::UnsupportedEventCode)
    );
}

#[test]
fn wm_request_frame_roundtrips() {
    let request = WmRequestPacket {
        transaction: TransactionId::from_raw(42),
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(7),
            workspace: WorkspaceId::from_raw(3),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            nodes: vec![node(1), node(2)],
        }),
    };

    let frame = encode_wm_request_frame(&request).unwrap();
    assert_eq!(
        frame.len(),
        SOPHIA_IPC_HEADER_LEN + frame_payload_len(&frame)
    );
    assert_eq!(decode_wm_request_frame(&frame), Ok(request));
}

#[test]
fn wm_response_frame_roundtrips() {
    let surface = SurfaceId::new(4, 9);
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(77),
        timeout_msec: 250,
        commands: vec![
            WmCommand::AssignWorkspace {
                surface,
                workspace: WorkspaceId::from_raw(5),
            },
            WmCommand::ConfigureSurface(SurfaceSizeRequest {
                surface,
                size: Size {
                    width: 640,
                    height: 480,
                },
            }),
            WmCommand::FocusSurface(surface),
            WmCommand::RenderSurface(SurfacePlacement {
                surface,
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
                z_index: 2,
                crop: Some(Rect {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 240,
                }),
                transform: Transform::IDENTITY,
            }),
        ],
    };

    let frame = encode_wm_response_frame(&response).unwrap();
    assert_eq!(decode_wm_response_frame(&frame), Ok(response));
}

#[test]
fn oversized_payload_is_rejected_before_allocation() {
    let mut frame = Vec::new();
    push_u32(&mut frame, SOPHIA_IPC_MAGIC);
    push_u16(&mut frame, SOPHIA_IPC_VERSION);
    push_u16(&mut frame, IpcMessageKind::WmRequest as u16);
    push_u64(&mut frame, 1);
    push_u32(&mut frame, (SOPHIA_IPC_MAX_PAYLOAD_LEN as u32) + 1);
    push_u32(&mut frame, 0);

    assert_eq!(
        decode_frame(&frame),
        Err(IpcCodecError::PayloadTooLarge(
            SOPHIA_IPC_MAX_PAYLOAD_LEN + 1
        ))
    );
}

#[test]
fn malformed_frames_fail_closed() {
    assert_eq!(decode_frame(&[]), Err(IpcCodecError::Truncated));

    let mut frame = encode_wm_request_frame(&WmRequestPacket {
        transaction: TransactionId::from_raw(1),
        kind: WmRequestKind::SurfaceRemoved {
            surface: SurfaceId::new(1, 1),
            workspace: WorkspaceId::from_raw(1),
        },
    })
    .unwrap();
    frame[0] = 0;
    assert_eq!(decode_frame(&frame), Err(IpcCodecError::BadMagic));

    let mut frame = encode_wm_request_frame(&WmRequestPacket {
        transaction: TransactionId::from_raw(1),
        kind: WmRequestKind::SurfaceRemoved {
            surface: SurfaceId::new(1, 1),
            workspace: WorkspaceId::from_raw(1),
        },
    })
    .unwrap();
    frame.push(0);
    assert_eq!(decode_frame(&frame), Err(IpcCodecError::TrailingBytes(1)));
}

#[test]
fn excessive_item_count_is_rejected() {
    let mut payload = Vec::new();
    push_u16(&mut payload, 2);
    push_u64(&mut payload, 1);
    push_u64(&mut payload, 1);
    encode_rect(
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        &mut payload,
    );
    push_u32(&mut payload, (SOPHIA_IPC_MAX_ITEMS as u32) + 1);
    let frame = encode_frame(
        IpcMessageKind::WmRequest,
        TransactionId::from_raw(9),
        &payload,
    )
    .unwrap();

    assert_eq!(
        decode_wm_request_frame(&frame),
        Err(IpcCodecError::CountTooLarge {
            count: SOPHIA_IPC_MAX_ITEMS + 1,
            max: SOPHIA_IPC_MAX_ITEMS,
        })
    );
}

fn layout_node(surface: SurfaceId, workspace: WorkspaceId) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        },
        generation: 1,
    }
}

fn node(index: u32) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(index, 1),
        workspace: WorkspaceId::from_raw(3),
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        geometry: Rect {
            x: (index as i32) * 10,
            y: 0,
            width: 320,
            height: 200,
        },
        generation: 11,
    }
}

fn frame_payload_len(frame: &[u8]) -> usize {
    u32::from_le_bytes(frame[16..20].try_into().unwrap()) as usize
}

fn encode_rect(rect: Rect, out: &mut Vec<u8>) {
    push_i32(out, rect.x);
    push_i32(out, rect.y);
    push_i32(out, rect.width);
    push_i32(out, rect.height);
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

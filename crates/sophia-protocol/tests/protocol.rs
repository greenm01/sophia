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
fn buffer_and_fence_handles_are_typed_and_nonzero() {
    let mut buffers = IdAllocator::<BufferHandle>::new();
    let mut fences = IdAllocator::<FenceHandle>::new();

    assert_eq!(buffers.next_id().raw(), 1);
    assert_eq!(fences.next_id().raw(), 1);
    assert!(!BufferHandle::INVALID.is_valid());
    assert!(!FenceHandle::INVALID.is_valid());
}

#[test]
fn dma_buf_descriptor_validation_is_bounded() {
    let valid = DmaBufDescriptor {
        handle: BufferHandle::from_raw(7),
        size: Size {
            width: 640,
            height: 480,
        },
        format: DRM_FORMAT_XRGB8888,
        modifier: DRM_FORMAT_MOD_INVALID,
        plane_count: 1,
        planes: [
            Some(DmaBufPlaneDescriptor {
                offset: 0,
                stride: 2560,
            }),
            None,
            None,
            None,
        ],
    };
    assert_eq!(valid.validate(), Ok(()));

    assert_eq!(
        DmaBufDescriptor {
            plane_count: 0,
            ..valid
        }
        .validate(),
        Err(DmaBufDescriptorError::InvalidPlaneCount)
    );
    assert_eq!(
        DmaBufDescriptor {
            planes: [
                Some(DmaBufPlaneDescriptor {
                    offset: 0,
                    stride: 64,
                }),
                None,
                None,
                None,
            ],
            ..valid
        }
        .validate(),
        Err(DmaBufDescriptorError::InvalidStride)
    );
    assert_eq!(
        DmaBufDescriptor {
            size: Size {
                width: DMA_BUF_MAX_DIMENSION + 1,
                height: 1,
            },
            ..valid
        }
        .validate(),
        Err(DmaBufDescriptorError::InvalidSize)
    );
}

#[test]
fn namespace_capabilities_are_directional_and_bounded() {
    let capabilities = NamespaceCapabilities::NONE
        .with_request(NamespacePortalCapability::Clipboard)
        .with_publish(NamespacePortalCapability::Notification);

    assert!(capabilities.allows_request(NamespacePortalCapability::Clipboard));
    assert!(!capabilities.allows_publish(NamespacePortalCapability::Clipboard));
    assert!(capabilities.allows_publish(NamespacePortalCapability::Notification));
    assert!(!capabilities.allows_request(NamespacePortalCapability::Notification));
    assert_eq!(
        NamespaceCapabilities::from_bits(capabilities.request_bits(), capabilities.publish_bits()),
        Some(capabilities)
    );
    assert_eq!(NamespaceCapabilities::from_bits(1 << 63, 0), None);
}

#[test]
fn every_portal_kind_maps_to_its_explicit_namespace_capability() {
    let mappings = [
        (
            PortalTransferKind::Clipboard,
            NamespacePortalCapability::Clipboard,
        ),
        (
            PortalTransferKind::DragAndDrop,
            NamespacePortalCapability::DragAndDrop,
        ),
        (
            PortalTransferKind::FileHandoff,
            NamespacePortalCapability::FileHandoff,
        ),
        (
            PortalTransferKind::ScreenCapture,
            NamespacePortalCapability::ScreenCapture,
        ),
        (
            PortalTransferKind::ScreenRecording,
            NamespacePortalCapability::ScreenRecording,
        ),
        (
            PortalTransferKind::UriOpen,
            NamespacePortalCapability::UriOpen,
        ),
        (
            PortalTransferKind::Notification,
            NamespacePortalCapability::Notification,
        ),
    ];

    for (kind, capability) in mappings {
        assert_eq!(kind.capability(), capability);
    }
}

#[test]
fn namespace_and_admission_contexts_reject_invalid_identity() {
    assert_eq!(
        NamespaceContext::new(
            NamespaceId::INVALID,
            NamespaceProfile::Confined,
            NamespaceCapabilities::NONE,
        ),
        None
    );
    assert_eq!(
        ClientAuthProvenance::new(ClientAuthenticationMethod::MitMagicCookie1, 0),
        None
    );

    let namespace = NamespaceContext::new(
        NamespaceId::from_raw(9),
        NamespaceProfile::ClassicShared,
        NamespaceCapabilities::ALL,
    )
    .unwrap();
    let provenance =
        ClientAuthProvenance::new(ClientAuthenticationMethod::MitMagicCookie1, 4).unwrap();
    let admission =
        ClientAdmissionContext::new(ClientAdmissionId::from_raw(12), namespace, provenance)
            .unwrap();

    assert!(admission.is_valid());
    assert_eq!(admission.namespace.profile, NamespaceProfile::ClassicShared);
    assert_eq!(admission.auth_provenance.session_generation, 4);
    assert_eq!(
        ClientAdmissionContext::new(ClientAdmissionId::INVALID, namespace, provenance),
        None
    );
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
        authority_local_id: Some(AuthorityLocalId::new(42, 1)),
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
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let cloned = snapshot.clone();

    assert_eq!(cloned.surface, surface);
    assert_eq!(cloned.damage.rects.len(), 1);
    assert_eq!(cloned.resize_sync, ResizeSyncCapability::ExplicitSync);
}

#[test]
fn authority_local_id_preserves_raw_id_and_generation() {
    let local = AuthorityLocalId::from(XWindowId::new(0x1200042, 7));

    assert!(local.is_valid());
    assert_eq!(local.raw(), 0x1200042);
    assert_eq!(local.generation(), 7);
}

#[test]
fn authority_surface_carries_protocol_ownership_without_metadata() {
    let surface = SurfaceSnapshot {
        surface: SurfaceId::new(3, 1),
        window: XWindowId::new(0x42, 5),
        toplevel: None,
        client: None,
        namespace: Some(NamespaceId::from_raw(2)),
        mapped: true,
        stack_rank: 4,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 640,
            height: 480,
        },
        source: BufferSource::XPixmap { pixmap: 77 },
        damage: Region::empty(),
        generation: 9,
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let authority_surface = surface.to_authority_surface(AuthorityKind::SophiaX);

    assert_eq!(authority_surface.authority, AuthorityKind::SophiaX);
    assert_eq!(authority_surface.local_id, AuthorityLocalId::new(0x42, 5));
    assert_eq!(authority_surface.surface, SurfaceId::new(3, 1));
    assert_eq!(authority_surface.namespace, Some(NamespaceId::from_raw(2)));
    assert!(authority_surface.mapped);
    assert_eq!(authority_surface.generation, 9);
}

#[test]
fn surface_transaction_carries_atomic_geometry_buffer_and_readiness() {
    let layer = LayerSnapshot {
        surface: SurfaceId::new(4, 1),
        authority_local_id: Some(AuthorityLocalId::new(0x99, 2)),
        namespace: Some(NamespaceId::from_raw(8)),
        stack_rank: 0,
        geometry: Rect {
            x: 30,
            y: 40,
            width: 800,
            height: 600,
        },
        source: BufferSource::DmaBuf { handle: 55 },
        damage: Region::single(Rect {
            x: 30,
            y: 40,
            width: 10,
            height: 10,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 6,
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let transaction = SurfaceTransaction::from_layer_snapshot(
        TransactionId::from_raw(12),
        AuthorityKind::SophiaX,
        &layer,
        SurfaceTransactionReadiness::Ready,
        250,
        5,
    );

    assert_eq!(transaction.transaction, TransactionId::from_raw(12));
    assert_eq!(transaction.authority, AuthorityKind::SophiaX);
    assert_eq!(transaction.surface, SurfaceId::new(4, 1));
    assert_eq!(transaction.target_geometry.width, 800);
    assert_eq!(
        transaction.target_buffer,
        BufferSource::DmaBuf { handle: 55 }
    );
    assert_eq!(transaction.damage.rects.len(), 1);
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.previous_committed_generation, 5);
}

#[test]
fn committed_surface_state_is_cloneable_visual_state() {
    let layer = LayerSnapshot {
        surface: SurfaceId::new(5, 1),
        authority_local_id: None,
        namespace: None,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 240,
        },
        source: BufferSource::CpuBuffer { handle: 3 },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 11,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    };

    let state = CommittedSurfaceState::from_layer_snapshot(&layer);
    let cloned = state.clone();

    assert_eq!(cloned.surface, SurfaceId::new(5, 1));
    assert_eq!(cloned.committed_generation, 11);
    assert_eq!(cloned.geometry.width, 320);
    assert_eq!(cloned.buffer, BufferSource::CpuBuffer { handle: 3 });
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
fn routed_input_request_is_protocol_neutral_and_surface_targeted() {
    let request = RoutedInputRequest {
        serial: 99,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1_000,
        target_surface: SurfaceId::new(42, 1),
        global_position: Point { x: 20.0, y: 30.0 },
        local_position: Point { x: 12.5, y: 9.0 },
        kind: InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
    };

    assert_eq!(request.serial, 99);
    assert_eq!(request.target_surface, SurfaceId::new(42, 1));
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
fn routed_input_decision_carries_authority_rejection() {
    let decision = RoutedInputDecision {
        serial: 100,
        target_surface: SurfaceId::new(55, 3),
        outcome: RoutedInputOutcome::RejectedDeniedNamespace,
    };

    assert_eq!(decision.serial, 100);
    assert_eq!(
        decision.outcome,
        RoutedInputOutcome::RejectedDeniedNamespace
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
fn broker_health_frame_roundtrips() {
    let packet = BrokerHealthPacket::new(
        BrokerKind::Portal,
        BrokerHealthState::Ready,
        12,
        Some("ready".to_owned()),
    )
    .unwrap();

    let frame = encode_broker_health_frame(&packet).unwrap();

    assert_eq!(decode_broker_health_frame(&frame), Ok(packet));
    assert_eq!(frame_payload_len(&frame), 12);
}

#[test]
fn portal_broker_and_payload_frames_roundtrip() {
    let transfer = PortalTransferId::from_raw(41);
    let request = PortalBrokerRequestPacket {
        request: PortalRequest {
            transfer: PortalTransfer {
                transfer,
                source_namespace: NamespaceId::from_raw(10),
                target_namespace: NamespaceId::from_raw(20),
                kind: PortalTransferKind::Clipboard,
                mime_type: Some("UTF8_STRING".to_owned()),
                byte_size: 6,
                decision: PortalDecision::Pending,
                generation: 7,
            },
            deadline_msec: 2_000,
        },
        source_may_publish: true,
        target_may_request: true,
    };
    let frame = encode_portal_broker_request_frame(&request).unwrap();
    assert_eq!(decode_portal_broker_request_frame(&frame), Ok(request));

    let response = PortalBrokerResponsePacket {
        transfer,
        decision: PortalBrokerResponseDecision::Allowed(PortalGrant {
            transfer,
            source_namespace: NamespaceId::from_raw(10),
            target_namespace: NamespaceId::from_raw(20),
            kind: PortalTransferKind::Clipboard,
            source_generation: 7,
            broker_generation: 3,
            deadline_msec: 2_000,
            state: PortalGrantState::Active,
        }),
    };
    let frame = encode_portal_broker_response_frame(&response).unwrap();
    assert_eq!(decode_portal_broker_response_frame(&frame), Ok(response));

    let frame = encode_portal_clipboard_payload_frame(transfer, b"sophia").unwrap();
    assert_eq!(
        decode_portal_clipboard_payload_frame(&frame),
        Ok((transfer, b"sophia".to_vec()))
    );
}

#[test]
fn metadata_broker_health_frame_roundtrips_without_message() {
    let packet =
        BrokerHealthPacket::new(BrokerKind::Metadata, BrokerHealthState::Stopped, 13, None)
            .unwrap();

    let frame = encode_broker_health_frame(&packet).unwrap();

    assert_eq!(decode_broker_health_frame(&frame), Ok(packet));
    assert_eq!(frame_payload_len(&frame), 5);
}

#[test]
fn broker_health_frame_rejects_oversized_status_message() {
    let mut payload = Vec::new();
    push_u16(&mut payload, 1);
    push_u16(&mut payload, 3);
    push_u8(&mut payload, 1);
    push_u16(
        &mut payload,
        (SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN as u16) + 1,
    );
    payload.extend(std::iter::repeat_n(
        b'x',
        SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1,
    ));
    let frame = encode_frame(
        IpcMessageKind::BrokerHealth,
        TransactionId::from_raw(4),
        &payload,
    )
    .unwrap();

    assert_eq!(
        decode_broker_health_frame(&frame),
        Err(IpcCodecError::TextTooLarge {
            field: "broker_health_message",
            len: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN + 1,
            max: SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN,
        })
    );
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
#[test]
fn output_topology_validates_bounded_engine_facts() {
    let topology = OutputTopologySnapshot {
        generation: 7,
        primary: OutputId::from_raw(2),
        outputs: vec![
            OutputTopologyEntry {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                pixel_size: Size {
                    width: 1920,
                    height: 1080,
                },
                scale: 1,
                refresh_millihz: 60_000,
            },
            OutputTopologyEntry {
                output: OutputId::from_raw(2),
                logical: Rect {
                    x: 1920,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 2560,
                    height: 1440,
                },
                scale: 2,
                refresh_millihz: 120_000,
            },
        ],
    };
    assert_eq!(
        topology.validate(),
        Ok(Size {
            width: 3200,
            height: 1080,
        })
    );
}

#[test]
fn output_topology_rejects_duplicate_and_unbounded_facts() {
    let mut topology = OutputTopologySnapshot::deterministic();
    topology.outputs.push(topology.outputs[0]);
    assert_eq!(
        topology.validate(),
        Err(OutputTopologyError::DuplicateOutput)
    );

    let mut topology = OutputTopologySnapshot::deterministic();
    topology.outputs[0].logical.width = i32::from(u16::MAX) + 1;
    assert_eq!(
        topology.validate(),
        Err(OutputTopologyError::RootSizeExceeded)
    );
}

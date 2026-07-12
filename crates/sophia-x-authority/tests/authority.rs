use sophia_portal::{ClipboardPortal, PortalCommand};
use sophia_protocol::{
    AuthorityKind, BufferSource, IpcCodecError, IpcMessageKind, NamespaceId, PortalDecision,
    PortalTransferId, Rect, Region, SOPHIA_IPC_MAGIC, Size, SurfaceConstraints, SurfaceId,
    SurfaceTransactionReadiness, TransactionId, encode_frame,
};
use sophia_x_authority::*;

#[test]
fn resource_lookup_is_namespace_scoped() {
    let trusted = NamespaceId::from_raw(1);
    let untrusted = NamespaceId::from_raw(2);
    let window = XResourceId::new(0x20, 1);
    let mut resources = XResourceTable::new();

    resources
        .insert(window, XResourceKind::Window, trusted, 1)
        .unwrap();

    assert_eq!(
        resources
            .lookup(trusted, window, XResourceKind::Window)
            .unwrap()
            .owner_namespace,
        trusted
    );
    assert_eq!(
        resources.lookup(untrusted, window, XResourceKind::Window),
        Err(XAuthorityAccessError::CrossNamespaceDenied)
    );
    assert_eq!(
        resources.lookup(trusted, window, XResourceKind::Pixmap),
        Err(XAuthorityAccessError::WrongResourceKind)
    );
}

#[test]
fn event_subscriptions_do_not_cross_namespaces() {
    let trusted = NamespaceId::from_raw(1);
    let untrusted = NamespaceId::from_raw(2);
    let window = XResourceId::new(0x30, 1);
    let mut resources = XResourceTable::new();
    let mut subscriptions = XEventSubscriptionTable::new();

    resources
        .insert(window, XResourceKind::Window, trusted, 1)
        .unwrap();
    subscriptions
        .subscribe(&resources, trusted, window, XEventClass::Structure)
        .unwrap();

    assert_eq!(
        subscriptions.subscribe(&resources, untrusted, window, XEventClass::Structure),
        Err(XAuthorityAccessError::CrossNamespaceDenied)
    );
    assert_eq!(
        subscriptions.subscribers(window, trusted, XEventClass::Structure),
        vec![trusted]
    );
    assert!(
        subscriptions
            .subscribers(window, untrusted, XEventClass::Structure)
            .is_empty()
    );
}

#[test]
fn window_lifecycle_creates_authority_surface_records() {
    let namespace = NamespaceId::from_raw(7);
    let window = XResourceId::new(0x40, 1);
    let surface = SurfaceId::new(3, 1);
    let mut windows = XWindowTable::new();

    let created = windows
        .apply(XWindowLifecycleEvent::Created {
            id: window,
            surface,
            namespace,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        })
        .unwrap()
        .expect("created window should emit authority surface");

    assert_eq!(created.authority, AuthorityKind::SophiaX);
    assert_eq!(created.local_id, window.local);
    assert_eq!(created.surface, surface);
    assert_eq!(created.namespace, Some(namespace));
    assert!(!created.mapped);

    let mapped = windows
        .apply(XWindowLifecycleEvent::Mapped {
            id: window,
            generation: 2,
        })
        .unwrap()
        .expect("mapped window should emit authority surface");

    assert!(mapped.mapped);
    assert_eq!(mapped.generation, 2);

    let destroyed = windows
        .apply(XWindowLifecycleEvent::Destroyed { id: window })
        .unwrap();

    assert_eq!(destroyed, None);
    assert!(windows.is_empty());
}

#[test]
fn present_pixmap_update_becomes_ready_surface_transaction() {
    let namespace = NamespaceId::from_raw(7);
    let window = XResourceId::new(0x50, 1);
    let mut windows = window_table_with_surface(window, namespace);

    windows
        .apply(XWindowLifecycleEvent::Mapped {
            id: window,
            generation: 2,
        })
        .unwrap();

    let transaction = surface_transaction_from_drawing_update(
        &windows,
        XDrawingUpdate::present_pixmap(
            TransactionId::from_raw(9),
            namespace,
            window,
            0x900,
            Region::single(Rect {
                x: 10,
                y: 20,
                width: 32,
                height: 24,
            }),
            4,
            250,
        ),
    )
    .unwrap();

    assert_eq!(transaction.transaction, TransactionId::from_raw(9));
    assert_eq!(transaction.authority, AuthorityKind::SophiaX);
    assert_eq!(transaction.surface, SurfaceId::new(3, 1));
    assert_eq!(transaction.namespace, Some(namespace));
    assert_eq!(
        transaction.target_buffer,
        BufferSource::XPixmap { pixmap: 0x900 }
    );
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.previous_committed_generation, 4);
    assert_eq!(transaction.timeout_msec, 250);
    assert_eq!(transaction.damage.rects.len(), 1);
}

#[test]
fn shm_and_core_draw_updates_become_ready_cpu_buffer_transactions() {
    let namespace = NamespaceId::from_raw(8);
    let window = XResourceId::new(0x60, 1);
    let windows = window_table_with_surface(window, namespace);

    let shm = surface_transaction_from_drawing_update(
        &windows,
        XDrawingUpdate::shm_put_image(
            TransactionId::from_raw(10),
            namespace,
            window,
            100,
            Region::single(Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            }),
            1,
            300,
        ),
    )
    .unwrap();
    let core = surface_transaction_from_drawing_update(
        &windows,
        XDrawingUpdate::core_draw(
            TransactionId::from_raw(11),
            namespace,
            window,
            101,
            Region::single(Rect {
                x: 5,
                y: 6,
                width: 7,
                height: 8,
            }),
            2,
            300,
        ),
    )
    .unwrap();

    assert_eq!(shm.target_buffer, BufferSource::CpuBuffer { handle: 100 });
    assert_eq!(shm.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(shm.previous_committed_generation, 1);
    assert_eq!(core.target_buffer, BufferSource::CpuBuffer { handle: 101 });
    assert_eq!(core.damage.rects[0].width, 7);
    assert_eq!(core.previous_committed_generation, 2);
}

#[test]
fn repeated_runtime_draws_advance_surface_generations() {
    let namespace = NamespaceId::from_raw(8);
    let window = XResourceId::new(0x61, 1);
    let mut runtime = XAuthorityRuntime::new();
    let created = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(12),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(12, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 40,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 5,
        },
    });
    assert_eq!(created.outcome, XAuthorityResponseOutcome::Accepted);

    let damage = Region::single(Rect {
        x: 1,
        y: 2,
        width: 8,
        height: 12,
    });
    let first = runtime.apply_core_draw(
        TransactionId::from_raw(13),
        namespace,
        window,
        damage.clone(),
    );
    let second = runtime.apply_core_draw(TransactionId::from_raw(14), namespace, window, damage);

    assert_eq!(first.transactions[0].previous_committed_generation, 5);
    assert_eq!(second.transactions[0].previous_committed_generation, 6);
}

#[test]
fn engine_size_control_updates_authority_geometry_without_consuming_client_generation() {
    let namespace = NamespaceId::from_raw(18);
    let window = XResourceId::new(0x62, 1);
    let surface = SurfaceId::new(18, 1);
    let mut runtime = XAuthorityRuntime::new();
    let created = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(18),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface,
            geometry: Rect {
                x: 9,
                y: 11,
                width: 80,
                height: 40,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 5,
        },
    });
    assert_eq!(created.outcome, XAuthorityResponseOutcome::Accepted);

    assert_eq!(
        runtime
            .configure_window_size_from_engine(
                namespace,
                window,
                Size {
                    width: 120,
                    height: 70,
                },
            )
            .unwrap(),
        Rect {
            x: 9,
            y: 11,
            width: 120,
            height: 70,
        }
    );
    let draw = runtime.apply_core_draw(
        TransactionId::from_raw(19),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        }),
    );
    assert_eq!(draw.transactions[0].previous_committed_generation, 5);
    assert_eq!(draw.transactions[0].target_geometry.width, 120);
    assert_eq!(draw.transactions[0].target_geometry.height, 70);
}

#[test]
fn cpu_buffer_patches_materialize_and_resize_replacements_keep_generation_order() {
    let namespace = NamespaceId::from_raw(19);
    let window = XResourceId::new(0x63, 1);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(20),
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window,
            surface: SurfaceId::new(19, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 40,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    });

    runtime.apply_core_draw(
        TransactionId::from_raw(21),
        namespace,
        window,
        Region::single(Rect {
            x: 1,
            y: 1,
            width: 3,
            height: 3,
        }),
    );
    let first = runtime.take_cpu_buffer_update().unwrap();
    assert!(matches!(first, XAuthorityCpuBufferUpdate::Replace(_)));
    runtime.apply_core_draw(
        TransactionId::from_raw(22),
        namespace,
        window,
        Region::single(Rect {
            x: 10,
            y: 10,
            width: 2,
            height: 2,
        }),
    );
    let second = runtime.take_cpu_buffer_update().unwrap();
    assert!(matches!(second, XAuthorityCpuBufferUpdate::Patch(_)));

    let mut materialized = std::collections::BTreeMap::new();
    first.apply_to(&mut materialized).unwrap();
    second.apply_to(&mut materialized).unwrap();
    assert_eq!(materialized.len(), 1);
    assert_eq!(materialized.values().next().unwrap().generation, 2);

    runtime
        .configure_window_size_from_engine(
            namespace,
            window,
            Size {
                width: 120,
                height: 70,
            },
        )
        .unwrap();
    runtime.apply_core_draw(
        TransactionId::from_raw(23),
        namespace,
        window,
        Region::single(Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        }),
    );
    let replacement = runtime.take_cpu_buffer_update().unwrap();
    assert!(matches!(replacement, XAuthorityCpuBufferUpdate::Replace(_)));
    assert_eq!(replacement.generation(), 3);
    replacement.apply_to(&mut materialized).unwrap();
    let resized = materialized.values().next().unwrap();
    assert_eq!(resized.size.width, 120);
    assert_eq!(resized.size.height, 70);
}

#[test]
fn evdev_keyboard_mapping_preserves_x_modifier_event_order() {
    let mut keyboard = XCoreKeyboardMapper::new();
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.modifier_mask(), 1);
    assert_eq!(keyboard.map_evdev_key(30, true), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(30, false), Some((38, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);

    assert_eq!(keyboard.map_evdev_key(58, true), Some((66, 0)));
    assert_eq!(keyboard.modifier_mask(), 2);
    assert_eq!(keyboard.map_evdev_key(103, true), Some((111, 2)));
    assert_eq!(keyboard.map_evdev_key(105, true), Some((113, 2)));
    assert_eq!(keyboard.map_evdev_key(106, true), Some((114, 2)));
    assert_eq!(keyboard.map_evdev_key(108, true), Some((116, 2)));
    assert_eq!(keyboard.map_evdev_key(0, true), None);
    assert_eq!(keyboard.map_evdev_key(u32::MAX, true), None);
}

#[test]
fn repeated_modifier_edges_do_not_leave_core_state_stuck() {
    let mut keyboard = XCoreKeyboardMapper::new();
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 0)));
    assert_eq!(keyboard.map_evdev_key(42, true), Some((50, 1)));
    assert_eq!(keyboard.map_evdev_key(42, false), Some((50, 1)));
    assert_eq!(keyboard.modifier_mask(), 0);
}

#[test]
fn evdev_pointer_mapping_preserves_core_button_state_order() {
    let mut pointer = XCorePointerMapper::new();

    assert_eq!(pointer.map_evdev_button(272, true), Some((1, 0)));
    assert_eq!(pointer.state(), 1 << 8);
    assert_eq!(pointer.map_evdev_button(272, false), Some((1, 1 << 8)));
    assert_eq!(pointer.state(), 0);
    assert_eq!(pointer.map_evdev_button(999, true), None);
}

#[test]
fn drawing_updates_fail_closed_for_cross_namespace_or_unknown_windows() {
    let owner = NamespaceId::from_raw(1);
    let other = NamespaceId::from_raw(2);
    let window = XResourceId::new(0x70, 1);
    let windows = window_table_with_surface(window, owner);

    assert_eq!(
        surface_transaction_from_drawing_update(
            &windows,
            XDrawingUpdate::present_pixmap(
                TransactionId::from_raw(12),
                other,
                window,
                0x901,
                Region::empty(),
                1,
                250,
            ),
        ),
        Err(XAuthorityAccessError::CrossNamespaceDenied)
    );

    assert_eq!(
        surface_transaction_from_drawing_update(
            &windows,
            XDrawingUpdate::present_pixmap(
                TransactionId::from_raw(12),
                owner,
                XResourceId::new(0x71, 1),
                0x901,
                Region::empty(),
                1,
                250,
            ),
        ),
        Err(XAuthorityAccessError::UnknownResource)
    );
}

#[test]
fn selection_owner_events_track_namespace_and_generation() {
    let namespace = NamespaceId::from_raw(11);
    let owner = XResourceId::new(0x80, 1);
    let windows = window_table_with_surface(owner, namespace);
    let mut monitor = XSelectionMonitor::new();

    let first = monitor.apply_event(
        XSelectionEvent {
            selection: 1,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );
    let second = monitor.apply_event(
        XSelectionEvent {
            selection: 1,
            owner: Some(owner),
            timestamp: 11,
            selection_timestamp: 11,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );

    assert_eq!(first.current.namespace, Some(namespace));
    assert_eq!(first.current.generation, 1);
    assert_eq!(second.previous, Some(first.current));
    assert_eq!(second.current.generation, 2);
    assert_eq!(
        monitor.current_owner_for_selection(1).unwrap(),
        second.current
    );
}

#[test]
fn selection_request_becomes_portal_prompt_and_native_denial_artifact() {
    let source_namespace = NamespaceId::from_raw(11);
    let target_namespace = NamespaceId::from_raw(12);
    let owner = XResourceId::new(0x90, 1);
    let requestor = XResourceId::new(0x91, 1);
    let windows =
        window_table_with_two_surfaces(owner, source_namespace, requestor, target_namespace);
    let mut monitor = XSelectionMonitor::new();
    monitor.apply_event(
        XSelectionEvent {
            selection: 7,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );

    let transfer = PortalTransferId::from_raw(5);
    let mut portal = ClipboardPortal::new();
    let dispatch = dispatch_clipboard_selection_request(
        XSelectionRequest {
            requestor,
            selection: 7,
            target: 8,
            target_name: "UTF8_STRING".to_owned(),
            property: 9,
            time: 30,
        },
        &monitor,
        &windows,
        transfer,
        &mut portal,
    )
    .unwrap();

    let PortalCommand::PromptClipboardTransfer(prompt) = &dispatch.command else {
        panic!("expected clipboard prompt");
    };
    assert_eq!(prompt.transfer, transfer);
    assert_eq!(prompt.source_namespace, source_namespace);
    assert_eq!(prompt.target_namespace, target_namespace);
    assert_eq!(prompt.decision, PortalDecision::Pending);
    assert_eq!(prompt.generation, 1);
    assert_eq!(dispatch.portal_request.property, 9);

    let PortalCommand::FailSelection { transfer: denied } = portal.deny(transfer).unwrap() else {
        panic!("expected fail-selection command");
    };
    let failure = clipboard_selection_failure_notify(dispatch.portal_request.failure);

    assert_eq!(denied, transfer);
    assert_eq!(failure.transfer, transfer);
    assert!(failure.failed_normally());
    assert_eq!(failure.notify.requestor, requestor);
    assert_eq!(failure.notify.selection, 7);
    assert_eq!(failure.notify.target, 8);
    assert_eq!(failure.notify.property, X_ATOM_NONE);
}

#[test]
fn approved_selection_request_becomes_bounded_text_handoff_artifact() {
    let source_namespace = NamespaceId::from_raw(13);
    let target_namespace = NamespaceId::from_raw(14);
    let owner = XResourceId::new(0xa0, 1);
    let requestor = XResourceId::new(0xa1, 1);
    let windows =
        window_table_with_two_surfaces(owner, source_namespace, requestor, target_namespace);
    let mut monitor = XSelectionMonitor::new();
    let update = monitor.apply_event(
        XSelectionEvent {
            selection: 17,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );
    let transfer = PortalTransferId::from_raw(6);
    let mut portal = ClipboardPortal::new();
    let dispatch = dispatch_clipboard_selection_request(
        XSelectionRequest {
            requestor,
            selection: 17,
            target: 18,
            target_name: "text/plain;charset=utf-8".to_owned(),
            property: 19,
            time: 31,
        },
        &monitor,
        &windows,
        transfer,
        &mut portal,
    )
    .unwrap();
    let command = portal
        .approve_generation(transfer, update.current.generation)
        .unwrap();
    let handoff =
        clipboard_selection_text_handoff_artifact(&command, &dispatch.portal_request, "hello")
            .unwrap();

    assert_eq!(handoff.transfer, transfer);
    assert_eq!(handoff.property.requestor, requestor);
    assert_eq!(handoff.property.property, 19);
    assert_eq!(handoff.property.target, 18);
    assert_eq!(handoff.property.bytes, b"hello");
    assert!(handoff.succeeded_normally());
    assert_eq!(handoff.notify.property, 19);
}

#[test]
fn selection_requests_fail_closed_without_cross_namespace_boundary() {
    let namespace = NamespaceId::from_raw(15);
    let owner = XResourceId::new(0xb0, 1);
    let requestor = XResourceId::new(0xb1, 1);
    let windows = window_table_with_two_surfaces(owner, namespace, requestor, namespace);
    let mut monitor = XSelectionMonitor::new();
    monitor.apply_event(
        XSelectionEvent {
            selection: 27,
            owner: Some(owner),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
        &windows,
    );

    assert_eq!(
        clipboard_portal_request_from_selection_request(
            XSelectionRequest {
                requestor,
                selection: 27,
                target: 28,
                target_name: "UTF8_STRING".to_owned(),
                property: 29,
                time: 32,
            },
            &monitor,
            &windows,
            PortalTransferId::from_raw(7),
        ),
        Err(ClipboardSelectionRequestError::SameNamespace)
    );
}

#[test]
fn x_authority_request_codec_round_trips_create_window() {
    let request = create_window_request(TransactionId::from_raw(100), NamespaceId::from_raw(21));

    let frame = encode_x_authority_request_frame(&request).unwrap();
    let decoded = decode_x_authority_request_frame(&frame).unwrap();

    assert_eq!(decoded, request);
}

#[test]
fn x_authority_response_codec_round_trips_runtime_outputs() {
    let namespace = NamespaceId::from_raw(22);
    let mut runtime = XAuthorityRuntime::new();
    let create = runtime.apply(create_window_request(
        TransactionId::from_raw(101),
        namespace,
    ));
    let map = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(102),
        namespace,
        kind: XAuthorityRequestKind::MapWindow {
            window: XResourceId::new(0xc0, 1),
            generation: 2,
        },
    });
    let present = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(103),
        namespace,
        kind: XAuthorityRequestKind::PresentPixmap {
            window: XResourceId::new(0xc0, 1),
            pixmap: 0x777,
            damage: Region::single(Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }),
            previous_committed_generation: 2,
            timeout_msec: 250,
        },
    });

    assert_eq!(create.surfaces.len(), 1);
    assert_eq!(map.surfaces.len(), 1);
    assert_eq!(present.transactions.len(), 1);

    let frame = encode_x_authority_response_frame(&present).unwrap();
    let decoded = decode_x_authority_response_frame(&frame).unwrap();

    assert_eq!(decoded, present);
}

#[test]
fn x_authority_codec_rejects_wrong_message_kind() {
    let payload = Vec::new();
    let frame = encode_frame(
        IpcMessageKind::WmRequest,
        TransactionId::from_raw(104),
        &payload,
    )
    .unwrap();

    assert_eq!(
        decode_x_authority_request_frame(&frame),
        Err(IpcCodecError::InvalidEnum {
            field: "message_kind",
            value: IpcMessageKind::WmRequest as u32,
        })
    );
}

#[test]
fn x_authority_codec_rejects_bad_magic_and_trailing_bytes() {
    let request = create_window_request(TransactionId::from_raw(105), NamespaceId::from_raw(23));
    let mut bad_magic = encode_x_authority_request_frame(&request).unwrap();
    bad_magic[0..4].copy_from_slice(&(SOPHIA_IPC_MAGIC ^ 0xffff).to_le_bytes());

    assert_eq!(
        decode_x_authority_request_frame(&bad_magic),
        Err(IpcCodecError::BadMagic)
    );

    let mut trailing = encode_x_authority_request_frame(&request).unwrap();
    trailing.push(0);

    assert_eq!(
        decode_x_authority_request_frame(&trailing),
        Err(IpcCodecError::TrailingBytes(1))
    );
}

#[test]
fn x_authority_runtime_sequence_emits_surface_transaction_and_portal_prompt() {
    let source_namespace = NamespaceId::from_raw(24);
    let target_namespace = NamespaceId::from_raw(25);
    let mut runtime = XAuthorityRuntime::new();

    assert_eq!(
        runtime
            .apply(create_window_request(
                TransactionId::from_raw(106),
                source_namespace
            ))
            .surfaces
            .len(),
        1
    );
    assert_eq!(
        runtime
            .apply(create_second_window_request(
                TransactionId::from_raw(107),
                target_namespace
            ))
            .surfaces
            .len(),
        1
    );
    runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(108),
        namespace: source_namespace,
        kind: XAuthorityRequestKind::SetSelectionOwner {
            selection: 77,
            owner: Some(XResourceId::new(0xc0, 1)),
            timestamp: 10,
            selection_timestamp: 10,
            kind: XSelectionChangeKind::SetOwner,
        },
    });
    let present = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(109),
        namespace: source_namespace,
        kind: XAuthorityRequestKind::PresentPixmap {
            window: XResourceId::new(0xc0, 1),
            pixmap: 0x778,
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 50,
                height: 60,
            }),
            previous_committed_generation: 1,
            timeout_msec: 250,
        },
    });
    let selection = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(110),
        namespace: target_namespace,
        kind: XAuthorityRequestKind::RequestSelection {
            requestor: XResourceId::new(0xc1, 1),
            selection: 77,
            target: 78,
            target_name: "UTF8_STRING".to_owned(),
            property: 79,
            time: 11,
            transfer: PortalTransferId::from_raw(12),
        },
    });

    assert_eq!(runtime.resource_count(), 2);
    assert_eq!(runtime.window_count(), 2);
    assert_eq!(present.transactions.len(), 1);
    assert_eq!(
        present.transactions[0].readiness,
        SurfaceTransactionReadiness::Ready
    );
    assert_eq!(selection.portal_commands.len(), 1);
}

#[test]
fn x_authority_runtime_selection_error_emits_native_failure_artifact() {
    let namespace = NamespaceId::from_raw(26);
    let mut runtime = XAuthorityRuntime::new();
    runtime.apply(create_window_request(
        TransactionId::from_raw(111),
        namespace,
    ));

    let response = runtime.apply(XAuthorityRequestPacket {
        transaction: TransactionId::from_raw(112),
        namespace,
        kind: XAuthorityRequestKind::RequestSelection {
            requestor: XResourceId::new(0xc0, 1),
            selection: 88,
            target: 89,
            target_name: "UTF8_STRING".to_owned(),
            property: 90,
            time: 12,
            transfer: PortalTransferId::from_raw(13),
        },
    });

    assert_eq!(
        response.outcome,
        XAuthorityResponseOutcome::Rejected(XAuthorityRuntimeError::UnknownSourceOwner)
    );
    assert_eq!(response.selection_artifacts.len(), 1);
}

#[cfg(unix)]
#[test]
fn x_authority_socket_round_trips_repeated_requests() {
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-authority-test-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = thread::spawn(move || {
        run_x_authority_socket_server_once(&server_path).unwrap();
    });

    wait_for_socket(&socket_path);
    let mut stream = UnixStream::connect(&socket_path).unwrap();
    write_x_authority_request(
        &mut stream,
        &create_window_request(TransactionId::from_raw(113), NamespaceId::from_raw(27)),
    )
    .unwrap();
    let first = read_x_authority_response(&mut stream).unwrap();
    write_x_authority_request(
        &mut stream,
        &XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(114),
            namespace: NamespaceId::from_raw(27),
            kind: XAuthorityRequestKind::MapWindow {
                window: XResourceId::new(0xc0, 1),
                generation: 2,
            },
        },
    )
    .unwrap();
    let second = read_x_authority_response(&mut stream).unwrap();

    assert_eq!(first.surfaces.len(), 1);
    assert_eq!(second.surfaces.len(), 1);
    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    let _ = server.join();
}

fn window_table_with_surface(window: XResourceId, namespace: NamespaceId) -> XWindowTable {
    let mut windows = XWindowTable::new();
    windows
        .apply(XWindowLifecycleEvent::Created {
            id: window,
            surface: SurfaceId::new(3, 1),
            namespace,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        })
        .unwrap();
    windows
}

fn window_table_with_two_surfaces(
    first: XResourceId,
    first_namespace: NamespaceId,
    second: XResourceId,
    second_namespace: NamespaceId,
) -> XWindowTable {
    let mut windows = XWindowTable::new();
    windows
        .apply(XWindowLifecycleEvent::Created {
            id: first,
            surface: SurfaceId::new(4, 1),
            namespace: first_namespace,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        })
        .unwrap();
    windows
        .apply(XWindowLifecycleEvent::Created {
            id: second,
            surface: SurfaceId::new(5, 1),
            namespace: second_namespace,
            geometry: Rect {
                x: 660,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        })
        .unwrap();
    windows
}

fn create_window_request(
    transaction: TransactionId,
    namespace: NamespaceId,
) -> XAuthorityRequestPacket {
    XAuthorityRequestPacket {
        transaction,
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window: XResourceId::new(0xc0, 1),
            surface: SurfaceId::new(30, 1),
            geometry: Rect {
                x: 10,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    }
}

fn create_second_window_request(
    transaction: TransactionId,
    namespace: NamespaceId,
) -> XAuthorityRequestPacket {
    XAuthorityRequestPacket {
        transaction,
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window: XResourceId::new(0xc1, 1),
            surface: SurfaceId::new(31, 1),
            geometry: Rect {
                x: 700,
                y: 20,
                width: 320,
                height: 240,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    }
}

#[cfg(unix)]
fn wait_for_socket(path: &std::path::Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out waiting for socket {}", path.display());
}

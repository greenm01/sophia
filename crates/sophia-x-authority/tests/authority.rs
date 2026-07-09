use sophia_portal::{ClipboardPortal, PortalCommand};
use sophia_protocol::{
    AuthorityKind, BufferSource, NamespaceId, PortalDecision, PortalTransferId, Rect, Region,
    SurfaceConstraints, SurfaceId, SurfaceTransactionReadiness, TransactionId,
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

use sophia_protocol::{
    AuthorityKind, BufferSource, NamespaceId, Rect, Region, SurfaceConstraints, SurfaceId,
    SurfaceTransactionReadiness, TransactionId,
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

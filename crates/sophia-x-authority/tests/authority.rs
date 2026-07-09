use sophia_protocol::{AuthorityKind, NamespaceId, Rect, SurfaceConstraints, SurfaceId};
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

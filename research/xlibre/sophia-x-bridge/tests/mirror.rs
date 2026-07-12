mod support;

use support::*;

#[test]
fn mirror_records_discovered_namespace_ownership() {
    let mut state = XMirrorState::default();
    let mut frame = mirror(0x20, None, 0);
    frame.client = Some(xid(0x30));
    frame.toplevel = Some(xid(0x20));
    let mut client = mirror(0x30, Some(0x20), 0);
    client.client = Some(xid(0x30));
    client.toplevel = Some(xid(0x20));
    state.ingest_window(frame);
    state.ingest_window(client);

    state.apply_namespace_ownership(&[NamespaceOwnership {
        window: xid(0x30),
        namespace: NamespaceId::from_raw(7),
    }]);

    for mirror in state.windows() {
        assert_eq!(mirror.namespace, Some(NamespaceId::from_raw(7)));
        assert_eq!(mirror.stale_metadata, 1);
    }
}

#[test]
fn test_client_config_has_bounded_defaults() {
    let config = TestClientConfig::default();

    assert!(config.size.width > 0);
    assert!(config.size.height > 0);
    assert!(config.hold_millis > 0);
}

#[test]
fn wraps_imported_xids_with_initial_generation() {
    assert_eq!(xid(0x1200042), XWindowId::new(0x1200042, 1));
}

#[test]
fn mirror_events_update_map_state() {
    let mut state = XMirrorState::default();
    state.ingest_window(mirror(0x10, None, 0));

    state.apply_event(XMirrorEvent::Map { window: xid(0x10) });
    assert!(state.windows()[0].mapped);

    state.apply_event(XMirrorEvent::Unmap { window: xid(0x10) });
    assert!(!state.windows()[0].mapped);
}

#[test]
fn mirror_events_remove_destroyed_windows_from_parent_children() {
    let mut state = XMirrorState::default();
    let mut parent = mirror(0x10, None, 0);
    parent.children.push(xid(0x20));
    state.ingest_window(parent);
    state.ingest_window(mirror(0x20, Some(0x10), 0));

    state.apply_event(XMirrorEvent::Destroy { window: xid(0x20) });

    assert_eq!(state.windows().len(), 1);
    assert!(state.windows()[0].children.is_empty());
}

#[test]
fn mirror_events_reparent_windows() {
    let mut state = XMirrorState::default();
    let mut old_parent = mirror(0x10, None, 0);
    old_parent.children.push(xid(0x30));
    state.ingest_window(old_parent);
    state.ingest_window(mirror(0x20, None, 1));
    state.ingest_window(mirror(0x30, Some(0x10), 0));

    state.apply_event(XMirrorEvent::Reparent {
        window: xid(0x30),
        parent: Some(xid(0x20)),
    });

    let old_parent = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x10))
        .unwrap();
    let new_parent = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x20))
        .unwrap();
    let child = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x30))
        .unwrap();

    assert!(old_parent.children.is_empty());
    assert_eq!(new_parent.children, vec![xid(0x30)]);
    assert_eq!(child.parent, Some(xid(0x20)));
    assert_eq!(child.stale_metadata, 1);
}

#[test]
fn mirror_events_track_restack_and_property_staleness() {
    let mut state = XMirrorState::default();
    state.ingest_window(mirror(0x10, None, 3));
    state.ingest_window(mirror(0x20, None, 5));

    state.apply_event(XMirrorEvent::Configure {
        window: xid(0x10),
        geometry: Rect {
            x: 1,
            y: 2,
            width: 300,
            height: 200,
        },
        above_sibling: Some(xid(0x20)),
    });
    state.apply_event(XMirrorEvent::Property {
        window: xid(0x10),
        atom: 42,
        deleted: false,
    });

    let window = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x10))
        .unwrap();

    assert_eq!(window.stack_rank, 6);
    assert_eq!(window.stale_metadata, 2);
}

#[test]
fn client_hints_mark_root_child_as_toplevel() {
    let mut state = XMirrorState::default();
    state.ingest_window(mirror(0x01, None, 0));
    state.ingest_window(mirror(0x20, Some(0x01), 0));

    state.apply_client_hints(&XClientHints {
        ewmh_clients: vec![xid(0x20)],
        icccm_clients: Vec::new(),
    });

    let client = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x20))
        .unwrap();

    assert_eq!(client.client, Some(xid(0x20)));
    assert_eq!(client.toplevel, Some(xid(0x20)));
}

#[test]
fn unmanaged_client_fallback_marks_mapped_root_children() {
    let mut state = XMirrorState::default();
    state.ingest_window(mirror(0x01, None, 0));
    let mut client = mirror(0x20, Some(0x01), 0);
    client.mapped = true;
    state.ingest_window(client);
    let mut nested = mirror(0x30, Some(0x20), 0);
    nested.mapped = true;
    state.ingest_window(nested);

    state.apply_unmanaged_client_fallback();

    let client = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x20))
        .unwrap();
    let nested = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x30))
        .unwrap();

    assert_eq!(client.client, Some(xid(0x20)));
    assert_eq!(client.toplevel, Some(xid(0x20)));
    assert_eq!(nested.client, None);
}

#[test]
fn client_hints_promote_reparented_frame_as_toplevel() {
    let mut state = XMirrorState::default();
    let mut root = mirror(0x01, None, 0);
    root.children.push(xid(0x20));
    let mut frame = mirror(0x20, Some(0x01), 0);
    frame.children.push(xid(0x30));
    state.ingest_window(root);
    state.ingest_window(frame);
    state.ingest_window(mirror(0x30, Some(0x20), 0));

    state.apply_client_hints(&XClientHints {
        ewmh_clients: Vec::new(),
        icccm_clients: vec![xid(0x30)],
    });

    let frame = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x20))
        .unwrap();
    let client = state
        .windows()
        .iter()
        .find(|mirror| mirror.window == xid(0x30))
        .unwrap();

    assert_eq!(frame.client, Some(xid(0x30)));
    assert_eq!(frame.toplevel, Some(xid(0x20)));
    assert_eq!(client.client, Some(xid(0x30)));
    assert_eq!(client.toplevel, Some(xid(0x20)));
}

#[test]
fn surface_id_map_returns_stable_surface_ids() {
    let mut surfaces = SurfaceIdMap::default();
    let window = xid(0x20);
    let first = surfaces.surface_for_window(window);
    let second = surfaces.surface_for_window(window);

    assert_eq!(first, second);
    assert!(first.is_valid());
}

#[test]
fn surface_id_map_resolves_window_for_surface() {
    let mut surfaces = SurfaceIdMap::default();
    let window = xid(0x20);
    let surface = surfaces.surface_for_window(window);

    assert_eq!(surfaces.window_for_surface(surface), Some(window));
    assert_eq!(surfaces.window_for_surface(SurfaceId::new(99, 1)), None);
}

#[test]
fn close_target_for_surface_prefers_client_window() {
    let mut state = XMirrorState::default();
    let mut surfaces = SurfaceIdMap::default();
    let frame = xid(0x20);
    let client = xid(0x30);
    let surface = surfaces.surface_for_window(frame);
    let mut frame_mirror = mirror(0x20, Some(0x01), 0);
    frame_mirror.client = Some(client);
    frame_mirror.toplevel = Some(frame);
    state.ingest_window(frame_mirror);

    assert_eq!(
        close_target_for_surface(&state, &surfaces, surface),
        Some(client)
    );
}

#[test]
fn close_target_for_surface_falls_back_to_mirrored_window() {
    let mut state = XMirrorState::default();
    let mut surfaces = SurfaceIdMap::default();
    let window = xid(0x20);
    let surface = surfaces.surface_for_window(window);
    state.ingest_window(mirror(0x20, Some(0x01), 0));

    assert_eq!(
        close_target_for_surface(&state, &surfaces, surface),
        Some(window)
    );
}

#[test]
fn wm_delete_client_message_uses_icccm_atoms() {
    let window = xid(0x20);
    let atoms = XAtoms {
        wm_state: 1,
        net_client_list: 2,
        wm_protocols: 3,
        wm_delete_window: 4,
    };
    let event = build_wm_delete_client_message(window, atoms, 55);

    assert_eq!(event.format, 32);
    assert_eq!(event.window, window.xid());
    assert_eq!(event.type_, atoms.wm_protocols);
    assert_eq!(
        event.data.as_data32(),
        [atoms.wm_delete_window, 55, 0, 0, 0]
    );
}

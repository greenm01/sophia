mod support;

use support::*;

#[test]
fn composite_pixmap_map_returns_buffer_sources() {
    let mut pixmaps = CompositePixmapMap::default();
    let window = xid(0x20);

    assert_eq!(pixmaps.source_for_window(window), BufferSource::None);

    let inserted = pixmaps.upsert_named_pixmap(window, 0x9000);

    assert_eq!(inserted.window, window);
    assert_eq!(inserted.retired, None);
    assert_eq!(inserted.current.unwrap().generation, 1);
    assert_eq!(
        pixmaps.source_for_window(window),
        BufferSource::XPixmap { pixmap: 0x9000 }
    );
    assert_eq!(pixmaps.record_for_window(window).unwrap().generation, 1);
    assert_eq!(pixmaps.remove_window(window), Some(0x9000));
    assert_eq!(pixmaps.pixmap_for_window(window), None);
}

#[test]
fn composite_pixmap_map_tracks_replacements_and_removals() {
    let mut pixmaps = CompositePixmapMap::default();
    let window = xid(0x20);

    let first = pixmaps.upsert_named_pixmap(window, 0x9000);
    let same = pixmaps.upsert_named_pixmap(window, 0x9000);
    let second = pixmaps.upsert_named_pixmap(window, 0x9001);
    let removed = pixmaps.remove_window_record(window).unwrap();

    assert_eq!(first.current.unwrap().generation, 1);
    assert_eq!(first.retired, None);
    assert_eq!(same.current.unwrap().generation, 1);
    assert_eq!(same.retired, None);
    assert_eq!(second.current.unwrap().pixmap, 0x9001);
    assert_eq!(second.current.unwrap().generation, 2);
    assert_eq!(second.retired.unwrap().pixmap, 0x9000);
    assert_eq!(second.retired.unwrap().generation, 1);
    assert_eq!(removed.current, None);
    assert_eq!(removed.retired.unwrap().pixmap, 0x9001);
    assert_eq!(removed.retired.unwrap().generation, 2);
    assert_eq!(pixmaps.record_for_window(window), None);
}

#[test]
fn cpu_buffer_store_reuses_handles_for_pixmap_updates() {
    let mut store = CpuBufferStore::default();
    let first = store.upsert_pixmap(
        0x9000,
        Size {
            width: 2,
            height: 2,
        },
        24,
        0x21,
        vec![1, 2, 3, 4],
    );
    let second = store.upsert_pixmap(
        0x9000,
        Size {
            width: 2,
            height: 2,
        },
        24,
        0x21,
        vec![5, 6, 7, 8],
    );

    assert_eq!(first.handle, second.handle);
    assert_eq!(store.handle_for_pixmap(0x9000), Some(first.handle));
    assert_eq!(store.get(first.handle).unwrap().bytes, vec![5, 6, 7, 8]);
    assert_eq!(store.remove_pixmap(0x9000).unwrap().handle, first.handle);
    assert!(store.is_empty());
}

#[test]
fn layers_from_surfaces_keeps_cpu_buffer_sources_renderable() {
    let surface = SurfaceSnapshot {
        surface: SurfaceId::new(1, 1),
        window: xid(0x20),
        toplevel: Some(xid(0x20)),
        client: Some(xid(0x20)),
        namespace: None,
        mapped: true,
        stack_rank: 7,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 320,
            height: 200,
        },
        source: BufferSource::CpuBuffer { handle: 9 },
        damage: Region::single(Rect {
            x: 10,
            y: 20,
            width: 320,
            height: 200,
        }),
        generation: 3,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    };

    let layers = layers_from_surfaces(&[surface]);

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].source, BufferSource::CpuBuffer { handle: 9 });
    assert_eq!(layers[0].stack_rank, 7);
    assert_eq!(layers[0].damage.rects.len(), 1);
}

#[test]
fn xlibre_surface_snapshot_maps_to_prototype_surface_transaction() {
    let surface = SurfaceSnapshot {
        surface: SurfaceId::new(1, 1),
        window: xid(0x20),
        toplevel: Some(xid(0x20)),
        client: Some(xid(0x20)),
        namespace: Some(NamespaceId::from_raw(3)),
        mapped: true,
        stack_rank: 7,
        geometry: Rect {
            x: 10,
            y: 20,
            width: 320,
            height: 200,
        },
        source: BufferSource::CpuBuffer { handle: 9 },
        damage: Region::single(Rect {
            x: 10,
            y: 20,
            width: 320,
            height: 200,
        }),
        generation: 3,
        resize_sync: ResizeSyncCapability::ExplicitSync,
    };

    let authority_surface = surface.to_authority_surface(AuthorityKind::XLibrePrototype);
    let transaction = surface.to_surface_transaction(
        TransactionId::from_raw(12),
        AuthorityKind::XLibrePrototype,
        SurfaceTransactionReadiness::Ready,
        250,
        2,
    );

    assert_eq!(authority_surface.authority, AuthorityKind::XLibrePrototype);
    assert_eq!(authority_surface.local_id, AuthorityLocalId::new(0x20, 1));
    assert_eq!(transaction.authority, AuthorityKind::XLibrePrototype);
    assert_eq!(transaction.namespace, Some(NamespaceId::from_raw(3)));
    assert_eq!(
        transaction.target_buffer,
        BufferSource::CpuBuffer { handle: 9 }
    );
    assert_eq!(transaction.readiness, SurfaceTransactionReadiness::Ready);
    assert_eq!(transaction.previous_committed_generation, 2);
}

#[test]
fn damage_tracker_maps_damage_handles_to_windows() {
    let mut tracker = DamageTracker::default();
    let window = xid(0x20);

    tracker.insert_damage(window, 0x5000);

    assert_eq!(tracker.damage_for_window(window), Some(0x5000));
    assert_eq!(tracker.window_for_damage(0x5000), Some(window));
    assert_eq!(
        tracker.record_for_window(window),
        Some(DamageRecord {
            window,
            damage: 0x5000
        })
    );
}

#[test]
fn damage_tracker_accumulates_and_drains_regions() {
    let mut tracker = DamageTracker::default();
    let window = xid(0x20);
    tracker.insert_damage(window, 0x5000);

    let applied = tracker.apply_event(XDamageEvent {
        window,
        damage: 0x5000,
        drawable: window,
        timestamp: 42,
        area: Rect {
            x: 5,
            y: 6,
            width: 70,
            height: 80,
        },
        drawable_geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
    });

    assert!(applied);
    assert_eq!(
        tracker.pending_damage(window).unwrap().rects,
        vec![Rect {
            x: 5,
            y: 6,
            width: 70,
            height: 80,
        }]
    );
    assert_eq!(tracker.drain_damage(window).rects.len(), 1);
    assert_eq!(tracker.pending_damage(window), None);
}

#[test]
fn x_damage_event_converts_known_x11_damage_notify() {
    let mut tracker = DamageTracker::default();
    let window = xid(0x20);
    tracker.insert_damage(window, 0x5000);

    let event = Event::DamageNotify(x11rb::protocol::damage::NotifyEvent {
        response_type: 0,
        level: ReportLevel::BOUNDING_BOX,
        sequence: 1,
        drawable: 0x20,
        damage: 0x5000,
        timestamp: 42,
        area: x11rb::protocol::xproto::Rectangle {
            x: 5,
            y: 6,
            width: 70,
            height: 80,
        },
        geometry: x11rb::protocol::xproto::Rectangle {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
    });

    let converted = XDamageEvent::from_x11_event(&event, &tracker).unwrap();

    assert_eq!(converted.window, window);
    assert_eq!(converted.damage, 0x5000);
    assert_eq!(
        converted.area,
        Rect {
            x: 5,
            y: 6,
            width: 70,
            height: 80,
        }
    );
}

#[test]
fn emits_damage_frame_from_tracked_client_damage() {
    let mut state = XMirrorState::default();
    let mut frame = mirror(0x20, None, 4);
    frame.mapped = true;
    frame.client = Some(xid(0x30));
    frame.toplevel = Some(xid(0x20));
    frame.geometry = Rect {
        x: 100,
        y: 200,
        width: 640,
        height: 480,
    };
    state.ingest_window(frame);

    let mut surfaces = SurfaceIdMap::default();
    let pixmaps = CompositePixmapMap::default();
    let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);

    let mut tracker = DamageTracker::default();
    tracker.insert_damage(xid(0x30), 0x5000);
    assert!(tracker.apply_event(XDamageEvent {
        window: xid(0x30),
        damage: 0x5000,
        drawable: xid(0x30),
        timestamp: 42,
        area: Rect {
            x: 5,
            y: 6,
            width: 70,
            height: 80,
        },
        drawable_geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
    }));

    let frame = emit_damage_frame(&mut tracker, OutputId::from_raw(1), 9, 2, 3, &snapshots);

    assert_eq!(frame.output, OutputId::from_raw(1));
    assert_eq!(frame.frame_serial, 9);
    assert_eq!(frame.buffer_age, 2);
    assert_eq!(frame.root_generation, 3);
    assert_eq!(frame.affected_surfaces, vec![snapshots[0].surface]);
    assert_eq!(
        frame.damage.rects,
        vec![Rect {
            x: 105,
            y: 206,
            width: 70,
            height: 80,
        }]
    );
    assert!(tracker.pending_damage(xid(0x30)).is_none());
}

#[test]
fn damage_frame_drops_unmapped_surface_damage() {
    let mut state = XMirrorState::default();
    let mut window = mirror(0x20, None, 4);
    window.client = Some(xid(0x20));
    window.toplevel = Some(xid(0x20));
    state.ingest_window(window);

    let mut surfaces = SurfaceIdMap::default();
    let pixmaps = CompositePixmapMap::default();
    let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);

    let mut tracker = DamageTracker::default();
    tracker.insert_damage(xid(0x20), 0x5000);
    assert!(tracker.apply_event(XDamageEvent {
        window: xid(0x20),
        damage: 0x5000,
        drawable: xid(0x20),
        timestamp: 42,
        area: Rect {
            x: 5,
            y: 6,
            width: 70,
            height: 80,
        },
        drawable_geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
    }));

    let frame = emit_damage_frame(&mut tracker, OutputId::from_raw(1), 9, 2, 3, &snapshots);

    assert!(frame.affected_surfaces.is_empty());
    assert!(frame.damage.is_empty());
    assert!(tracker.pending_damage(xid(0x20)).is_none());
}

#[test]
fn cpu_buffer_store_applies_packed_damage_patch_to_cached_pixmap() {
    let mut buffers = CpuBufferStore::default();
    let base = buffers.upsert_pixmap(
        0x5000,
        Size {
            width: 4,
            height: 3,
        },
        24,
        0x20,
        vec![0; 4 * 3 * 4],
    );
    let patch = buffers
        .patch_pixmap(
            0x5000,
            Rect {
                x: 1,
                y: 1,
                width: 2,
                height: 1,
            },
            vec![1, 2, 3, 4, 5, 6, 7, 8],
        )
        .unwrap();

    assert_eq!(patch.handle, base.handle);
    assert_eq!(patch.bytes, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        &buffers.get(base.handle).unwrap().bytes[20..28],
        &[1, 2, 3, 4, 5, 6, 7, 8]
    );
}

#[test]
fn emits_surface_and_layer_snapshots_for_detected_clients() {
    let mut state = XMirrorState::default();
    let mut window = mirror(0x20, None, 4);
    window.mapped = true;
    window.client = Some(xid(0x20));
    window.toplevel = Some(xid(0x20));
    window.geometry = Rect {
        x: 10,
        y: 20,
        width: 640,
        height: 480,
    };
    state.ingest_window(window);

    let mut surfaces = SurfaceIdMap::default();
    let pixmaps = CompositePixmapMap::default();
    let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);
    let layers = state.emit_layers(&mut surfaces, &pixmaps);

    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].window, xid(0x20));
    assert_eq!(snapshots[0].geometry.width, 640);
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].surface, snapshots[0].surface);
    assert_eq!(layers[0].source, BufferSource::None);
}

#[test]
fn wm_protocols_advertisement_maps_to_resize_sync_capability() {
    let sync_atom = 0x200;

    assert_eq!(
        sync_capability_from_wm_protocols(&[0x100, sync_atom], sync_atom),
        ResizeSyncCapability::ExplicitSync
    );
    assert_eq!(
        sync_capability_from_wm_protocols(&[0x100], sync_atom),
        ResizeSyncCapability::ImplicitOnly
    );
}

#[test]
fn sync_registry_marks_advertised_clients_explicit_until_reputation_downgrade() {
    let class = ClientClassKey::new("slow-browser").unwrap();
    let mut registry = SurfaceSyncRegistry::new(SyncReputationTracker::new(3));
    registry.upsert_profile(ClientSyncProfile {
        window: xid(0x30),
        namespace: Some(NamespaceId::from_raw(7)),
        class_key: Some(class),
        advertised_sync: true,
    });

    assert_eq!(
        registry.capability_for_window(xid(0x30)),
        ResizeSyncCapability::ExplicitSync
    );
    assert!(registry.record_timeout_for_window(xid(0x30)));
    assert!(registry.record_timeout_for_window(xid(0x30)));
    assert_eq!(
        registry.capability_for_window(xid(0x30)),
        ResizeSyncCapability::ExplicitSync
    );
    assert!(registry.record_timeout_for_window(xid(0x30)));
    assert_eq!(
        registry.capability_for_window(xid(0x30)),
        ResizeSyncCapability::ImplicitOnly
    );
}

#[test]
fn sync_registry_does_not_blacklist_clients_without_valid_class_key() {
    let mut registry = SurfaceSyncRegistry::new(SyncReputationTracker::new(1));
    registry.upsert_profile(ClientSyncProfile {
        window: xid(0x30),
        namespace: Some(NamespaceId::from_raw(7)),
        class_key: None,
        advertised_sync: true,
    });

    assert!(!registry.record_timeout_for_window(xid(0x30)));
    assert_eq!(
        registry.capability_for_window(xid(0x30)),
        ResizeSyncCapability::ExplicitSync
    );
    assert_eq!(ClientClassKey::new(""), None);
    assert_eq!(ClientClassKey::new("bad\nclass"), None);
}

#[test]
fn sync_registry_feeds_snapshots_without_leaking_class_metadata() {
    let mut state = XMirrorState::default();
    let mut frame = mirror(0x20, None, 4);
    frame.mapped = true;
    frame.client = Some(xid(0x30));
    frame.toplevel = Some(xid(0x20));
    frame.namespace = Some(NamespaceId::from_raw(7));
    state.ingest_window(frame);

    let class = ClientClassKey::new("private-class").unwrap();
    let mut registry = SurfaceSyncRegistry::new(SyncReputationTracker::new(3));
    registry.upsert_profile(ClientSyncProfile {
        window: xid(0x30),
        namespace: Some(NamespaceId::from_raw(7)),
        class_key: Some(class),
        advertised_sync: true,
    });
    let mut surfaces = SurfaceIdMap::default();
    let pixmaps = CompositePixmapMap::default();

    let snapshots = state.emit_surfaces_with_sync(&mut surfaces, &pixmaps, Some(&registry));
    let layers = layers_from_surfaces(&snapshots);

    assert_eq!(snapshots[0].resize_sync, ResizeSyncCapability::ExplicitSync);
    assert_eq!(layers[0].resize_sync, ResizeSyncCapability::ExplicitSync);
    assert!(!format!("{:?}", snapshots[0]).contains("private-class"));
}

#[test]
fn emits_named_pixmap_sources_for_detected_clients() {
    let mut state = XMirrorState::default();
    let mut frame = mirror(0x20, None, 4);
    frame.mapped = true;
    frame.client = Some(xid(0x30));
    frame.toplevel = Some(xid(0x20));
    state.ingest_window(frame);

    let mut surfaces = SurfaceIdMap::default();
    let mut pixmaps = CompositePixmapMap::default();
    pixmaps.insert_named_pixmap(xid(0x30), 0x9000);

    let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);
    let layers = state.emit_layers(&mut surfaces, &pixmaps);

    assert_eq!(
        snapshots[0].source,
        BufferSource::XPixmap { pixmap: 0x9000 }
    );
    assert_eq!(layers[0].source, BufferSource::XPixmap { pixmap: 0x9000 });
}

#[test]
fn composite_redirect_targets_use_unique_mapped_clients() {
    let mut state = XMirrorState::default();
    let mut frame = mirror(0x20, None, 0);
    frame.mapped = true;
    frame.client = Some(xid(0x30));
    frame.toplevel = Some(xid(0x20));
    let mut client = mirror(0x30, Some(0x20), 0);
    client.mapped = true;
    client.client = Some(xid(0x30));
    client.toplevel = Some(xid(0x20));
    let mut unmapped = mirror(0x40, None, 0);
    unmapped.client = Some(xid(0x40));
    unmapped.toplevel = Some(xid(0x40));
    state.ingest_window(frame);
    state.ingest_window(client);
    state.ingest_window(unmapped);

    let targets = state.composite_redirect_targets();

    assert_eq!(
        targets,
        vec![CompositeRedirectTarget {
            window: xid(0x30),
            update: CompositeUpdateMode::Manual,
        }]
    );
}

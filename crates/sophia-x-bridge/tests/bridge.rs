mod tests {
    use sophia_protocol::*;
    use sophia_x_bridge::*;
    use x11rb::protocol::Event;
    use x11rb::protocol::damage::ReportLevel;
    use x11rb::protocol::xfixes::SelectionEvent;

    fn xid(window: u32) -> XWindowId {
        XWindowId::new(window, 1)
    }

    fn status(extension: RequiredExtension, present: bool) -> ExtensionStatus {
        ExtensionStatus {
            extension,
            present,
            major_opcode: present.then_some(128),
            first_event: present.then_some(64),
            first_error: present.then_some(32),
        }
    }

    #[test]
    fn probe_reports_missing_required_extensions() {
        let probe = XConnectionProbe {
            display_name: Some(":99".to_owned()),
            screen_num: 0,
            required_extensions: vec![
                status(RequiredExtension::Composite, true),
                status(RequiredExtension::Damage, false),
            ],
            namespaces: StaticNamespaceConfig::default(),
        };

        assert_eq!(probe.missing_extensions(), vec![RequiredExtension::Damage]);
        assert!(!probe.has_required_extensions());
    }

    #[test]
    fn static_namespace_config_records_known_namespaces() {
        let config = StaticNamespaceConfig::new(vec![NamespaceRecord {
            namespace: NamespaceId::from_raw(1),
            label: "trusted".to_owned(),
            source: NamespaceSource::StaticConfig,
        }]);

        assert_eq!(config.namespaces().len(), 1);
        assert_eq!(config.namespaces()[0].label, "trusted");
        assert_eq!(config.namespaces()[0].source, NamespaceSource::StaticConfig);
    }

    #[test]
    fn discovered_namespace_records_replace_static_records() {
        let config = StaticNamespaceConfig::new(vec![NamespaceRecord {
            namespace: NamespaceId::from_raw(1),
            label: "trusted-static".to_owned(),
            source: NamespaceSource::StaticConfig,
        }])
        .with_discovered(vec![
            NamespaceRecord {
                namespace: NamespaceId::from_raw(1),
                label: "trusted-server".to_owned(),
                source: NamespaceSource::XServer,
            },
            NamespaceRecord {
                namespace: NamespaceId::from_raw(2),
                label: "browser".to_owned(),
                source: NamespaceSource::XServer,
            },
        ]);

        assert_eq!(config.namespaces().len(), 2);
        assert_eq!(config.namespaces()[0].label, "trusted-server");
        assert_eq!(config.namespaces()[0].source, NamespaceSource::XServer);
        assert_eq!(config.namespaces()[1].label, "browser");
    }

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
    fn selection_monitor_attributes_owner_to_namespace() {
        let mut state = XMirrorState::default();
        let mut owner = mirror(0x20, None, 0);
        owner.namespace = Some(NamespaceId::from_raw(7));
        state.ingest_window(owner);
        let mut monitor = XSelectionMonitor::new();

        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(xid(0x20)),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &state,
        );

        assert_eq!(update.previous, None);
        assert_eq!(update.current.namespace, Some(NamespaceId::from_raw(7)));
        assert_eq!(update.current.generation, 1);
        assert_eq!(
            monitor
                .owner(0x100, Some(NamespaceId::from_raw(7)))
                .unwrap()
                .owner,
            Some(xid(0x20))
        );
    }

    #[test]
    fn selection_monitor_increments_generation_per_namespace_selection() {
        let mut state = XMirrorState::default();
        let mut owner = mirror(0x20, None, 0);
        owner.namespace = Some(NamespaceId::from_raw(7));
        state.ingest_window(owner);
        let mut monitor = XSelectionMonitor::new();

        monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(xid(0x20)),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &state,
        );
        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(xid(0x20)),
                timestamp: 13,
                selection_timestamp: 12,
                kind: XSelectionChangeKind::SetOwner,
            },
            &state,
        );

        assert_eq!(update.previous.unwrap().generation, 1);
        assert_eq!(update.current.generation, 2);
    }

    #[test]
    fn selection_owner_update_converts_to_clipboard_portal_owner_change() {
        let mut state = XMirrorState::default();
        let mut owner = mirror(0x20, None, 0);
        owner.namespace = Some(NamespaceId::from_raw(7));
        state.ingest_window(owner);
        let mut monitor = XSelectionMonitor::new();

        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(xid(0x20)),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &state,
        );

        assert_eq!(
            clipboard_portal_owner_change_from_selection_update(&update),
            Some(ClipboardPortalOwnerChange {
                source_namespace: NamespaceId::from_raw(7),
                generation: 1
            })
        );
    }

    #[test]
    fn clipboard_selection_failure_uses_native_selection_notify_none_property() {
        let failure = clipboard_selection_failure_notify(ClipboardSelectionFailureRequest {
            transfer: PortalTransferId::from_raw(9),
            requestor: 0x44,
            selection: 0x100,
            target: 0x200,
            time: 55,
        });

        assert_eq!(failure.transfer, PortalTransferId::from_raw(9));
        assert!(failure.failed_normally());
        assert_eq!(failure.event.requestor, 0x44);
        assert_eq!(failure.event.selection, 0x100);
        assert_eq!(failure.event.target, 0x200);
        assert_eq!(failure.event.property, 0);
    }

    #[test]
    fn selection_owner_loss_uses_previous_known_namespace_for_portal_change() {
        let mut state = XMirrorState::default();
        let mut owner = mirror(0x20, None, 0);
        owner.namespace = Some(NamespaceId::from_raw(7));
        state.ingest_window(owner);
        let mut monitor = XSelectionMonitor::new();
        monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(xid(0x20)),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &state,
        );

        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: None,
                timestamp: 13,
                selection_timestamp: 12,
                kind: XSelectionChangeKind::OwnerClientClosed,
            },
            &state,
        );

        assert_eq!(update.current.namespace, Some(NamespaceId::from_raw(7)));
        assert_eq!(
            clipboard_portal_owner_change_from_selection_update(&update),
            Some(ClipboardPortalOwnerChange {
                source_namespace: NamespaceId::from_raw(7),
                generation: 2
            })
        );
    }

    #[test]
    fn unknown_selection_owner_update_does_not_emit_portal_owner_change() {
        let state = XMirrorState::default();
        let mut monitor = XSelectionMonitor::new();

        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: None,
                timestamp: 13,
                selection_timestamp: 12,
                kind: XSelectionChangeKind::Unknown,
            },
            &state,
        );

        assert_eq!(
            clipboard_portal_owner_change_from_selection_update(&update),
            None
        );
    }

    #[test]
    fn selection_monitor_records_unknown_namespace_for_owner_loss() {
        let state = XMirrorState::default();
        let mut monitor = XSelectionMonitor::new();

        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: None,
                timestamp: 13,
                selection_timestamp: 12,
                kind: XSelectionChangeKind::OwnerClientClosed,
            },
            &state,
        );

        assert_eq!(update.current.namespace, None);
        assert_eq!(update.current.owner, None);
        assert_eq!(update.kind, XSelectionChangeKind::OwnerClientClosed);
    }

    #[test]
    fn xfixes_selection_notify_converts_to_selection_event() {
        let event = Event::XfixesSelectionNotify(x11rb::protocol::xfixes::SelectionNotifyEvent {
            response_type: 0,
            subtype: SelectionEvent::SET_SELECTION_OWNER,
            sequence: 1,
            window: 0x01,
            owner: 0x20,
            selection: 0x100,
            timestamp: 13,
            selection_timestamp: 12,
        });

        let converted = XSelectionEvent::from_x11_event(&event).unwrap();

        assert_eq!(converted.selection, 0x100);
        assert_eq!(converted.owner, Some(xid(0x20)));
        assert_eq!(converted.kind, XSelectionChangeKind::SetOwner);
        assert_eq!(converted.timestamp, 13);
        assert_eq!(converted.selection_timestamp, 12);
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

    fn mirror(window: u32, parent: Option<u32>, stack_rank: u32) -> XWindowMirror {
        XWindowMirror {
            window: xid(window),
            parent: parent.map(xid),
            children: Vec::new(),
            toplevel: None,
            client: None,
            mapped: false,
            stack_rank,
            geometry: Rect {
                x: i32::try_from(window).unwrap_or(0),
                y: 0,
                width: 100,
                height: 50,
            },
            namespace: None,
            stale_metadata: 0,
        }
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
        };

        let layers = layers_from_surfaces(&[surface]);

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].source, BufferSource::CpuBuffer { handle: 9 });
        assert_eq!(layers[0].stack_rank, 7);
        assert_eq!(layers[0].damage.rects.len(), 1);
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

    #[test]
    fn builds_flat_routed_input_request_for_xlibre() {
        let event = input_event(10);
        let route = input_route(
            10,
            InputRouteOutcome::Routed,
            Some(xid(0x30)),
            Some(Point { x: 12.0, y: 8.0 }),
            Transform::IDENTITY,
        );

        let request = build_flat_routed_input_request(&event, &route).unwrap();

        assert_eq!(request.serial, 10);
        assert_eq!(request.seat, SeatId::from_raw(1));
        assert_eq!(request.device, DeviceId::from_raw(2));
        assert_eq!(request.target_window, xid(0x30));
        assert_eq!(request.local_position, Point { x: 12.0, y: 8.0 });
        assert_eq!(
            request.kind,
            InputEventKind::PointerButton {
                button: 1,
                pressed: true,
            }
        );
    }

    #[test]
    fn flat_routed_input_rejects_transformed_routes() {
        let event = input_event(11);
        let route = input_route(
            11,
            InputRouteOutcome::Routed,
            Some(xid(0x30)),
            Some(Point { x: 1.0, y: 2.0 }),
            Transform {
                matrix: [
                    2.0, 0.0, 0.0, //
                    0.0, 2.0, 0.0, //
                    0.0, 0.0, 1.0,
                ],
            },
        );

        assert_eq!(
            build_flat_routed_input_request(&event, &route),
            Err(RoutedInputAdapterError::UnsupportedTransform)
        );
    }

    #[test]
    fn transformed_routed_input_uses_engine_supplied_local_coordinates() {
        let event = input_event(13);
        let route = input_route(
            13,
            InputRouteOutcome::Routed,
            Some(xid(0x30)),
            Some(Point { x: 3.5, y: 4.25 }),
            Transform {
                matrix: [
                    2.0, 0.0, 30.0, //
                    0.0, 2.0, 40.0, //
                    0.0, 0.0, 1.0,
                ],
            },
        );

        let request = build_routed_input_request(&event, &route).unwrap();

        assert_eq!(request.serial, 13);
        assert_eq!(request.target_window, xid(0x30));
        assert_eq!(request.local_position, Point { x: 3.5, y: 4.25 });
    }

    #[test]
    fn transformed_routed_input_rejects_non_finite_local_coordinates() {
        let event = input_event(14);
        let route = input_route(
            14,
            InputRouteOutcome::Routed,
            Some(xid(0x30)),
            Some(Point {
                x: f64::NAN,
                y: 4.25,
            }),
            Transform {
                matrix: [
                    2.0, 0.0, 30.0, //
                    0.0, 2.0, 40.0, //
                    0.0, 0.0, 1.0,
                ],
            },
        );

        assert_eq!(
            build_routed_input_request(&event, &route),
            Err(RoutedInputAdapterError::InvalidLocalPosition)
        );
    }

    #[test]
    fn flat_routed_input_rejects_stale_target_before_xlibre_request() {
        let event = input_event(12);
        let route = input_route(
            12,
            InputRouteOutcome::StaleTarget,
            Some(xid(0x30)),
            Some(Point { x: 1.0, y: 2.0 }),
            Transform::IDENTITY,
        );

        assert_eq!(
            build_flat_routed_input_request(&event, &route),
            Err(RoutedInputAdapterError::StaleTarget)
        );
    }

    #[test]
    fn xlibre_decision_blocks_denied_namespace_grab_and_focus_cases() {
        for outcome in [
            XLibreRoutedInputOutcome::RejectedDeniedNamespace,
            XLibreRoutedInputOutcome::RejectedActiveGrab,
            XLibreRoutedInputOutcome::RejectedFocusPolicy,
            XLibreRoutedInputOutcome::RejectedStaleTarget,
        ] {
            let decision = XLibreRoutedInputDecision {
                serial: 13,
                target_window: xid(0x30),
                outcome,
            };

            assert!(!routed_input_decision_allows_delivery(&decision));
        }
    }

    #[test]
    fn xlibre_decision_accepts_only_server_accepted_delivery() {
        let decision = XLibreRoutedInputDecision {
            serial: 14,
            target_window: xid(0x30),
            outcome: XLibreRoutedInputOutcome::Accepted,
        };

        assert!(routed_input_decision_allows_delivery(&decision));
    }

    #[test]
    fn routed_input_wire_length_is_fixed_for_dispatch_measurement() {
        assert_eq!(routed_input_request_wire_len(), 44);
    }

    #[test]
    fn routed_input_dispatch_stats_summarize_samples() {
        let stats = RoutedInputDispatchStats::from_samples([
            std::time::Duration::from_micros(50),
            std::time::Duration::from_micros(100),
            std::time::Duration::from_micros(150),
        ]);

        assert_eq!(stats.sample_count(), 3);
        assert_eq!(stats.min(), Some(std::time::Duration::from_micros(50)));
        assert_eq!(stats.max(), Some(std::time::Duration::from_micros(150)));
        assert_eq!(stats.average(), Some(std::time::Duration::from_micros(100)));
    }

    #[test]
    fn routed_input_dispatch_stats_report_nearest_percentiles() {
        let stats = RoutedInputDispatchStats::from_samples([
            std::time::Duration::from_micros(10),
            std::time::Duration::from_micros(20),
            std::time::Duration::from_micros(30),
            std::time::Duration::from_micros(40),
            std::time::Duration::from_micros(50),
        ]);

        assert_eq!(
            stats.percentile_nearest(0),
            Some(std::time::Duration::from_micros(10))
        );
        assert_eq!(
            stats.percentile_nearest(50),
            Some(std::time::Duration::from_micros(30))
        );
        assert_eq!(
            stats.percentile_nearest(95),
            Some(std::time::Duration::from_micros(50))
        );
        assert_eq!(
            stats.percentile_nearest(100),
            Some(std::time::Duration::from_micros(50))
        );
    }

    #[test]
    fn routed_input_dispatch_stats_keep_x11_path_until_threshold_is_exceeded() {
        let mut stats = RoutedInputDispatchStats::new();

        assert_eq!(
            stats.recommendation(std::time::Duration::from_micros(500)),
            RoutedInputOptimizationRecommendation::KeepX11RequestPath
        );

        stats.record(std::time::Duration::from_micros(250));
        stats.record(std::time::Duration::from_micros(500));
        assert_eq!(
            stats.recommendation(std::time::Duration::from_micros(500)),
            RoutedInputOptimizationRecommendation::KeepX11RequestPath
        );

        stats.record(std::time::Duration::from_micros(501));
        assert_eq!(
            stats.recommendation(std::time::Duration::from_micros(500)),
            RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing
        );
    }

    #[test]
    fn routed_input_transport_keeps_x11_when_shm_is_not_recommended() {
        assert_eq!(
            select_routed_input_transport(
                RoutedInputOptimizationRecommendation::KeepX11RequestPath,
                SharedMemoryRouteRingState::Available
            ),
            RoutedInputTransport::X11Request
        );
    }

    #[test]
    fn routed_input_transport_selects_shm_only_when_available_and_recommended() {
        assert_eq!(
            select_routed_input_transport(
                RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing,
                SharedMemoryRouteRingState::Available
            ),
            RoutedInputTransport::SharedMemoryRing
        );
    }

    #[test]
    fn routed_input_transport_falls_back_to_x11_when_shm_is_unavailable_or_failed() {
        for shm_state in [
            SharedMemoryRouteRingState::Unavailable,
            SharedMemoryRouteRingState::Failed,
        ] {
            assert_eq!(
                select_routed_input_transport(
                    RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing,
                    shm_state
                ),
                RoutedInputTransport::X11Request
            );
        }
    }

    fn input_event(serial: u64) -> InputEventPacket {
        InputEventPacket {
            serial,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: 1_000,
            kind: InputEventKind::PointerButton {
                button: 1,
                pressed: true,
            },
            global_position: Some(Point { x: 100.0, y: 200.0 }),
            target_surface: Some(SurfaceId::new(3, 1)),
            target_window: Some(xid(0x30)),
            local_position: Some(Point { x: 12.0, y: 8.0 }),
        }
    }

    fn input_route(
        serial: u64,
        outcome: InputRouteOutcome,
        target_window: Option<XWindowId>,
        local_position: Option<Point>,
        transform: Transform,
    ) -> InputRoute {
        InputRoute {
            input_serial: serial,
            target_surface: Some(SurfaceId::new(3, 1)),
            target_window,
            global_position: Point { x: 100.0, y: 200.0 },
            local_position,
            transform,
            outcome,
        }
    }
}

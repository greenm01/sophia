mod support;

use support::*;

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
fn selection_request_converts_to_cross_namespace_clipboard_import() {
    let mut state = XMirrorState::default();
    let mut owner = mirror(0x20, None, 0);
    owner.namespace = Some(NamespaceId::from_raw(7));
    let mut requestor = mirror(0x44, None, 0);
    requestor.namespace = Some(NamespaceId::from_raw(9));
    state.ingest_window(owner);
    state.ingest_window(requestor);

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
    let event = selection_request(0x20, 0x44);

    let request = clipboard_portal_request_from_selection_request(
        &event,
        "UTF8_STRING",
        &monitor,
        &state,
        PortalTransferId::from_raw(12),
    )
    .unwrap();

    assert_eq!(request.request.transfer, PortalTransferId::from_raw(12));
    assert_eq!(request.request.source_namespace, NamespaceId::from_raw(7));
    assert_eq!(request.request.target_namespace, NamespaceId::from_raw(9));
    assert_eq!(request.request.generation, 1);
    assert_eq!(
        request.request.target,
        ClipboardTarget::Atom("UTF8_STRING".to_owned())
    );
    assert_eq!(request.failure.requestor, 0x44);
    assert_eq!(request.failure.selection, 0x100);
    assert_eq!(request.failure.target, 0x200);
    assert_eq!(request.property, 0x300);
}

#[test]
fn selection_request_without_requestor_namespace_fails_closed() {
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

    assert_eq!(
        clipboard_portal_request_from_selection_request(
            &selection_request(0x20, 0x44),
            "UTF8_STRING",
            &monitor,
            &state,
            PortalTransferId::from_raw(12),
        ),
        Err(ClipboardSelectionRequestError::UnknownRequestorNamespace)
    );
}

#[test]
fn same_namespace_selection_request_bypasses_cross_namespace_portal() {
    let mut state = XMirrorState::default();
    let mut owner = mirror(0x20, None, 0);
    owner.namespace = Some(NamespaceId::from_raw(7));
    let mut requestor = mirror(0x44, None, 0);
    requestor.namespace = Some(NamespaceId::from_raw(7));
    state.ingest_window(owner);
    state.ingest_window(requestor);

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

    assert_eq!(
        clipboard_portal_request_from_selection_request(
            &selection_request(0x20, 0x44),
            "UTF8_STRING",
            &monitor,
            &state,
            PortalTransferId::from_raw(12),
        ),
        Err(ClipboardSelectionRequestError::SameNamespace)
    );
}

#[test]
fn live_selection_request_event_dispatches_into_clipboard_portal() {
    let mut state = XMirrorState::default();
    let mut owner = mirror(0x20, None, 0);
    owner.namespace = Some(NamespaceId::from_raw(7));
    let mut requestor = mirror(0x44, None, 0);
    requestor.namespace = Some(NamespaceId::from_raw(9));
    state.ingest_window(owner);
    state.ingest_window(requestor);

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
    let mut portal = ClipboardPortal::new();

    let dispatch = dispatch_clipboard_selection_request_event(
        &selection_request_event(0x20, 0x44),
        "UTF8_STRING",
        &monitor,
        &state,
        PortalTransferId::from_raw(12),
        &mut portal,
    )
    .unwrap();

    let PortalCommand::PromptClipboardTransfer(transfer) = dispatch.command else {
        panic!("expected clipboard prompt");
    };
    assert_eq!(transfer.transfer, PortalTransferId::from_raw(12));
    assert_eq!(transfer.source_namespace, NamespaceId::from_raw(7));
    assert_eq!(transfer.target_namespace, NamespaceId::from_raw(9));
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(12))
            .map(|transfer| transfer.generation),
        Some(1)
    );
    assert_eq!(dispatch.portal_request.failure.requestor, 0x44);
}

#[test]
fn live_selection_request_dispatch_rejects_non_selection_events() {
    let mut portal = ClipboardPortal::new();

    assert_eq!(
        dispatch_clipboard_selection_request_event(
            &Event::MapNotify(x11rb::protocol::xproto::MapNotifyEvent {
                response_type: 0,
                sequence: 1,
                event: 0x01,
                window: 0x02,
                override_redirect: false,
            }),
            "UTF8_STRING",
            &XSelectionMonitor::new(),
            &XMirrorState::default(),
            PortalTransferId::from_raw(12),
            &mut portal,
        ),
        Err(ClipboardSelectionDispatchError::NotSelectionRequest)
    );
}

#[test]
fn live_selection_request_dispatch_keeps_portal_target_validation() {
    let mut state = XMirrorState::default();
    let mut owner = mirror(0x20, None, 0);
    owner.namespace = Some(NamespaceId::from_raw(7));
    let mut requestor = mirror(0x44, None, 0);
    requestor.namespace = Some(NamespaceId::from_raw(9));
    state.ingest_window(owner);
    state.ingest_window(requestor);

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
    let mut portal = ClipboardPortal::new();

    assert_eq!(
        dispatch_clipboard_selection_request_event(
            &selection_request_event(0x20, 0x44),
            "image/png",
            &monitor,
            &state,
            PortalTransferId::from_raw(12),
            &mut portal,
        ),
        Err(ClipboardSelectionDispatchError::Portal(
            PortalError::UnsupportedTarget
        ))
    );
    assert_eq!(portal.transfer(PortalTransferId::from_raw(12)), None);
}

#[test]
fn approved_clipboard_handoff_builds_bounded_text_property_and_success_notify() {
    let mut state = XMirrorState::default();
    let mut owner = mirror(0x20, None, 0);
    owner.namespace = Some(NamespaceId::from_raw(7));
    let mut requestor = mirror(0x44, None, 0);
    requestor.namespace = Some(NamespaceId::from_raw(9));
    state.ingest_window(owner);
    state.ingest_window(requestor);

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
    let mut portal = ClipboardPortal::new();
    let dispatch = dispatch_clipboard_selection_request_event(
        &selection_request_event(0x20, 0x44),
        "UTF8_STRING",
        &monitor,
        &state,
        PortalTransferId::from_raw(12),
        &mut portal,
    )
    .unwrap();
    let command = portal
        .approve_generation(PortalTransferId::from_raw(12), 1)
        .unwrap();

    let handoff =
        clipboard_selection_text_handoff_notify(&command, &dispatch.portal_request, "hello")
            .unwrap();

    assert_eq!(handoff.transfer, PortalTransferId::from_raw(12));
    assert_eq!(handoff.property.requestor, 0x44);
    assert_eq!(handoff.property.property, 0x300);
    assert_eq!(handoff.property.target, 0x200);
    assert_eq!(handoff.property.bytes, b"hello");
    assert_eq!(handoff.event.property, 0x300);
    assert_eq!(handoff.event.requestor, 0x44);
    assert!(handoff.succeeded_normally());
}

#[test]
fn clipboard_handoff_rejects_mismatched_transfer() {
    let request = clipboard_portal_request(12, 0x300, "UTF8_STRING");
    let command = PortalCommand::HandoffClipboard {
        transfer: PortalTransferId::from_raw(13),
    };

    assert_eq!(
        clipboard_selection_text_handoff_notify(&command, &request, "hello").unwrap_err(),
        ClipboardSelectionHandoffError::TransferMismatch
    );
}

#[test]
fn clipboard_handoff_rejects_missing_success_property() {
    let request = clipboard_portal_request(12, 0, "UTF8_STRING");
    let command = PortalCommand::HandoffClipboard {
        transfer: PortalTransferId::from_raw(12),
    };

    assert_eq!(
        clipboard_selection_text_handoff_notify(&command, &request, "hello").unwrap_err(),
        ClipboardSelectionHandoffError::MissingProperty
    );
}

#[test]
fn clipboard_handoff_rejects_oversized_text_payload() {
    let request = clipboard_portal_request(12, 0x300, "UTF8_STRING");
    let command = PortalCommand::HandoffClipboard {
        transfer: PortalTransferId::from_raw(12),
    };
    let text = "x".repeat(MAX_CLIPBOARD_TEXT_HANDOFF_BYTES + 1);

    assert_eq!(
        clipboard_selection_text_handoff_notify(&command, &request, &text).unwrap_err(),
        ClipboardSelectionHandoffError::TextTooLarge {
            len: MAX_CLIPBOARD_TEXT_HANDOFF_BYTES + 1,
            max: MAX_CLIPBOARD_TEXT_HANDOFF_BYTES,
        }
    );
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

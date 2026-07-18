mod support;
use sophia_engine::{WmShortcutDecision, WmShortcutRegistry};
use sophia_protocol::{
    WM_API_VERSION, WmActionId, WmBindingRegistration, WmCapabilities, WmHello, WmModifierMask,
};
use support::*;

#[test]
fn wm_socket_transport_roundtrips_one_engine_minted_transaction() {
    let request = wm_request(TransactionId::from_raw(42));
    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: vec![WmCommand::FocusSurface(SurfaceId::new(1, 1))],
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());

    let decoded = request_wm_over_stream(&mut stream, &request).unwrap();

    assert_eq!(decoded, response);
    assert_eq!(decode_wm_request_frame(&stream.written).unwrap(), request);
}

#[test]
fn wm_socket_transport_rejects_transaction_mismatch() {
    let request = wm_request(TransactionId::from_raw(42));
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(43),
        commands: Vec::new(),
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());

    assert_eq!(
        request_wm_over_stream(&mut stream, &request),
        Err(WmIpcError::TransactionMismatch {
            expected: TransactionId::from_raw(42),
            actual: TransactionId::from_raw(43),
        })
    );
}

#[test]
fn wm_socket_transport_rejects_oversized_response_before_payload_read() {
    let request = wm_request(TransactionId::from_raw(42));
    let mut response = Vec::new();
    push_u32(&mut response, SOPHIA_IPC_MAGIC);
    push_u16(&mut response, SOPHIA_IPC_VERSION);
    push_u16(&mut response, 2);
    push_u64(&mut response, 42);
    push_u32(&mut response, (SOPHIA_IPC_MAX_PAYLOAD_LEN as u32) + 1);
    push_u32(&mut response, 0);
    assert_eq!(response.len(), SOPHIA_IPC_HEADER_LEN);
    let mut stream = TestDuplex::new(response);

    assert_eq!(
        request_wm_over_stream(&mut stream, &request),
        Err(WmIpcError::Codec(IpcCodecError::PayloadTooLarge(
            SOPHIA_IPC_MAX_PAYLOAD_LEN + 1
        )))
    );
}

#[test]
fn wm_transaction_helper_commits_valid_response() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(50));
    let surface = SurfaceId::new(0, 1);
    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: vec![WmCommand::RenderSurface(SurfacePlacement {
            surface,
            geometry: Rect {
                x: 50,
                y: 60,
                width: 700,
                height: 500,
            },
            z_index: 3,
            crop: None,
            transform: Transform::IDENTITY,
        })],
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert_eq!(update.ipc_error, None);
    assert_eq!(update.commit.outcome, TransactionOutcome::Committed);
    assert_eq!(layers[0].geometry.x, 50);
    assert_eq!(layers[0].geometry.width, 700);
}

#[test]
fn wm_transaction_helper_preserves_layout_on_malformed_response() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(51));
    let mut bad_response = encode_wm_response_frame(&WmResponsePacket {
        transaction: request.transaction,
        commands: Vec::new(),
        timeout_msec: 250,
    })
    .unwrap();
    bad_response[0] = 0;
    let mut stream = TestDuplex::new(bad_response);
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];
    let before = layers.clone();

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert_eq!(update.commit.transaction, request.transaction);
    assert_eq!(update.commit.outcome, TransactionOutcome::TimedOut);
    assert!(matches!(
        update.ipc_error,
        Some(WmIpcError::Codec(IpcCodecError::BadMagic))
    ));
    assert_eq!(layers, before);
}

#[test]
fn wm_transaction_helper_preserves_layout_on_missing_response() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(52));
    let mut stream = TestDuplex::new(Vec::new());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];
    let before = layers.clone();

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert_eq!(update.commit.transaction, request.transaction);
    assert_eq!(update.commit.outcome, TransactionOutcome::TimedOut);
    assert!(matches!(update.ipc_error, Some(WmIpcError::Io(_))));
    assert_eq!(layers, before);
}

#[test]
fn wm_transaction_cache_records_committed_layout() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(53));
    let surface = SurfaceId::new(0, 1);
    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: vec![WmCommand::RenderSurface(SurfacePlacement {
            surface,
            geometry: Rect {
                x: 90,
                y: 100,
                width: 640,
                height: 480,
            },
            z_index: 4,
            crop: None,
            transform: Transform::IDENTITY,
        })],
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];
    let mut cache = LastCommittedLayout::default();

    let update =
        engine.request_and_cache_wm_transaction(&mut stream, &request, &mut layers, &mut cache);

    assert_eq!(update.commit.outcome, TransactionOutcome::Committed);
    assert_eq!(cache.layers()[0].geometry.x, 90);
    assert_eq!(cache.layers()[0].geometry.width, 640);
}

#[test]
fn wm_transaction_cache_restores_last_committed_layout_when_wm_is_absent() {
    let engine = HeadlessEngine::default();
    let cached = test_layer(0, 0, 9, Region::empty());
    let mut cache = LastCommittedLayout::new(vec![cached.clone()]);
    let request = wm_request(TransactionId::from_raw(54));
    let mut stream = TestDuplex::new(Vec::new());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];

    let update =
        engine.request_and_cache_wm_transaction(&mut stream, &request, &mut layers, &mut cache);

    assert_eq!(update.commit.outcome, TransactionOutcome::TimedOut);
    assert!(matches!(update.ipc_error, Some(WmIpcError::Io(_))));
    assert_eq!(layers, vec![cached]);
    assert_eq!(cache.layers(), layers.as_slice());
}

#[test]
fn session_tick_records_fresh_layers_and_replays_frame() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let layers = vec![test_layer(0, 0, 0, Region::empty())];
    let mut cache = LastCommittedLayout::default();

    let report = engine
        .run_session_tick(
            SessionTickRequest {
                output: output.id,
                frame_serial: 70,
                layers: SessionLayerSource::Fresh(layers.clone()),
            },
            &mut cache,
        )
        .unwrap();

    assert!(!report.restored_last_committed);
    assert_eq!(report.frame.frame_serial, 70);
    assert_eq!(report.replay.steps.len(), 1);
    assert_eq!(cache.layers(), layers.as_slice());
}

#[test]
fn session_tick_restores_cached_layout_when_requested() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let cached = vec![test_layer(0, 0, 5, Region::empty())];
    let mut cache = LastCommittedLayout::new(cached.clone());

    let report = engine
        .run_session_tick(
            SessionTickRequest {
                output: output.id,
                frame_serial: 71,
                layers: SessionLayerSource::RestoreLastCommitted,
            },
            &mut cache,
        )
        .unwrap();

    assert!(report.restored_last_committed);
    assert_eq!(report.frame.layers, cached);
    assert_eq!(report.replay.steps.len(), 1);
}

#[test]
fn wm_shortcuts_validate_and_suppress_repeats_until_release() {
    let action = WmActionId::from_raw(7);
    let hello = WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        bindings: vec![WmBindingRegistration {
            action,
            keycode: 28,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
    };
    let mut shortcuts = WmShortcutRegistry::from_hello(&hello).unwrap();

    assert_eq!(shortcuts.binding_count(), 1);
    assert_eq!(
        shortcuts.handle_key(
            28,
            WmModifierMask {
                bits: WmModifierMask::SUPER
            },
            true
        ),
        WmShortcutDecision {
            action: Some(action),
            consumed: true
        }
    );
    assert_eq!(
        shortcuts.handle_key(
            28,
            WmModifierMask {
                bits: WmModifierMask::SUPER
            },
            true
        ),
        WmShortcutDecision {
            action: None,
            consumed: true
        }
    );
    assert!(
        shortcuts
            .handle_key(28, WmModifierMask { bits: 0 }, false)
            .consumed
    );
    assert_eq!(
        shortcuts
            .handle_key(
                28,
                WmModifierMask {
                    bits: WmModifierMask::SUPER
                },
                true
            )
            .action,
        Some(action)
    );
}

#[test]
fn wm_shortcuts_reject_the_emergency_chord() {
    let hello = WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        bindings: vec![WmBindingRegistration {
            action: WmActionId::from_raw(1),
            keycode: 14,
            modifiers: WmModifierMask {
                bits: WmModifierMask::CONTROL | WmModifierMask::ALT,
            },
        }],
    };

    assert_eq!(
        WmShortcutRegistry::from_hello(&hello),
        Err(WmIpcError::Negotiation("reserved emergency chord"))
    );
}

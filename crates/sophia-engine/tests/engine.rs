use sophia_engine::{
    BufferImportPath, ChromeActionDecision, ChromeActionRejectReason, ChromeBroker,
    DeterministicFrameClock, DrmKmsMode, DrmKmsOutputDescriptor, DrmKmsOutputRegistry, EngineError,
    FrameClock, FramePlanRequest, FrameScheduleDecision, HeadlessEngine, ImportCapableRenderer,
    ImportedBufferHandle, LastCommittedLayout, LayoutEpochState, LibinputDeviceDescriptor,
    LibinputDeviceKind, LibinputEventIngest, LibinputEventSource, MetadataChromeRejectReason,
    MetadataChromeUpdate, NotificationChromePresenter, NotificationChromeRejectReason,
    NotificationChromeUpdate, RoutedInputCoalescer, RoutedInputFlushReason, RoutedInputQueueAction,
    RoutedInputRequestError, SanitizedChromeMetadata, SessionCommand, SessionEvent,
    SessionLayerSource, SessionTickRequest, WmIpcError, WmRestartReason, WmRuntimeAction,
    measure_resize_behavior, notification_chrome_command_from_portal, request_wm_over_stream,
    routed_input_request_from_physical_event, routed_input_requests_from_flush,
    schedule_frame_from_damage, update_wm_supervisor_from_runtime_action,
};
use sophia_portal::{NotificationRequest, NotificationUrgency, PortalCommand};
use sophia_protocol::{
    AttentionState, BufferSource, ChromeActionKind, ChromeActionRequest, ChromeDescriptor,
    DamageFrame, DeviceId, DisplayLabel, IconTokenId, InputEventKind, InputEventPacket, InputRoute,
    InputRouteOutcome, IpcCodecError, LayerSnapshot, LayoutNodeCapabilities, LayoutNodeKind,
    LayoutNodeSnapshot, LayoutNodeState, LayoutTransaction, NamespaceId, OutputId, Point,
    PortalTransferId, Rect, Region, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAGIC,
    SOPHIA_IPC_MAX_PAYLOAD_LEN, SOPHIA_IPC_VERSION, SeatId, SurfaceConstraints, SurfaceId,
    SurfacePlacement, TransactionId, TransactionOutcome, Transform, TrustLevel, WmCommand,
    WmRequestKind, WmRequestPacket, WmResponsePacket, WorkspaceId, XWindowId,
    decode_wm_request_frame, encode_wm_response_frame,
};
use sophia_runtime::{RestartPolicy, SupervisedProcessKind, SupervisorCommand, SupervisorState};
use std::io::{Cursor, Read, Result as IoResult, Write};
use std::time::Duration;

#[test]
fn headless_engine_exposes_deterministic_output() {
    let engine = HeadlessEngine::default();
    let output = engine.output();

    assert_eq!(output.id, OutputId::from_raw(1));
    assert_eq!(
        output.size,
        sophia_protocol::Size {
            width: 1280,
            height: 720,
        }
    );
    assert_eq!(output.scale, 1);
}

#[test]
fn drm_kms_output_registry_tracks_connector_mode_and_scale() {
    let descriptor = DrmKmsOutputDescriptor {
        output: OutputId::from_raw(7),
        connector_id: 42,
        crtc_id: 99,
        mode: DrmKmsMode::new(1920, 1080, 60_000),
        scale: 2,
    };
    let mut registry = DrmKmsOutputRegistry::new();

    registry.upsert(descriptor);

    assert_eq!(registry.get(OutputId::from_raw(7)), Some(&descriptor));
    assert_eq!(registry.outputs().count(), 1);
    assert_eq!(
        registry.primary_engine_output(),
        Some(descriptor.as_engine_output())
    );
    assert_eq!(
        descriptor.as_engine_output().size,
        sophia_protocol::Size {
            width: 1920,
            height: 1080,
        }
    );
    assert_eq!(descriptor.as_engine_output().scale, 2);
    assert_eq!(registry.remove(OutputId::from_raw(7)), Some(descriptor));
    assert!(registry.primary_engine_output().is_none());
}

#[test]
fn drm_kms_descriptor_can_seed_engine_output() {
    let descriptor = DrmKmsOutputDescriptor {
        output: OutputId::from_raw(8),
        connector_id: 43,
        crtc_id: 100,
        mode: DrmKmsMode::new(2560, 1440, 144_000),
        scale: 1,
    };
    let engine = HeadlessEngine::new(descriptor.as_engine_output());
    let frame = engine
        .plan_frame(
            FramePlanRequest {
                output: OutputId::from_raw(8),
                frame_serial: 1,
            },
            Vec::new(),
        )
        .unwrap();

    assert_eq!(frame.output_size, descriptor.mode.size);
    assert_eq!(frame.output_scale, descriptor.scale);
}

#[test]
fn libinput_event_source_accepts_registered_device_events_in_order() {
    let mut source = LibinputEventSource::new();
    let device = LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    };
    source.register_device(device);

    assert_eq!(source.device(DeviceId::from_raw(2)), Some(&device));
    assert_eq!(source.devices().count(), 1);
    assert_eq!(
        source.push_event(motion_event(1, 10.0, 20.0)),
        LibinputEventIngest::Accepted
    );
    assert_eq!(
        source.push_event(motion_event(2, 11.0, 21.0)),
        LibinputEventIngest::Accepted
    );

    let events = source.drain_events();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].serial, 1);
    assert_eq!(events[1].serial, 2);
    assert_eq!(source.pending_len(), 0);
    assert_eq!(source.remove_device(DeviceId::from_raw(2)), Some(device));
}

#[test]
fn libinput_event_source_rejects_unknown_or_wrong_seat_events() {
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(9),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Keyboard,
    });

    assert_eq!(
        source.push_event(motion_event(1, 0.0, 0.0)),
        LibinputEventIngest::SeatMismatch
    );

    let mut unknown_device_event = motion_event(2, 0.0, 0.0);
    unknown_device_event.device = DeviceId::from_raw(99);
    assert_eq!(
        source.push_event(unknown_device_event),
        LibinputEventIngest::UnknownDevice
    );
    assert_eq!(source.pending_len(), 0);
}

#[test]
fn headless_engine_returns_frame_value() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let request = FramePlanRequest {
        output: output.id,
        frame_serial: 7,
    };
    let frame = engine.plan_frame(request, Vec::new()).unwrap();

    assert_eq!(frame.output, request.output);
    assert_eq!(frame.output_size, output.size);
    assert_eq!(frame.output_scale, output.scale);
    assert_eq!(frame.frame_serial, 7);
    assert!(frame.layers.is_empty());
    assert!(frame.commands.is_empty());
}

#[test]
fn frame_plan_sorts_layers_by_stack_rank() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 1,
    };
    let frame = engine
        .plan_frame(
            request,
            vec![
                test_layer(0, 20, 20, Region::empty()),
                test_layer(1, 10, 10, Region::empty()),
            ],
        )
        .unwrap();

    assert_eq!(frame.layers[0].stack_rank, 10);
    assert_eq!(frame.layers[1].stack_rank, 20);
    assert_eq!(frame.commands[0].source, Some(frame.layers[0].surface));
}

#[test]
fn frame_plan_aggregates_layer_damage() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 1,
    };
    let frame = engine
        .plan_frame(
            request,
            vec![
                test_layer(
                    0,
                    0,
                    0,
                    Region::single(Rect {
                        x: 0,
                        y: 0,
                        width: 10,
                        height: 10,
                    }),
                ),
                test_layer(
                    1,
                    1,
                    100,
                    Region::single(Rect {
                        x: 100,
                        y: 0,
                        width: 5,
                        height: 5,
                    }),
                ),
            ],
        )
        .unwrap();

    assert_eq!(frame.damage.rects.len(), 2);
}

#[test]
fn frame_plan_rejects_stale_surface() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 1,
    };
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.surface = SurfaceId::INVALID;

    assert_eq!(
        engine.plan_frame(request, vec![layer]),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn frame_snapshot_replays_with_mock_surfaces() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 11,
    };
    let frame = engine
        .plan_frame(
            request,
            vec![
                test_layer(0, 0, 0, Region::empty()),
                test_layer(1, 1, 100, Region::empty()),
            ],
        )
        .unwrap();

    let replay = engine.replay_frame(&frame).unwrap();

    assert_eq!(replay.output, engine.output().id);
    assert_eq!(replay.output_size, engine.output().size);
    assert_eq!(replay.output_scale, engine.output().scale);
    assert_eq!(replay.frame_serial, 11);
    assert_eq!(replay.steps.len(), 2);
    assert_eq!(replay.steps[0].source, Some(frame.layers[0].surface));
}

#[test]
fn layout_transaction_moves_and_resizes_layers_atomically() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(0, 1);
    let layers = vec![test_layer(0, 0, 0, Region::empty())];
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(1),
        requested_sizes: Vec::new(),
        focus: Some(surface),
        render_positions: vec![SurfacePlacement {
            surface,
            geometry: Rect {
                x: 25,
                y: 30,
                width: 400,
                height: 300,
            },
            z_index: 7,
            crop: None,
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };

    let committed = engine
        .apply_layout_transaction(&transaction, layers)
        .unwrap();

    assert_eq!(committed[0].geometry.x, 25);
    assert_eq!(committed[0].geometry.width, 400);
    assert_eq!(committed[0].stack_rank, 7);
    assert_eq!(committed[0].generation, 2);
    assert_eq!(committed[0].damage.rects.len(), 2);
}

#[test]
fn layout_transaction_rejects_unknown_surfaces() {
    let engine = HeadlessEngine::default();
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(1),
        requested_sizes: Vec::new(),
        focus: None,
        render_positions: vec![SurfacePlacement {
            surface: SurfaceId::new(99, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            z_index: 0,
            crop: None,
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };

    assert_eq!(
        engine.apply_layout_transaction(&transaction, vec![test_layer(0, 0, 0, Region::empty())]),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn commit_layout_transaction_reports_outcome() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(0, 1);
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(44),
        requested_sizes: Vec::new(),
        focus: Some(surface),
        render_positions: vec![SurfacePlacement {
            surface,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 500,
                height: 400,
            },
            z_index: 1,
            crop: Some(Rect {
                x: 0,
                y: 0,
                width: 250,
                height: 200,
            }),
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };

    let commit = engine.commit_layout_transaction(&transaction, &mut layers);

    assert_eq!(commit.transaction, TransactionId::from_raw(44));
    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(commit.applied_surfaces, vec![surface]);
    assert_eq!(
        layers[0].crop,
        Some(Rect {
            x: 0,
            y: 0,
            width: 250,
            height: 200,
        })
    );
}

#[test]
fn absent_wm_preserves_committed_layers() {
    let engine = HeadlessEngine::default();
    let layers = vec![test_layer(0, 0, 0, Region::empty())];
    let before = layers.clone();

    let commit = engine.preserve_layout_on_wm_absent(TransactionId::from_raw(45), &layers);

    assert_eq!(commit.transaction, TransactionId::from_raw(45));
    assert_eq!(commit.outcome, TransactionOutcome::TimedOut);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(layers, before);
}

#[test]
fn frame_snapshot_replay_rejects_unknown_surface() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 12,
    };
    let mut frame = engine
        .plan_frame(request, vec![test_layer(0, 0, 0, Region::empty())])
        .unwrap();
    frame.commands[0].source = Some(SurfaceId::new(99, 1));

    assert_eq!(
        engine.replay_frame(&frame),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn render_frame_reports_cpu_fallback_imports() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 13,
    };
    let cpu_layer = test_layer(0, 0, 0, Region::empty());
    let mut dma_layer = test_layer(1, 1, 100, Region::empty());
    dma_layer.source = BufferSource::DmaBuf { handle: 99 };

    let frame = engine
        .plan_frame(request, vec![cpu_layer, dma_layer])
        .unwrap();
    let rendered = engine.render_frame(&frame).unwrap();

    assert_eq!(rendered.replay.frame_serial, 13);
    assert_eq!(rendered.replay.steps.len(), 2);
    assert_eq!(rendered.imports.len(), 2);
    assert_eq!(rendered.imports[0].requested, BufferImportPath::CpuReadback);
    assert_eq!(rendered.imports[0].used, BufferImportPath::CpuReadback);
    assert_eq!(
        rendered.imports[0].handle,
        ImportedBufferHandle::CpuReadback {
            source: rendered.imports[0].source
        }
    );
    assert!(!rendered.imports[0].used_fallback);
    assert_eq!(rendered.imports[1].requested, BufferImportPath::DmaBuf);
    assert_eq!(rendered.imports[1].used, BufferImportPath::CpuReadback);
    assert_eq!(
        rendered.imports[1].handle,
        ImportedBufferHandle::CpuReadback {
            source: BufferSource::DmaBuf { handle: 99 }
        }
    );
    assert!(rendered.imports[1].used_fallback);
}

#[test]
fn import_capable_renderer_uses_native_buffer_handles_when_supported() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 15,
    };
    let mut xpixmap_layer = test_layer(0, 0, 0, Region::empty());
    xpixmap_layer.source = BufferSource::XPixmap { pixmap: 44 };
    let mut dmabuf_layer = test_layer(1, 1, 100, Region::empty());
    dmabuf_layer.source = BufferSource::DmaBuf { handle: 99 };
    let renderer = ImportCapableRenderer::new(true, true);

    let frame = engine
        .plan_frame(request, vec![xpixmap_layer, dmabuf_layer])
        .unwrap();
    let rendered = engine.render_frame_with(&renderer, &frame).unwrap();

    assert_eq!(rendered.imports.len(), 2);
    assert_eq!(rendered.imports[0].requested, BufferImportPath::XPixmap);
    assert_eq!(rendered.imports[0].used, BufferImportPath::XPixmap);
    assert_eq!(
        rendered.imports[0].handle,
        ImportedBufferHandle::XPixmap { pixmap: 44 }
    );
    assert!(!rendered.imports[0].used_fallback);
    assert_eq!(rendered.imports[1].requested, BufferImportPath::DmaBuf);
    assert_eq!(rendered.imports[1].used, BufferImportPath::DmaBuf);
    assert_eq!(
        rendered.imports[1].handle,
        ImportedBufferHandle::DmaBuf { handle: 99 }
    );
    assert!(!rendered.imports[1].used_fallback);
}

#[test]
fn import_capable_renderer_falls_back_for_unsupported_handles() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 16,
    };
    let mut xpixmap_layer = test_layer(0, 0, 0, Region::empty());
    xpixmap_layer.source = BufferSource::XPixmap { pixmap: 44 };
    let mut dmabuf_layer = test_layer(1, 1, 100, Region::empty());
    dmabuf_layer.source = BufferSource::DmaBuf { handle: 99 };
    let renderer = ImportCapableRenderer::new(false, true);

    let frame = engine
        .plan_frame(request, vec![xpixmap_layer, dmabuf_layer])
        .unwrap();
    let rendered = engine.render_frame_with(&renderer, &frame).unwrap();

    assert_eq!(rendered.imports[0].requested, BufferImportPath::XPixmap);
    assert_eq!(rendered.imports[0].used, BufferImportPath::CpuReadback);
    assert!(rendered.imports[0].used_fallback);
    assert_eq!(rendered.imports[1].requested, BufferImportPath::DmaBuf);
    assert_eq!(rendered.imports[1].used, BufferImportPath::DmaBuf);
    assert!(!rendered.imports[1].used_fallback);
}

#[test]
fn render_frame_reuses_replay_validation() {
    let engine = HeadlessEngine::default();
    let request = FramePlanRequest {
        output: engine.output().id,
        frame_serial: 14,
    };
    let mut frame = engine
        .plan_frame(request, vec![test_layer(0, 0, 0, Region::empty())])
        .unwrap();
    frame.commands[0].source = Some(SurfaceId::new(99, 1));

    assert_eq!(
        engine.render_frame(&frame).map(|report| report.imports),
        Err(EngineError::InvalidSurface)
    );
}

#[test]
fn chrome_broker_keeps_metadata_separate_from_layout() {
    let mut broker = ChromeBroker::default();
    let surface = SurfaceId::new(3, 1);

    broker.upsert(ChromeDescriptor {
        surface,
        label: Some(DisplayLabel {
            text: "Redacted Title".to_owned(),
            redacted: true,
        }),
        icon: Some(IconTokenId::from_raw(12)),
        trust_level: TrustLevel::Isolated,
        attention: AttentionState::None,
        generation: 4,
    });

    let descriptor = broker.get(surface).unwrap();

    assert_eq!(broker.len(), 1);
    assert_eq!(
        descriptor.label.as_ref().map(|label| label.redacted),
        Some(true)
    );
    assert_eq!(descriptor.icon, Some(IconTokenId::from_raw(12)));
    assert_eq!(descriptor.trust_level, TrustLevel::Isolated);
}

#[test]
fn chrome_broker_removes_surface_metadata() {
    let mut broker = ChromeBroker::default();
    let surface = SurfaceId::new(4, 1);

    broker.upsert(ChromeDescriptor {
        surface,
        label: None,
        icon: None,
        trust_level: TrustLevel::Unknown,
        attention: AttentionState::None,
        generation: 1,
    });

    assert!(broker.remove_surface(surface).is_some());
    assert!(broker.get(surface).is_none());
    assert!(broker.is_empty());
}

#[test]
fn metadata_broker_output_updates_chrome_descriptor() {
    let mut broker = ChromeBroker::default();
    let surface = SurfaceId::new(5, 1);

    assert_eq!(
        broker.apply_metadata(SanitizedChromeMetadata {
            surface,
            label: Some("Untrusted Browser".to_owned()),
            label_redacted: true,
            icon: Some(IconTokenId::from_raw(7)),
            trust_level: TrustLevel::Untrusted,
            attention: AttentionState::Notice,
            generation: 3,
        }),
        MetadataChromeUpdate::Upserted { surface }
    );

    let descriptor = broker.get(surface).unwrap();
    assert_eq!(descriptor.surface, surface);
    assert_eq!(
        descriptor.label.as_ref(),
        Some(&DisplayLabel {
            text: "Untrusted Browser".to_owned(),
            redacted: true,
        })
    );
    assert_eq!(descriptor.icon, Some(IconTokenId::from_raw(7)));
    assert_eq!(descriptor.trust_level, TrustLevel::Untrusted);
    assert_eq!(descriptor.attention, AttentionState::Notice);
    assert_eq!(descriptor.generation, 3);
}

#[test]
fn metadata_broker_output_rejects_stale_generation() {
    let mut broker = ChromeBroker::default();
    let surface = SurfaceId::new(6, 1);

    broker.apply_metadata(metadata(surface, "Current", 9));
    let update = broker.apply_metadata(metadata(surface, "Old", 8));

    assert_eq!(
        update,
        MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration)
    );
    assert_eq!(
        broker
            .get(surface)
            .and_then(|descriptor| descriptor.label.as_ref())
            .map(|label| label.text.as_str()),
        Some("Current")
    );
}

#[test]
fn metadata_broker_output_rejects_unsanitized_label() {
    let mut broker = ChromeBroker::default();
    let surface = SurfaceId::new(7, 1);
    let mut metadata = metadata(surface, "Bad\nTitle", 1);
    metadata.label_redacted = false;

    let update = broker.apply_metadata(metadata);

    assert_eq!(
        update,
        MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel)
    );
    assert!(broker.get(surface).is_none());
}

#[test]
fn metadata_broker_removal_clears_descriptor_with_generation_check() {
    let mut broker = ChromeBroker::default();
    let surface = SurfaceId::new(8, 1);

    broker.apply_metadata(metadata(surface, "Visible", 4));
    assert_eq!(
        broker.remove_metadata(surface, 3),
        MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration)
    );
    assert!(broker.get(surface).is_some());

    assert_eq!(
        broker.remove_metadata(surface, 4),
        MetadataChromeUpdate::Removed { surface }
    );
    assert!(broker.get(surface).is_none());
}

#[test]
fn notification_chrome_presents_only_after_delivery_command() {
    let mut presenter = NotificationChromePresenter::new();
    let request = notification_request(42);
    let transfer = request.transfer;

    assert_eq!(
        presenter.stage_request(&request),
        NotificationChromeUpdate::Staged { transfer }
    );
    assert!(presenter.pending(transfer).is_some());
    assert!(presenter.visible(transfer).is_none());

    let update = presenter.apply_portal_command(&PortalCommand::DeliverNotification { transfer });

    assert_eq!(update, NotificationChromeUpdate::Presented { transfer });
    assert!(presenter.pending(transfer).is_none());
    let visible = presenter.visible(transfer).unwrap();
    assert_eq!(visible.summary, "Build finished");
    assert_eq!(visible.body.as_deref(), Some("Sophia smoke completed"));
    assert_eq!(visible.urgency, NotificationUrgency::Normal);
}

#[test]
fn notification_chrome_drop_dismisses_pending_notification() {
    let mut presenter = NotificationChromePresenter::new();
    let request = notification_request(43);
    let transfer = request.transfer;

    presenter.stage_request(&request);
    let update = presenter.apply_portal_command(&PortalCommand::DropNotification { transfer });

    assert_eq!(update, NotificationChromeUpdate::Dismissed { transfer });
    assert!(presenter.pending(transfer).is_none());
    assert!(presenter.visible(transfer).is_none());
}

#[test]
fn notification_chrome_rejects_unknown_delivery() {
    let mut presenter = NotificationChromePresenter::new();
    let transfer = PortalTransferId::from_raw(99);

    let update = presenter.apply_portal_command(&PortalCommand::DeliverNotification { transfer });

    assert_eq!(
        update,
        NotificationChromeUpdate::Rejected(NotificationChromeRejectReason::UnknownTransfer)
    );
}

#[test]
fn notification_chrome_ignores_unrelated_portal_commands() {
    let transfer = PortalTransferId::from_raw(12);

    assert_eq!(
        notification_chrome_command_from_portal(&PortalCommand::HandoffClipboard { transfer }),
        None
    );
}

#[test]
fn chrome_close_request_validates_generation_and_closability() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(9, 1);
    let nodes = vec![layout_node(surface, 3, true)];
    let request = ChromeActionRequest {
        surface,
        generation: 3,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    assert_eq!(
        engine.validate_chrome_action(&request, &nodes),
        ChromeActionDecision::RequestPoliteClose { surface }
    );
}

#[test]
fn chrome_close_request_rejects_unknown_surface() {
    let engine = HeadlessEngine::default();
    let request = ChromeActionRequest {
        surface: SurfaceId::new(99, 1),
        generation: 1,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    assert_eq!(
        engine.validate_chrome_action(&request, &[]),
        ChromeActionDecision::Rejected(ChromeActionRejectReason::UnknownSurface)
    );
}

#[test]
fn chrome_close_request_rejects_stale_generation() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(10, 1);
    let nodes = vec![layout_node(surface, 7, true)];
    let request = ChromeActionRequest {
        surface,
        generation: 6,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    assert_eq!(
        engine.validate_chrome_action(&request, &nodes),
        ChromeActionDecision::Rejected(ChromeActionRejectReason::StaleGeneration)
    );
}

#[test]
fn chrome_close_request_rejects_non_closable_surface() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(11, 1);
    let nodes = vec![layout_node(surface, 2, false)];
    let request = ChromeActionRequest {
        surface,
        generation: 2,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    assert_eq!(
        engine.validate_chrome_action(&request, &nodes),
        ChromeActionDecision::Rejected(ChromeActionRejectReason::NotClosable)
    );
}

#[test]
fn session_event_routes_accepted_chrome_close_to_x_bridge_command() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(12, 1);
    let nodes = vec![layout_node(surface, 4, true)];
    let request = ChromeActionRequest {
        surface,
        generation: 4,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    let update = engine.handle_session_event(SessionEvent::ChromeAction(request), &nodes);

    assert_eq!(
        update.chrome_decision,
        Some(ChromeActionDecision::RequestPoliteClose { surface })
    );
    assert_eq!(
        update.commands,
        vec![SessionCommand::RequestPoliteClose { surface }]
    );
}

#[test]
fn session_event_does_not_emit_close_command_for_rejected_chrome_action() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(13, 1);
    let nodes = vec![layout_node(surface, 8, true)];
    let request = ChromeActionRequest {
        surface,
        generation: 7,
        kind: ChromeActionKind::CloseSurfaceRequested,
    };

    let update = engine.handle_session_event(SessionEvent::ChromeAction(request), &nodes);

    assert_eq!(
        update.chrome_decision,
        Some(ChromeActionDecision::Rejected(
            ChromeActionRejectReason::StaleGeneration
        ))
    );
    assert!(update.commands.is_empty());
}

#[test]
fn session_event_notifies_wm_only_after_surface_removed() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(14, 1);
    let workspace = WorkspaceId::from_raw(3);
    let transaction = TransactionId::from_raw(99);

    let update = engine.handle_session_event(
        SessionEvent::SurfaceRemoved {
            transaction,
            surface,
            workspace,
        },
        &[],
    );

    assert_eq!(update.chrome_decision, None);
    assert_eq!(update.commands.len(), 1);
    let SessionCommand::SendWmRequest(request) = &update.commands[0] else {
        panic!("expected WM request command");
    };
    assert_eq!(request.transaction, transaction);
    assert_eq!(
        request.kind,
        WmRequestKind::SurfaceRemoved { surface, workspace }
    );
}

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
fn deterministic_frame_clock_advances_serials_predictably() {
    let output = OutputId::from_raw(4);
    let mut clock = DeterministicFrameClock::new(5, 16);

    let first = clock.next_frame(output);
    let second = clock.next_frame(output);

    assert_eq!(first.output, output);
    assert_eq!(first.frame_serial, 5);
    assert_eq!(first.target_msec, 80);
    assert_eq!(second.output, output);
    assert_eq!(second.frame_serial, 6);
    assert_eq!(second.target_msec, 96);
    assert_eq!(clock.next_serial(), 7);
    assert_eq!(clock.frame_interval_msec(), 16);
}

#[test]
fn clocked_session_tick_uses_clock_serial_and_updates_cache() {
    let engine = HeadlessEngine::default();
    let layers = vec![test_layer(0, 0, 0, Region::empty())];
    let mut cache = LastCommittedLayout::default();
    let mut clock = DeterministicFrameClock::new(10, 16);

    let report = engine
        .run_clocked_session_tick(
            &mut clock,
            SessionLayerSource::Fresh(layers.clone()),
            &mut cache,
        )
        .unwrap();

    assert_eq!(report.frame.frame_serial, 10);
    assert_eq!(report.replay.frame_serial, 10);
    assert!(!report.restored_last_committed);
    assert_eq!(cache.layers(), layers.as_slice());
    assert_eq!(clock.next_serial(), 11);
}

#[test]
fn clocked_session_tick_can_restore_last_committed_layout() {
    let engine = HeadlessEngine::default();
    let cached = vec![test_layer(0, 0, 8, Region::empty())];
    let mut cache = LastCommittedLayout::new(cached.clone());
    let mut clock = DeterministicFrameClock::new(20, 16);

    let report = engine
        .run_clocked_session_tick(
            &mut clock,
            SessionLayerSource::RestoreLastCommitted,
            &mut cache,
        )
        .unwrap();

    assert_eq!(report.frame.frame_serial, 20);
    assert!(report.restored_last_committed);
    assert_eq!(report.frame.layers, cached);
    assert_eq!(clock.next_serial(), 21);
}

#[test]
fn frame_scheduler_waits_without_damage() {
    let tick = frame_tick(1);

    assert_eq!(
        schedule_frame_from_damage(tick, None, None),
        FrameScheduleDecision::WaitForDamage
    );
}

#[test]
fn frame_scheduler_waits_for_pending_layout_epoch_surfaces() {
    let tick = frame_tick(2);
    let surface_a = SurfaceId::new(1, 1);
    let surface_b = SurfaceId::new(2, 1);
    let mut epoch = LayoutEpochState::new(9, [surface_a, surface_b]);
    let damage = damage_frame(2, &[surface_a]);

    assert_eq!(
        schedule_frame_from_damage(tick, Some(damage), Some(&mut epoch)),
        FrameScheduleDecision::WaitForLayoutEpoch {
            epoch: 9,
            pending_surfaces: vec![surface_b],
        }
    );
    assert_eq!(epoch.pending_surfaces(), vec![surface_b]);
}

#[test]
fn frame_scheduler_renders_when_damage_completes_layout_epoch() {
    let tick = frame_tick(3);
    let surface = SurfaceId::new(1, 1);
    let mut epoch = LayoutEpochState::new(10, [surface]);
    let damage = damage_frame(3, &[surface]);

    let decision = schedule_frame_from_damage(tick, Some(damage.clone()), Some(&mut epoch));

    assert_eq!(
        decision,
        FrameScheduleDecision::Render {
            output: tick.output,
            frame_serial: 3,
            damage,
            completed_epoch: Some(10),
        }
    );
    assert!(epoch.is_complete());
}

#[test]
fn resize_behavior_sample_reports_completed_epoch() {
    let surface = SurfaceId::new(1, 1);
    let mut epoch = LayoutEpochState::with_timing(11, [surface], 100, 300);
    epoch.observe_damage(&damage_frame(4, &[surface]));

    let sample = measure_resize_behavior(&epoch, 180);

    assert_eq!(sample.epoch, 11);
    assert_eq!(sample.elapsed_msec, 80);
    assert_eq!(sample.timeout_msec, 300);
    assert!(sample.completed);
    assert!(!sample.timed_out);
    assert!(sample.pending_surfaces.is_empty());
}

#[test]
fn resize_behavior_sample_reports_slow_non_cooperative_epoch_timeout() {
    let surface = SurfaceId::new(1, 1);
    let epoch = LayoutEpochState::with_timing(12, [surface], 100, 250);

    let sample = measure_resize_behavior(&epoch, 351);

    assert_eq!(sample.epoch, 12);
    assert_eq!(sample.elapsed_msec, 251);
    assert_eq!(sample.timeout_msec, 250);
    assert!(!sample.completed);
    assert!(sample.timed_out);
    assert_eq!(sample.pending_surfaces, vec![surface]);
}

#[test]
fn wm_runtime_action_keeps_running_after_valid_response() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(60));
    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: Vec::new(),
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert_eq!(update.runtime_action(), WmRuntimeAction::KeepRunning);
}

#[test]
fn wm_runtime_action_restarts_after_ipc_failure() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(61));
    let mut stream = TestDuplex::new(Vec::new());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert!(matches!(
        update.runtime_action(),
        WmRuntimeAction::RestartWm {
            reason: WmRestartReason::IpcFailure(WmIpcError::Io(_))
        }
    ));
}

#[test]
fn wm_runtime_action_does_not_restart_for_valid_rejected_layout() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(62));
    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: vec![WmCommand::RenderSurface(SurfacePlacement {
            surface: SurfaceId::new(99, 1),
            geometry: Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            z_index: 0,
            crop: None,
            transform: Transform::IDENTITY,
        })],
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert_eq!(
        update.commit.outcome,
        TransactionOutcome::RejectedInvalidSurface
    );
    assert_eq!(update.runtime_action(), WmRuntimeAction::KeepRunning);
}

#[test]
fn wm_supervisor_adapter_keeps_supervisor_idle_when_wm_keeps_running() {
    let state = SupervisorState::new(SupervisedProcessKind::WindowManager);

    let (state, command) = update_wm_supervisor_from_runtime_action(
        state,
        WmRuntimeAction::KeepRunning,
        RestartPolicy::default(),
    );

    assert_eq!(state.restart_attempts, 0);
    assert_eq!(command, SupervisorCommand::None);
}

#[test]
fn wm_supervisor_adapter_restarts_wm_after_runtime_restart_action() {
    let state = SupervisorState::new(SupervisedProcessKind::WindowManager);

    let (state, command) = update_wm_supervisor_from_runtime_action(
        state,
        WmRuntimeAction::RestartWm {
            reason: WmRestartReason::IpcFailure(WmIpcError::Io("closed".to_owned())),
        },
        RestartPolicy {
            max_attempts: 2,
            initial_backoff: Duration::from_millis(25),
            max_backoff: Duration::from_millis(100),
        },
    );

    assert_eq!(state.restart_attempts, 1);
    assert_eq!(
        command,
        SupervisorCommand::StartProcess {
            process: SupervisedProcessKind::WindowManager,
            delay: Duration::ZERO
        }
    );
}

#[test]
fn routed_input_coalescer_keeps_latest_stable_motion_until_frame() {
    let mut coalescer = RoutedInputCoalescer::new();

    assert_eq!(
        coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0)),
        RoutedInputQueueAction::BufferedMotion
    );
    assert_eq!(
        coalescer.push(motion_event(2, 20.0, 20.0), route(2, 0x30, 20.0, 20.0)),
        RoutedInputQueueAction::BufferedMotion
    );

    let flush = coalescer.flush_frame().unwrap();

    assert_eq!(flush.reason, RoutedInputFlushReason::FrameBoundary);
    assert_eq!(flush.inputs.len(), 1);
    assert_eq!(flush.inputs[0].event.serial, 2);
    assert!(!coalescer.has_pending_motion());
}

#[test]
fn routed_input_coalescer_flushes_on_target_crossing() {
    let mut coalescer = RoutedInputCoalescer::new();
    coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));

    let action = coalescer.push(motion_event(2, 11.0, 11.0), route(2, 0x40, 1.0, 1.0));

    let RoutedInputQueueAction::Flushed(flush) = action else {
        panic!("expected target crossing flush");
    };
    assert_eq!(flush.reason, RoutedInputFlushReason::TargetCrossing);
    assert_eq!(
        flush
            .inputs
            .iter()
            .map(|input| input.event.serial)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(!coalescer.has_pending_motion());
}

#[test]
fn routed_input_coalescer_flushes_for_button_and_key_events() {
    let mut coalescer = RoutedInputCoalescer::new();
    coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));

    let button = input_event(
        2,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
        10.0,
        10.0,
    );
    let action = coalescer.push(button, route(2, 0x30, 10.0, 10.0));

    let RoutedInputQueueAction::Flushed(flush) = action else {
        panic!("expected button flush");
    };
    assert_eq!(flush.reason, RoutedInputFlushReason::StateChangingInput);
    assert_eq!(flush.inputs.len(), 2);
    assert!(!coalescer.has_pending_motion());

    let key = input_event(
        3,
        InputEventKind::Key {
            keycode: 38,
            pressed: true,
        },
        0.0,
        0.0,
    );
    let action = coalescer.push(key, route(3, 0x30, 0.0, 0.0));

    let RoutedInputQueueAction::Flushed(flush) = action else {
        panic!("expected key flush");
    };
    assert_eq!(flush.reason, RoutedInputFlushReason::StateChangingInput);
    assert_eq!(flush.inputs.len(), 1);
    assert_eq!(flush.inputs[0].event.serial, 3);
}

#[test]
fn routed_input_coalescer_flushes_for_drag_grab_and_focus_barriers() {
    for reason in [
        RoutedInputFlushReason::DragStateChanged,
        RoutedInputFlushReason::GrabChanged,
        RoutedInputFlushReason::FocusChanged,
    ] {
        let mut coalescer = RoutedInputCoalescer::new();
        coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));

        let flush = coalescer.flush_barrier(reason).unwrap();

        assert_eq!(flush.reason, reason);
        assert_eq!(flush.inputs.len(), 1);
        assert_eq!(flush.inputs[0].event.serial, 1);
        assert!(!coalescer.has_pending_motion());
    }
}

#[test]
fn physical_input_route_becomes_xlibre_request() {
    let event = motion_event(77, 25.0, 35.0);
    let route = route(77, 0x44, 5.0, 6.0);

    let request = routed_input_request_from_physical_event(&event, &route).unwrap();

    assert_eq!(request.serial, 77);
    assert_eq!(request.seat, event.seat);
    assert_eq!(request.device, event.device);
    assert_eq!(request.time_msec, event.time_msec);
    assert_eq!(request.target_window, XWindowId::new(0x44, 1));
    assert_eq!(request.local_position, Point { x: 5.0, y: 6.0 });
    assert_eq!(request.kind, InputEventKind::PointerMotion);
}

#[test]
fn physical_input_flush_becomes_xlibre_requests_after_state_change() {
    let mut coalescer = RoutedInputCoalescer::new();
    coalescer.push(motion_event(1, 10.0, 10.0), route(1, 0x30, 10.0, 10.0));
    let button = input_event(
        2,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
        10.0,
        10.0,
    );

    let RoutedInputQueueAction::Flushed(flush) = coalescer.push(button, route(2, 0x30, 10.0, 10.0))
    else {
        panic!("expected state-changing flush");
    };
    let requests = routed_input_requests_from_flush(&flush).unwrap();

    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].serial, 1);
    assert_eq!(requests[1].serial, 2);
    assert_eq!(
        requests[1].kind,
        InputEventKind::PointerButton {
            button: 1,
            pressed: true
        }
    );
}

#[test]
fn physical_input_route_rejects_malformed_routes() {
    let event = motion_event(1, 10.0, 10.0);
    let mut mismatched = route(2, 0x30, 10.0, 10.0);
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::SerialMismatch)
    );

    mismatched.input_serial = 1;
    mismatched.outcome = InputRouteOutcome::NoTarget;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::RouteNotAccepted)
    );

    mismatched.outcome = InputRouteOutcome::Routed;
    mismatched.target_window = None;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::MissingTargetWindow)
    );

    mismatched.target_window = Some(XWindowId::new(0x30, 1));
    mismatched.local_position = None;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::MissingLocalPosition)
    );
}

fn test_layer(surface_index: u32, stack_rank: u32, x: i32, damage: Region) -> LayerSnapshot {
    LayerSnapshot {
        surface: SurfaceId::new(surface_index, 1),
        window: None,
        namespace: None,
        stack_rank,
        geometry: Rect {
            x,
            y: 0,
            width: 100,
            height: 100,
        },
        source: BufferSource::CpuBuffer {
            handle: u64::from(surface_index) + 1,
        },
        damage,
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
    }
}

fn motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    input_event(serial, InputEventKind::PointerMotion, x, y)
}

fn input_event(serial: u64, kind: InputEventKind, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial * 10,
        kind,
        global_position: Some(Point { x, y }),
        target_surface: Some(SurfaceId::new(1, 1)),
        target_window: Some(XWindowId::new(0x30, 1)),
        local_position: Some(Point { x, y }),
    }
}

fn route(serial: u64, target_window: u32, x: f64, y: f64) -> InputRoute {
    InputRoute {
        input_serial: serial,
        target_surface: Some(SurfaceId::new(1, 1)),
        target_window: Some(XWindowId::new(target_window, 1)),
        global_position: Point { x, y },
        local_position: Some(Point { x, y }),
        transform: Transform::IDENTITY,
        outcome: InputRouteOutcome::Routed,
    }
}

fn frame_tick(frame_serial: u64) -> sophia_engine::FrameClockTick {
    sophia_engine::FrameClockTick {
        output: OutputId::from_raw(1),
        frame_serial,
        target_msec: frame_serial * 16,
    }
}

fn damage_frame(frame_serial: u64, affected_surfaces: &[SurfaceId]) -> DamageFrame {
    DamageFrame {
        output: OutputId::from_raw(1),
        frame_serial,
        buffer_age: 1,
        root_generation: frame_serial,
        affected_surfaces: affected_surfaces.to_vec(),
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        }),
    }
}

fn wm_request(transaction: TransactionId) -> WmRequestPacket {
    WmRequestPacket {
        transaction,
        kind: WmRequestKind::SurfaceRemoved {
            surface: SurfaceId::new(1, 1),
            workspace: WorkspaceId::from_raw(1),
        },
    }
}

fn metadata(surface: SurfaceId, label: &str, generation: u64) -> SanitizedChromeMetadata {
    SanitizedChromeMetadata {
        surface,
        label: Some(label.to_owned()),
        label_redacted: true,
        icon: None,
        trust_level: TrustLevel::Unknown,
        attention: AttentionState::None,
        generation,
    }
}

fn notification_request(raw_transfer: u64) -> NotificationRequest {
    NotificationRequest {
        transfer: PortalTransferId::from_raw(raw_transfer),
        source_namespace: NamespaceId::from_raw(1),
        target_namespace: NamespaceId::from_raw(2),
        summary: "Build finished".to_owned(),
        body: Some("Sophia smoke completed".to_owned()),
        urgency: NotificationUrgency::Normal,
        actions: vec!["Open log".to_owned()],
        generation: 7,
    }
}

struct TestDuplex {
    read: Cursor<Vec<u8>>,
    written: Vec<u8>,
}

impl TestDuplex {
    fn new(read: Vec<u8>) -> Self {
        Self {
            read: Cursor::new(read),
            written: Vec::new(),
        }
    }
}

impl Read for TestDuplex {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read.read(buf)
    }
}

impl Write for TestDuplex {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
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

fn layout_node(surface: SurfaceId, generation: u64, closable: bool) -> LayoutNodeSnapshot {
    let mut capabilities = LayoutNodeCapabilities::STANDARD_TOPLEVEL;
    capabilities.closable = closable;

    LayoutNodeSnapshot {
        surface,
        workspace: WorkspaceId::from_raw(1),
        kind: LayoutNodeKind::Toplevel,
        capabilities,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        generation,
    }
}

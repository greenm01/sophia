use sophia_engine::{
    ChromeActionDecision, ChromeActionRejectReason, ChromeBroker, EngineError, FramePlanRequest,
    HeadlessEngine,
};
use sophia_protocol::{
    AttentionState, BufferSource, ChromeActionKind, ChromeActionRequest, ChromeDescriptor,
    DisplayLabel, IconTokenId, LayerSnapshot, LayoutNodeCapabilities, LayoutNodeKind,
    LayoutNodeSnapshot, LayoutNodeState, LayoutTransaction, OutputId, Rect, Region,
    SurfaceConstraints, SurfaceId, SurfacePlacement, TransactionId, TransactionOutcome, Transform,
    TrustLevel, WorkspaceId,
};

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

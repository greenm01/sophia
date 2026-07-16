mod support;
use support::*;

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
    assert_eq!(replay.steps[0].clip, frame.commands[0].clip);
    assert_eq!(replay.steps[0].transform, frame.commands[0].transform);
}

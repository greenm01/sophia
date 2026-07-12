mod support;
use support::*;

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
fn transformed_scene_hit_test_routes_to_topmost_layer_local_coordinates() {
    let mut lower = test_layer(0, 0, 0, Region::empty());
    lower.authority_local_id = Some(AuthorityLocalId::new(0x20, 1));
    let mut upper = test_layer(1, 10, 0, Region::empty());
    upper.authority_local_id = Some(AuthorityLocalId::new(0x30, 1));
    upper.transform = scale_translate_transform(2.0, 30.0, 40.0);
    let event = motion_event(70, 50.0, 60.0);

    let route = hit_test_scene_for_input(&event, &[lower, upper]);

    assert_eq!(route.outcome, InputRouteOutcome::Routed);
    assert_eq!(route.target_surface, Some(SurfaceId::new(1, 1)));
    assert_eq!(route.global_position, Point { x: 50.0, y: 60.0 });
    assert_eq!(route.local_position, Some(Point { x: 10.0, y: 10.0 }));
    assert_eq!(route.transform, scale_translate_transform(2.0, 30.0, 40.0));
}

#[test]
fn transformed_scene_hit_test_reports_no_target_for_miss() {
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.authority_local_id = Some(AuthorityLocalId::new(0x20, 1));
    layer.transform = scale_translate_transform(2.0, 30.0, 40.0);
    let event = motion_event(71, 10.0, 10.0);

    let route = hit_test_scene_for_input(&event, &[layer]);

    assert_eq!(route.outcome, InputRouteOutcome::NoTarget);
    assert_eq!(route.target_surface, None);
    assert_eq!(route.local_position, None);
}

#[test]
fn surface_hit_test_routes_without_exposing_authority_window_identity() {
    let layer = test_layer(0, 0, 0, Region::empty());
    let event = motion_event(73, 10.0, 10.0);

    let route = hit_test_scene_surface_for_input(&event, &[layer]);

    assert_eq!(route.outcome, InputRouteOutcome::Routed);
    assert_eq!(route.target_surface, Some(SurfaceId::new(0, 1)));
    assert_eq!(route.local_position, Some(Point { x: 10.0, y: 10.0 }));
}

#[test]
fn transformed_scene_hit_test_feeds_routed_input_request_generation() {
    let mut layer = test_layer(0, 0, 0, Region::empty());
    layer.authority_local_id = Some(AuthorityLocalId::new(0x30, 1));
    layer.transform = scale_translate_transform(2.0, 30.0, 40.0);
    let event = motion_event(72, 54.0, 64.0);

    let route = hit_test_scene_for_input(&event, &[layer]);
    let request = routed_input_request_from_physical_event(&event, &route).unwrap();

    assert_eq!(request.serial, 72);
    assert_eq!(request.target_surface, SurfaceId::new(0, 1));
    assert_eq!(request.local_position, Point { x: 12.0, y: 12.0 });
    assert_eq!(request.kind, InputEventKind::PointerMotion);
}

#[test]
fn physical_input_route_becomes_authority_request() {
    let event = motion_event(77, 25.0, 35.0);
    let route = route(77, 0x44, 5.0, 6.0);

    let request = routed_input_request_from_physical_event(&event, &route).unwrap();

    assert_eq!(request.serial, 77);
    assert_eq!(request.seat, event.seat);
    assert_eq!(request.device, event.device);
    assert_eq!(request.time_msec, event.time_msec);
    assert_eq!(request.target_surface, SurfaceId::new(0x44, 1));
    assert_eq!(request.local_position, Point { x: 5.0, y: 6.0 });
    assert_eq!(request.kind, InputEventKind::PointerMotion);
}

#[test]
fn physical_input_flush_becomes_authority_requests_after_state_change() {
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
    mismatched.target_surface = None;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::MissingTargetSurface)
    );

    mismatched.target_surface = Some(SurfaceId::new(0x30, 1));
    mismatched.local_position = None;
    assert_eq!(
        routed_input_request_from_physical_event(&event, &mismatched),
        Err(RoutedInputRequestError::MissingLocalPosition)
    );
}

mod support;

use support::*;

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
fn routed_input_edge_smoke_reports_grab_and_focus_as_closed_routes() {
    let reports = smoke_routed_input_edges(xid(0x30));

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].edge, RoutedInputEdgeKind::ActiveGrab);
    assert_eq!(
        reports[0].decision.outcome,
        XLibreRoutedInputOutcome::RejectedActiveGrab
    );
    assert!(!reports[0].delivery_allowed);
    assert_eq!(reports[1].edge, RoutedInputEdgeKind::FocusPolicy);
    assert_eq!(
        reports[1].decision.outcome,
        XLibreRoutedInputOutcome::RejectedFocusPolicy
    );
    assert!(!reports[1].delivery_allowed);
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

mod support;
use support::*;

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
fn wm_transaction_update_maps_to_runtime_observation() {
    let engine = HeadlessEngine::default();
    let request = wm_request(TransactionId::from_raw(63));
    let response = WmResponsePacket {
        transaction: request.transaction,
        commands: Vec::new(),
        timeout_msec: 250,
    };
    let mut stream = TestDuplex::new(encode_wm_response_frame(&response).unwrap());
    let mut layers = vec![test_layer(0, 0, 0, Region::empty())];

    let update = engine.request_and_commit_wm_transaction(&mut stream, &request, &mut layers);

    assert_eq!(
        runtime_observation_from_wm_transaction_update(&update),
        SessionRuntimeObservation::WmLayoutReady
    );

    let mut closed_stream = TestDuplex::new(Vec::new());
    let restart_update =
        engine.request_and_commit_wm_transaction(&mut closed_stream, &request, &mut layers);

    assert_eq!(
        runtime_observation_from_wm_transaction_update(&restart_update),
        SessionRuntimeObservation::WmRestartRequested
    );
}

#[test]
fn authority_transaction_commit_maps_to_reduced_runtime_observation() {
    let commit = TransactionCommit {
        transaction: TransactionId::from_raw(88),
        outcome: TransactionOutcome::Committed,
        applied_surfaces: vec![SurfaceId::new(1, 1), SurfaceId::new(2, 1)],
    };

    assert_eq!(
        runtime_observation_from_authority_transaction_commit(&commit),
        SessionRuntimeObservation::AuthorityTransactionObserved {
            outcome: TransactionOutcome::Committed,
            applied_surface_count: 2,
        }
    );
}

#[test]
fn authority_removal_batch_drops_the_committed_surface() {
    let engine = HeadlessEngine::default();
    let surface = SurfaceId::new(77, 1);
    let mut committed = vec![CommittedSurfaceState {
        surface,
        committed_generation: 4,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 60,
        },
        buffer: BufferSource::CpuBuffer { handle: 77 },
        damage: Region::empty(),
    }];

    let commit = AuthorityTransactionIntake::new(TransactionId::from_raw(89), Vec::new())
        .with_surface_removals(vec![surface])
        .commit(&engine, &mut committed);

    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert!(commit.applied_surfaces.is_empty());
    assert!(committed.is_empty());
}

#[test]
fn slow_client_visual_decisions_map_to_count_only_runtime_observation() {
    let surface_a = SurfaceId::new(1, 1);
    let surface_b = SurfaceId::new(2, 1);
    let decisions = [
        SlowClientVisualDecision::PreserveCommitted {
            surface: surface_a,
            committed: None,
        },
        SlowClientVisualDecision::DegradeToPending {
            surface: surface_b,
            degraded: CommittedSurfaceState {
                surface: surface_b,
                committed_generation: 3,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                buffer: BufferSource::CpuBuffer { handle: 99 },
                damage: Region::empty(),
            },
        },
        SlowClientVisualDecision::NotTimedOut {
            surface: SurfaceId::new(3, 1),
            readiness: SurfaceTransactionCommitReadiness::Ready,
        },
    ];

    assert_eq!(
        runtime_observation_from_slow_client_visual_decisions(&decisions),
        SessionRuntimeObservation::SlowClientVisualDecisionsObserved {
            timeout_count: 2,
            preserved_count: 1,
            degraded_count: 1,
        }
    );
}

#[test]
fn frame_reports_map_to_runtime_render_observations() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut last_committed = LastCommittedLayout::default();
    let session_report = engine
        .run_session_tick(
            SessionTickRequest {
                output: output.id,
                frame_serial: 77,
                layers: SessionLayerSource::Fresh(vec![test_layer(0, 0, 0, Region::empty())]),
            },
            &mut last_committed,
        )
        .unwrap();

    assert_eq!(
        runtime_observation_from_session_tick_report(&session_report),
        SessionRuntimeObservation::FrameRendered { frame_serial: 77 }
    );

    let render_report = engine.render_frame(&session_report.frame).unwrap();

    assert_eq!(
        runtime_observation_from_render_frame_report(&render_report),
        SessionRuntimeObservation::FrameRendered { frame_serial: 77 }
    );
}

#[test]
fn portal_commands_map_to_runtime_portal_observation() {
    let commands = [
        PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(1),
        },
        PortalCommand::DeliverNotification {
            transfer: PortalTransferId::from_raw(2),
        },
    ];

    assert_eq!(
        runtime_observation_from_portal_commands(&commands),
        SessionRuntimeObservation::PortalCommandsReady { count: 2 }
    );
}

#[test]
fn chrome_updates_map_to_runtime_chrome_observations() {
    let notification_updates = [
        NotificationChromeUpdate::Staged {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Presented {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Dismissed {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Ignored,
    ];

    assert_eq!(
        runtime_observation_from_notification_chrome_updates(&notification_updates),
        SessionRuntimeObservation::ChromeCommandsReady { count: 2 }
    );

    let metadata_updates = [
        MetadataChromeUpdate::Upserted {
            surface: SurfaceId::new(1, 1),
        },
        MetadataChromeUpdate::Removed {
            surface: SurfaceId::new(2, 1),
        },
        MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel),
    ];

    assert_eq!(
        runtime_observation_from_metadata_chrome_updates(&metadata_updates),
        SessionRuntimeObservation::ChromeCommandsReady { count: 2 }
    );
}

#[test]
fn headless_session_driver_executes_runtime_commands_to_idle() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let transaction = TransactionId::from_raw(80);

    let report = driver
        .run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: 90,
            x_event_count: 1,
            layers: vec![test_layer(1, 0, 0, Region::empty())],
            wm_update: Some(WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![SurfaceId::new(1, 1)],
                },
                ipc_error: None,
            }),
            portal_commands: vec![PortalCommand::DropNotification {
                transfer: PortalTransferId::from_raw(1),
            }],
            chrome_command_count: 2,
        })
        .unwrap();

    assert_eq!(
        report.runtime_commands,
        vec![
            SessionRuntimeCommand::PollXEvents,
            SessionRuntimeCommand::RequestWmLayout,
            SessionRuntimeCommand::ScheduleFrame,
            SessionRuntimeCommand::RenderFrame { frame_serial: 90 },
            SessionRuntimeCommand::SubmitScanout { frame_serial: 90 },
            SessionRuntimeCommand::DrainPortalCommands,
            SessionRuntimeCommand::PresentChrome,
        ]
    );
    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(report.runtime_state.x_events_polled, 1);
    assert_eq!(report.runtime_state.frames_rendered, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(report.runtime_state.in_flight_scanouts, 1);
    assert_eq!(report.runtime_state.portal_commands_drained, 1);
    assert_eq!(report.runtime_state.chrome_commands_presented, 2);
    assert_eq!(report.cached_layers, 1);
    assert_eq!(
        report
            .session_tick
            .as_ref()
            .map(|tick| tick.frame.frame_serial),
        Some(90)
    );
}

#[test]
fn headless_session_driver_executes_through_runtime_adapter_trait() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = HeadlessRuntimeAdapter {
        x_event_count: 1,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
    };

    let report = driver
        .run_with_adapter(output.id, 92, &mut adapter)
        .expect("headless adapter should drive one runtime tick");

    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(report.runtime_state.frames_rendered, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(
        report.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(
        report
            .session_tick
            .as_ref()
            .map(|tick| tick.frame.frame_serial),
        Some(92)
    );
}

#[test]
fn live_runtime_driver_adapter_executes_through_shared_command_executor() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 1,
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: vec![PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(3),
        }],
        chrome_command_count: 1,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    });

    let report = driver
        .run_with_adapter(output.id, 93, &mut adapter)
        .expect("live adapter intake should drive one runtime tick");

    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(report.runtime_state.x_events_polled, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(report.runtime_state.portal_commands_drained, 1);
    assert_eq!(report.runtime_state.chrome_commands_presented, 1);
    assert_eq!(
        report
            .session_tick
            .as_ref()
            .map(|tick| tick.frame.frame_serial),
        Some(93)
    );
}

#[test]
fn live_runtime_driver_adapter_builds_from_nonblocking_intake_values() {
    let update = WmTransactionUpdate {
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(83),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: vec![SurfaceId::new(1, 1)],
        },
        ipc_error: None,
    };

    let adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 2,
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: Some(update.clone()),
        portal_commands: vec![PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(3),
        }],
        chrome_command_count: 4,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: Some(RuntimeScanoutState::Submitted),
        scanout_lifecycle_states: Vec::new(),
    });

    assert_eq!(adapter.x, LiveXRuntimeAdapter::from_polled_event_count(2));
    assert_eq!(
        adapter.wm,
        LiveWmRuntimeAdapter::from_transaction_update(update)
    );
    assert_eq!(
        adapter.portal.drain_observation(),
        SessionRuntimeObservation::PortalCommandsReady { count: 1 }
    );
    assert_eq!(
        adapter.chrome.present_observation(),
        SessionRuntimeObservation::ChromeCommandsReady { count: 4 }
    );
    assert_eq!(adapter.renderer.layers.len(), 1);
    assert_eq!(
        adapter.scanout.submit_observation(77),
        SessionRuntimeObservation::ScanoutStateChanged {
            state: RuntimeScanoutState::Submitted,
            frame_serial: Some(77),
        }
    );
}

#[test]
fn live_runtime_driver_adapter_reports_rejected_scanout_submit() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 0,
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: Some(RuntimeScanoutState::Rejected),
        scanout_lifecycle_states: Vec::new(),
    });

    let report = driver
        .run_with_adapter(output.id, 96, &mut adapter)
        .expect("rejected scanout submit should stay reduced");

    assert_eq!(report.runtime_state.scanout_submissions, 0);
    assert_eq!(report.runtime_state.scanout_rejections, 1);
    assert_eq!(report.runtime_state.in_flight_scanouts, 0);
    assert_eq!(
        report.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
}

#[test]
fn live_runtime_driver_adapter_records_async_scanout_retirement_before_submit() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 0,
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layers: vec![test_layer(1, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: Some(RuntimeScanoutState::Submitted),
        scanout_lifecycle_states: vec![RuntimeScanoutState::Retired],
    });

    let report = driver
        .run_with_adapter(output.id, 97, &mut adapter)
        .expect("async scanout retirement should not disrupt frame scheduling");

    assert_eq!(report.runtime_state.scanout_retirements, 1);
    assert_eq!(report.runtime_state.scanout_submissions, 1);
    assert_eq!(report.runtime_state.in_flight_scanouts, 1);
    assert_eq!(report.runtime_state.phase, SessionRuntimePhase::Idle);
    assert_eq!(
        report.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
}

#[test]
fn live_runtime_driver_adapter_records_authority_transaction_commits() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: 1,
        authority_commits: vec![TransactionCommit {
            transaction: TransactionId::from_raw(84),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: vec![SurfaceId::new(7, 1)],
        }],
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layers: vec![test_layer(7, 0, 0, Region::empty())],
        committed_surfaces: Vec::new(),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    });

    let report = driver
        .run_with_adapter(output.id, 94, &mut adapter)
        .expect("authority transaction observations should feed runtime state");

    assert_eq!(report.runtime_state.x_events_polled, 1);
    assert_eq!(report.runtime_state.authority_transactions_committed, 1);
    assert_eq!(report.runtime_state.authority_surfaces_applied, 1);
}

#[test]
fn live_runtime_driver_adapter_commits_authority_batches_before_rendering() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let surface = SurfaceId::new(9, 1);
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(86),
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: Some(NamespaceId::from_raw(3)),
        target_geometry: Rect {
            x: 20,
            y: 30,
            width: 140,
            height: 90,
        },
        target_buffer: BufferSource::CpuBuffer { handle: 700 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 140,
            height: 90,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };
    let mut driver = HeadlessSessionDriver::new(engine.clone());
    let mut adapter = LiveRuntimeDriverAdapter::from_authority_batches(
        &engine,
        LiveRuntimeDriverIntake {
            x_event_count: 1,
            authority_commits: Vec::new(),
            authority_batches: vec![AuthorityTransactionIntake::new(
                TransactionId::from_raw(86),
                vec![transaction],
            )],
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 0,
            layers: vec![test_layer(9, 0, 0, Region::empty())],
            committed_surfaces: Vec::new(),
            scanout_submit_state: None,
            scanout_lifecycle_states: Vec::new(),
        },
    );

    let report = driver
        .run_with_adapter(output.id, 95, &mut adapter)
        .expect("authority batches should commit before frame projection");

    assert_eq!(report.runtime_state.authority_transactions_committed, 1);
    assert_eq!(report.runtime_state.authority_surfaces_applied, 1);
    let frame = &report.session_tick.unwrap().frame;
    assert_eq!(frame.layers[0].geometry.x, 20);
    assert_eq!(
        frame.layers[0].source,
        BufferSource::CpuBuffer { handle: 700 }
    );
}

#[test]
fn headless_session_driver_stops_before_rendering_when_wm_restart_is_requested() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut driver = HeadlessSessionDriver::new(engine);
    let transaction = TransactionId::from_raw(81);

    let report = driver
        .run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: 91,
            x_event_count: 1,
            layers: vec![test_layer(1, 0, 0, Region::empty())],
            wm_update: Some(WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::TimedOut,
                    applied_surfaces: Vec::new(),
                },
                ipc_error: Some(WmIpcError::Io("closed".to_owned())),
            }),
            portal_commands: Vec::new(),
            chrome_command_count: 0,
        })
        .unwrap();

    assert_eq!(
        report.runtime_commands,
        vec![
            SessionRuntimeCommand::PollXEvents,
            SessionRuntimeCommand::RequestWmLayout,
            SessionRuntimeCommand::RestartWindowManager,
        ]
    );
    assert_eq!(
        report.runtime_state.phase,
        SessionRuntimePhase::ApplyingWmPolicy
    );
    assert_eq!(report.runtime_state.frames_rendered, 0);
    assert!(report.session_tick.is_none());
}

#[test]
fn live_x_runtime_adapter_emits_bounded_event_count_observation() {
    let adapter = LiveXRuntimeAdapter {
        pending_event_count: 12,
        authority_commits: Vec::new(),
    };

    assert_eq!(
        adapter.poll_observation(),
        SessionRuntimeObservation::XEventsPolled { count: 12 }
    );
}

#[test]
fn live_x_runtime_adapter_emits_authority_commit_observations() {
    let adapter = LiveXRuntimeAdapter {
        pending_event_count: 2,
        authority_commits: vec![TransactionCommit {
            transaction: TransactionId::from_raw(85),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: vec![SurfaceId::new(8, 1)],
        }],
    };

    assert_eq!(
        adapter.poll_observations(),
        vec![
            SessionRuntimeObservation::XEventsPolled { count: 2 },
            SessionRuntimeObservation::AuthorityTransactionObserved {
                outcome: TransactionOutcome::Committed,
                applied_surface_count: 1,
            },
        ]
    );
}

#[test]
fn live_wm_runtime_adapter_maps_restart_update_to_observation() {
    let adapter = LiveWmRuntimeAdapter {
        update: Some(WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(82),
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
            ipc_error: Some(WmIpcError::Io("closed".to_owned())),
        }),
    };

    assert_eq!(
        adapter.layout_observation(),
        SessionRuntimeObservation::WmRestartRequested
    );
}

#[test]
fn live_broker_runtime_adapter_routes_health_without_message_payload() {
    let packet = BrokerHealthPacket::new(
        BrokerKind::Portal,
        BrokerHealthState::Ready,
        44,
        Some("ready".to_owned()),
    )
    .unwrap();

    assert_eq!(
        LiveBrokerRuntimeAdapter::from_health_packet(&packet),
        SessionRuntimeObservation::BrokerHealthChanged {
            broker: BrokerKind::Portal,
            state: BrokerHealthState::Ready,
            generation: 44,
            status_message_len: 5,
        }
    );
}

#[test]
fn live_portal_chrome_and_renderer_adapters_emit_counts_and_frame_serials() {
    let portal = LivePortalRuntimeAdapter::from_commands(vec![
        PortalCommand::DropNotification {
            transfer: PortalTransferId::from_raw(1),
        },
        PortalCommand::DeliverNotification {
            transfer: PortalTransferId::from_raw(2),
        },
    ]);
    let notification_updates = [
        NotificationChromeUpdate::Staged {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Presented {
            transfer: PortalTransferId::from_raw(1),
        },
        NotificationChromeUpdate::Dismissed {
            transfer: PortalTransferId::from_raw(1),
        },
    ];
    let chrome = LiveChromeRuntimeAdapter::from_notification_updates(&notification_updates);
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut last_committed = LastCommittedLayout::default();
    let mut renderer =
        LiveRendererRuntimeAdapter::from_layers(vec![test_layer(1, 0, 0, Region::empty())]);

    let report = renderer
        .render_frame(&engine, output.id, 94, &mut last_committed)
        .unwrap();

    assert_eq!(
        portal.drain_observation(),
        SessionRuntimeObservation::PortalCommandsReady { count: 2 }
    );
    assert_eq!(
        chrome.present_observation(),
        SessionRuntimeObservation::ChromeCommandsReady { count: 2 }
    );
    assert_eq!(
        LiveRendererRuntimeAdapter::rendered_observation(&report),
        SessionRuntimeObservation::FrameRendered { frame_serial: 94 }
    );
    assert_eq!(
        LiveRendererRuntimeAdapter::from_render_frame_report(
            &engine.render_frame(&report.frame).unwrap()
        ),
        SessionRuntimeObservation::FrameRendered { frame_serial: 94 }
    );
}

#[test]
fn live_renderer_runtime_adapter_projects_committed_state_before_frame_planning() {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut last_committed = LastCommittedLayout::default();
    let template = test_layer(1, 0, 0, Region::empty());
    let committed = CommittedSurfaceState {
        surface: template.surface,
        committed_generation: 3,
        geometry: Rect {
            x: 200,
            y: 220,
            width: 320,
            height: 240,
        },
        buffer: BufferSource::DmaBuf { handle: 701 },
        damage: Region::single(Rect {
            x: 200,
            y: 220,
            width: 320,
            height: 240,
        }),
    };
    let mut renderer = LiveRendererRuntimeAdapter::from_committed_surface_states(
        vec![committed.clone()],
        vec![template],
    );

    let report = renderer
        .render_frame(&engine, output.id, 95, &mut last_committed)
        .unwrap();

    assert_eq!(report.frame.layers[0].geometry, committed.geometry);
    assert_eq!(
        report.frame.layers[0].source,
        BufferSource::DmaBuf { handle: 701 }
    );
    assert_eq!(report.frame.commands[0].target.rects[0], committed.geometry);
}

#[test]
fn live_chrome_runtime_adapter_counts_metadata_updates() {
    let updates = [
        MetadataChromeUpdate::Upserted {
            surface: SurfaceId::new(1, 1),
        },
        MetadataChromeUpdate::Removed {
            surface: SurfaceId::new(1, 2),
        },
        MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel),
    ];

    let chrome = LiveChromeRuntimeAdapter::from_metadata_updates(&updates);

    assert_eq!(
        chrome.present_observation(),
        SessionRuntimeObservation::ChromeCommandsReady { count: 2 }
    );
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

#[derive(Default)]
struct RecordingOutputRuntimeAdapter {
    output_count: usize,
    generations: Vec<(usize, u64)>,
}

impl ProductionOutputRuntimeAdapter for RecordingOutputRuntimeAdapter {
    type Report = usize;
    type Error = String;

    fn output_count(&self) -> usize {
        self.output_count
    }

    fn run_output(
        &mut self,
        output_index: usize,
        committed: &[CommittedSurfaceState],
    ) -> Result<Self::Report, Self::Error> {
        self.generations
            .push((output_index, committed[0].committed_generation));
        Ok(output_index)
    }
}

#[test]
fn production_coordinator_projects_one_snapshot_through_output_runtime_adapter() {
    let engine = HeadlessEngine::default();
    let committed = vec![engine.committed_state_from_layer(&test_layer(0, 0, 0, Region::empty()))];
    let coordinator = ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let mut adapter = RecordingOutputRuntimeAdapter {
        output_count: 2,
        ..RecordingOutputRuntimeAdapter::default()
    };

    let reports = coordinator.run_outputs(&mut adapter).unwrap();

    assert_eq!(reports, [0, 1]);
    assert_eq!(adapter.generations, [(0, 1), (1, 1)]);
}

#[derive(Default)]
struct RecordingProductionAdapter {
    calls: Vec<&'static str>,
    fail_at: Option<&'static str>,
    pending: Vec<(u64, usize)>,
    withhold_retirement: bool,
    feedback_cycles: Vec<u64>,
}

impl ProductionPresentationAdapter for RecordingProductionAdapter {
    type Frame = usize;
    type Submission = usize;
    type Retirement = usize;
    type Evidence = usize;
    type Error = &'static str;

    fn compose(
        &mut self,
        _cycle: u64,
        committed: &[CommittedSurfaceState],
        _authority_commits: &[TransactionCommit],
    ) -> Result<Self::Frame, Self::Error> {
        self.calls.push("compose");
        if self.fail_at == Some("compose") {
            return Err("compose");
        }
        Ok(committed.len())
    }

    fn submit_frame(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Submission, Self::Error> {
        self.calls.push("submit");
        if self.fail_at == Some("submit") {
            return Err("submit");
        }
        self.pending.push((cycle, frame));
        Ok(frame)
    }

    fn poll_retirements(
        &mut self,
    ) -> Result<Vec<ProductionRetirement<Self::Retirement>>, Self::Error> {
        self.calls.push("retire");
        if self.withhold_retirement {
            return Ok(Vec::new());
        }
        if self.fail_at == Some("retire") {
            return Err("retire");
        }
        Ok(self
            .pending
            .drain(..)
            .map(|(cycle, retirement)| ProductionRetirement { cycle, retirement })
            .collect())
    }

    fn route_protocol_feedback(
        &mut self,
        cycle: u64,
        retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error> {
        self.calls.push("feedback");
        self.feedback_cycles.push(cycle);
        if self.fail_at == Some("feedback") {
            return Err("feedback");
        }
        assert_eq!(retirement, 1);
        Ok(retirement)
    }
}

fn production_surface_batch(transaction: u64) -> AuthorityTransactionIntake {
    let surface = SurfaceId::new(44, 1);
    AuthorityTransactionIntake::new(
        TransactionId::from_raw(transaction),
        vec![SurfaceTransaction {
            transaction: TransactionId::from_raw(transaction),
            authority: AuthorityKind::SophiaX,
            surface,
            namespace: Some(NamespaceId::from_raw(2)),
            target_geometry: Rect {
                x: 10,
                y: 20,
                width: 320,
                height: 200,
            },
            target_buffer: BufferSource::CpuBuffer { handle: 900 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            }),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        }],
    )
}

#[test]
fn production_coordinator_applies_prepared_present_to_its_owned_snapshot() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut coordinator =
        ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let mut next_layer = old_layer;
    next_layer.geometry.width = 640;
    next_layer.source = BufferSource::DmaBuf { handle: 77 };
    let mut transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(205),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    transaction.previous_committed_generation = 99;
    let prepared =
        coordinator.prepare_full_state_present(TransactionId::from_raw(205), &[transaction]);

    let mut feedback_calls = 0;
    let completion = coordinator
        .complete_prepared_retirement(prepared, || {
            feedback_calls += 1;
            Ok::<_, &str>("flip-idle")
        })
        .expect("matching retirement should commit before feedback");

    assert_eq!(completion.commit.outcome, TransactionOutcome::Committed);
    assert_eq!(completion.evidence, "flip-idle");
    assert_eq!(feedback_calls, 1);
    assert_eq!(coordinator.committed_surfaces()[0].geometry.width, 640);
    assert_eq!(
        coordinator.committed_surfaces()[0].buffer,
        BufferSource::DmaBuf { handle: 77 }
    );
}

#[test]
fn production_coordinator_suppresses_feedback_when_prepared_baseline_is_stale() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(0, 0, 0, Region::empty());
    let committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut coordinator =
        ProductionSessionCoordinator::new(engine).with_committed_surfaces(committed);
    let transaction = old_layer.to_surface_transaction(
        TransactionId::from_raw(206),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let prepared = coordinator.engine().prepare_surface_transactions(
        TransactionId::from_raw(206),
        &[transaction],
        coordinator.committed_surfaces(),
    );
    let mut changed = coordinator.committed_surfaces().to_vec();
    changed[0].committed_generation = 9;
    coordinator.replace_committed_surfaces(changed);
    let mut feedback_calls = 0;

    let result = coordinator.complete_prepared_retirement(prepared, || {
        feedback_calls += 1;
        Ok::<_, &str>(())
    });

    assert!(matches!(
        result,
        Err(ProductionPreparedRetirementError::EngineCommit(commit))
            if commit.outcome == TransactionOutcome::RejectedStaleSurface
    ));
    assert_eq!(feedback_calls, 0);
    assert_eq!(coordinator.committed_surfaces()[0].committed_generation, 9);
}

#[test]
fn production_coordinator_orders_commit_composition_retirement_and_feedback() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter::default();

    let report = coordinator
        .run_cycle(&[production_surface_batch(201)], &mut adapter)
        .expect("production cycle should complete");

    assert_eq!(adapter.calls, ["compose", "submit", "retire", "feedback"]);
    assert_eq!(report.cycle, 1);
    assert_eq!(
        report.authority_commits[0].outcome,
        TransactionOutcome::Committed
    );
    assert_eq!(report.committed_surfaces.len(), 1);
    assert_eq!(report.submission, 1);
    assert_eq!(report.evidence, [1]);
}

#[test]
fn production_coordinator_routes_delayed_feedback_only_after_a_later_retirement_poll() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter {
        withhold_retirement: true,
        ..RecordingProductionAdapter::default()
    };

    let first = coordinator
        .run_cycle(&[production_surface_batch(204)], &mut adapter)
        .expect("submission without a page flip remains in flight");
    assert!(first.evidence.is_empty());
    assert!(adapter.feedback_cycles.is_empty());

    adapter.withhold_retirement = false;
    let second = coordinator
        .run_cycle(&[], &mut adapter)
        .expect("later page flip poll should retire queued submissions");
    assert_eq!(second.evidence, [1, 1]);
    assert_eq!(adapter.feedback_cycles, [1, 2]);
}

#[test]
fn production_coordinator_never_routes_feedback_before_retirement() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter {
        calls: Vec::new(),
        fail_at: Some("retire"),
        ..RecordingProductionAdapter::default()
    };

    let error = coordinator
        .run_cycle(&[production_surface_batch(202)], &mut adapter)
        .expect_err("missing retirement must fail the cycle");

    assert_eq!(adapter.calls, ["compose", "submit", "retire"]);
    assert_eq!(
        error,
        ProductionSessionCycleError {
            cycle: 1,
            phase: ProductionSessionPhase::KmsRetire,
            source: "retire",
        }
    );
    assert_eq!(coordinator.committed_surfaces().len(), 1);
}

#[test]
fn production_coordinator_reports_feedback_failure_after_retirement() {
    let mut coordinator = ProductionSessionCoordinator::new(HeadlessEngine::default());
    let mut adapter = RecordingProductionAdapter {
        calls: Vec::new(),
        fail_at: Some("feedback"),
        ..RecordingProductionAdapter::default()
    };

    let error = coordinator
        .run_cycle(&[production_surface_batch(203)], &mut adapter)
        .expect_err("feedback failure must remain explicit");

    assert_eq!(adapter.calls, ["compose", "submit", "retire", "feedback"]);
    assert_eq!(error.phase, ProductionSessionPhase::ProtocolFeedback);
    assert_eq!(coordinator.committed_surfaces().len(), 1);
}

#[test]
fn production_async_service_coordinator_owns_dynamic_kms_phase_order() {
    let mut coordinator = ProductionAsyncServiceCoordinator::new();
    assert_eq!(
        coordinator.next_phase(ProductionAsyncServiceObservation {
            native_in_flight: true,
            cleanup_pending: false,
            present_queued: true,
            pending_frame: true,
        }),
        Some(ProductionAsyncServicePhase::KmsRetire)
    );
    assert_eq!(
        coordinator.next_phase(ProductionAsyncServiceObservation {
            native_in_flight: false,
            cleanup_pending: false,
            present_queued: true,
            pending_frame: true,
        }),
        Some(ProductionAsyncServicePhase::SchedulePresent)
    );
    assert_eq!(
        coordinator.next_phase(ProductionAsyncServiceObservation {
            native_in_flight: true,
            cleanup_pending: false,
            present_queued: false,
            pending_frame: true,
        }),
        None
    );
}

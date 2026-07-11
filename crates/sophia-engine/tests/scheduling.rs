mod support;
use support::*;

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
fn per_output_frame_clocks_advance_independently_at_each_refresh_rate() {
    let output_60 = OutputId::from_raw(1);
    let output_120 = OutputId::from_raw(2);
    let mut outputs = DrmKmsOutputRegistry::new();
    outputs.upsert(DrmKmsOutputDescriptor {
        output: output_60,
        connector_id: 11,
        crtc_id: 21,
        mode: DrmKmsMode::new(1920, 1080, 60_000),
        scale: 1,
    });
    outputs.upsert(DrmKmsOutputDescriptor {
        output: output_120,
        connector_id: 12,
        crtc_id: 22,
        mode: DrmKmsMode::new(1920, 1080, 120_000),
        scale: 1,
    });
    let mut clocks =
        PerOutputFrameClock::from_outputs(&outputs, DeterministicFrameClock::default());

    let first_60 = clocks.next_frame(output_60);
    let first_120 = clocks.next_frame(output_120);
    let second_120 = clocks.next_frame(output_120);

    assert_eq!((first_60.frame_serial, first_60.target_msec), (1, 16));
    assert_eq!((first_120.frame_serial, first_120.target_msec), (1, 8));
    assert_eq!((second_120.frame_serial, second_120.target_msec), (2, 16));
    assert_eq!(clocks.get(output_60).unwrap().next_serial(), 2);
    assert_eq!(clocks.get(output_120).unwrap().next_serial(), 3);
}

#[test]
fn per_output_presentation_keeps_damage_and_retirement_independent() {
    let output_a = OutputId::from_raw(1);
    let output_b = OutputId::from_raw(2);
    let mut outputs = DrmKmsOutputRegistry::new();
    for (output, connector, crtc) in [(output_a, 11, 21), (output_b, 12, 22)] {
        outputs.upsert(DrmKmsOutputDescriptor {
            output,
            connector_id: connector,
            crtc_id: crtc,
            mode: DrmKmsMode::new(1280, 720, 60_000),
            scale: 1,
        });
    }
    let mut presentation = OutputPresentationRegistry::from_outputs(&outputs);

    assert!(presentation.mark_damage(output_a));
    assert!(presentation.mark_damage(output_b));
    let OutputPresentationSchedule::Scheduled(frame_a) = presentation.schedule(output_a) else {
        panic!("output A should schedule");
    };
    let OutputPresentationSchedule::Scheduled(frame_b) = presentation.schedule(output_b) else {
        panic!("output B should schedule independently");
    };
    assert_eq!(frame_a.frame_serial, 1);
    assert_eq!(frame_b.frame_serial, 1);
    assert_eq!(
        presentation.schedule(output_a),
        OutputPresentationSchedule::WaitingForRetirement { frame_serial: 1 }
    );
    assert_eq!(
        presentation.retire(output_a, 2),
        OutputPresentationRetire::UnexpectedFrame {
            expected: 1,
            actual: 2
        }
    );
    assert_eq!(
        presentation.retire(output_b, 1),
        OutputPresentationRetire::Retired { frame_serial: 1 }
    );
    assert_eq!(presentation.get(output_a).unwrap().in_flight_frame, Some(1));
    assert_eq!(presentation.get(output_b).unwrap().in_flight_frame, None);
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
fn page_flip_commit_gate_waits_for_matching_output_tick() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(1, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(81),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let mut gate = PageFlipCommitGate::new();
    gate.stage(
        OutputId::from_raw(1),
        TransactionId::from_raw(81),
        vec![transaction],
    );

    let outcome = gate.commit_on_page_flip(
        &engine,
        sophia_engine::FrameClockTick {
            output: OutputId::from_raw(2),
            frame_serial: 11,
            target_msec: 176,
        },
        &mut committed,
    );

    assert_eq!(
        outcome,
        PageFlipCommitOutcome::WaitingForOutput {
            expected: OutputId::from_raw(1),
            actual: OutputId::from_raw(2),
            transaction: TransactionId::from_raw(81),
        }
    );
    assert_eq!(committed[0].geometry.width, old_layer.geometry.width);
    assert!(gate.staged().is_some());
}

#[test]
fn page_flip_commit_gate_preserves_committed_state_until_transactions_are_ready() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(1, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(82),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Pending,
        250,
        1,
    );
    let mut gate = PageFlipCommitGate::new();
    gate.stage(
        OutputId::from_raw(1),
        TransactionId::from_raw(82),
        vec![transaction],
    );

    let outcome = gate.commit_on_page_flip(&engine, frame_tick(12), &mut committed);

    assert_eq!(
        outcome,
        PageFlipCommitOutcome::WaitingForTransactionReadiness {
            transaction: TransactionId::from_raw(82),
            pending_surfaces: vec![old_layer.surface],
        }
    );
    assert_eq!(committed[0].geometry.width, old_layer.geometry.width);
    assert!(gate.staged().is_some());
}

#[test]
fn page_flip_commit_gate_clears_timed_out_transactions_without_changing_visual_state() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(1, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(84),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::TimedOut,
        250,
        1,
    );
    let mut gate = PageFlipCommitGate::new();
    gate.stage(
        OutputId::from_raw(1),
        TransactionId::from_raw(84),
        vec![transaction],
    );

    let outcome = gate.commit_on_page_flip(&engine, frame_tick(14), &mut committed);

    let PageFlipCommitOutcome::Rejected {
        frame_serial,
        commit,
    } = outcome
    else {
        panic!("expected timed-out page-flip transaction to fail closed");
    };
    assert_eq!(frame_serial, 14);
    assert_eq!(commit.transaction, TransactionId::from_raw(84));
    assert_eq!(commit.outcome, TransactionOutcome::TimedOut);
    assert!(commit.applied_surfaces.is_empty());
    assert_eq!(committed[0].geometry.width, old_layer.geometry.width);
    assert!(gate.staged().is_none());
}

#[test]
fn page_flip_commit_gate_commits_ready_transactions_on_page_flip() {
    let engine = HeadlessEngine::default();
    let old_layer = test_layer(1, 0, 0, Region::empty());
    let mut committed = vec![engine.committed_state_from_layer(&old_layer)];
    let mut next_layer = old_layer.clone();
    next_layer.geometry.width = 500;
    next_layer.generation = 2;
    let transaction = next_layer.to_surface_transaction(
        TransactionId::from_raw(83),
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        1,
    );
    let mut gate = PageFlipCommitGate::new();
    gate.stage(
        OutputId::from_raw(1),
        TransactionId::from_raw(83),
        vec![transaction],
    );

    let outcome = gate.commit_on_page_flip(&engine, frame_tick(13), &mut committed);

    let PageFlipCommitOutcome::Committed {
        frame_serial,
        commit,
    } = outcome
    else {
        panic!("expected page-flip commit");
    };
    assert_eq!(frame_serial, 13);
    assert_eq!(commit.transaction, TransactionId::from_raw(83));
    assert_eq!(commit.outcome, TransactionOutcome::Committed);
    assert_eq!(commit.applied_surfaces, vec![old_layer.surface]);
    assert_eq!(committed[0].geometry.width, 500);
    assert_eq!(committed[0].committed_generation, 2);
    assert!(gate.staged().is_none());
}

#[test]
fn layout_epoch_for_explicit_sync_uses_only_cooperative_layers() {
    let mut explicit = test_layer(1, 0, 0, Region::empty());
    explicit.resize_sync = ResizeSyncCapability::ExplicitSync;
    let implicit = test_layer(2, 1, 100, Region::empty());

    let surfaces = explicit_sync_surfaces(&[explicit.clone(), implicit.clone()]);
    let epoch = layout_epoch_for_explicit_sync(12, 100, 250, &[explicit, implicit])
        .expect("explicit layer should create layout epoch");

    assert_eq!(surfaces, vec![SurfaceId::new(1, 1)]);
    assert_eq!(epoch.epoch, 12);
    assert_eq!(epoch.started_msec(), 100);
    assert_eq!(epoch.timeout_msec(), 250);
    assert_eq!(epoch.pending_surfaces(), vec![SurfaceId::new(1, 1)]);
}

#[test]
fn layout_epoch_for_explicit_sync_skips_implicit_only_layers() {
    let layers = [test_layer(1, 0, 0, Region::empty())];

    assert_eq!(explicit_sync_surfaces(&layers), Vec::<SurfaceId>::new());
    assert_eq!(layout_epoch_for_explicit_sync(13, 100, 250, &layers), None);
}

#[test]
fn layout_epoch_maps_damage_wait_to_surface_transaction_readiness() {
    let surface = SurfaceId::new(1, 1);
    let mut epoch = LayoutEpochState::with_timing(15, [surface], 100, 250);

    assert_eq!(
        surface_transaction_readiness_for_epoch(surface, Some(&epoch)),
        SurfaceTransactionReadiness::Pending
    );

    epoch.observe_damage(&damage_frame(4, &[surface]));

    assert_eq!(
        surface_transaction_readiness_for_epoch(surface, Some(&epoch)),
        SurfaceTransactionReadiness::Ready
    );
    assert_eq!(
        surface_transaction_readiness_for_epoch(surface, None),
        SurfaceTransactionReadiness::Ready
    );
}

#[test]
fn layout_epoch_timeout_expiration_clears_pending_surfaces() {
    let surface = SurfaceId::new(1, 1);
    let mut epoch = LayoutEpochState::with_timing(14, [surface], 100, 250);

    assert_eq!(epoch.expire_if_timed_out(349), None);
    let timeout = epoch
        .expire_if_timed_out(350)
        .expect("epoch should time out at timeout boundary");

    assert_eq!(timeout.epoch, 14);
    assert_eq!(timeout.elapsed_msec, 250);
    assert_eq!(timeout.timeout_msec, 250);
    assert_eq!(timeout.pending_surfaces, vec![surface]);
    assert!(epoch.is_complete());
    assert_eq!(epoch.expire_if_timed_out(351), None);
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

#![cfg(feature = "libdrm-events")]

use std::sync::mpsc;

use sophia_backend_live::{
    CompositorBackendTickInput, FakeLibdrmNativePageFlipReader, FakeLibdrmPageFlipEventPoller,
    LibdrmBackendFdAuthority, LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
    LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus, LibdrmNativeCrtcRoute,
    LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus, LibdrmNativeOutputRoute,
    LibdrmNativeOutputSlot, LibdrmNativePageFlipCallback, LibdrmNativePageFlipDecodeReport,
    LibdrmNativePageFlipDecodeStatus, LibdrmNativePageFlipReadResult, LibdrmNativePageFlipReader,
    LibdrmNativePageFlipSource, LibdrmNativePageFlipSourceReport, LibdrmNativePageFlipSourceStatus,
    LibdrmNativePollerDiagnostics, LibdrmNativeReadAndPollReport, LibdrmNativeReadLoopReport,
    LibdrmNativeReadLoopStatus, LibdrmPageFlipEventPollReport, LibdrmPageFlipEventPollStatus,
    LibdrmPageFlipEventPoller, LiveBackendConfig, LiveHardwareValidationGateReport,
    LiveHardwareValidationGateStatus, LiveHardwareValidationSmokeReport,
    LiveHardwareValidationSmokeStatus, LiveHardwareValidationTarget, LiveLibdrmPollerDiagnostics,
    LiveLibdrmPollerDiagnosticsStatus, LiveLibdrmPollerStartupReport,
    LiveLibdrmPollerStartupStatus, LivePageFlipCallback, LivePageFlipCallbackQueue,
    LivePageFlipCallbackSourceReport, LivePageFlipEvent, LivePageFlipEventStatus,
    NativeLibdrmPageFlipEventPoller, NativeLibdrmPageFlipEventReader, OutputId, QueuedInputPoller,
    decode_native_page_flip_batch, discover_live_backend, libdrm_dependency_admission_report,
    libdrm_fd_authority_report, native_libdrm_event_adapter_report,
    native_libdrm_event_adapter_report_for_authority, real_libdrm_events_validation_gate,
    real_libdrm_events_validation_smoke_report, reduce_native_page_flip_event,
};

#[test]
fn libdrm_dependency_is_admitted_without_exposing_native_event_shape() {
    assert_eq!(
        libdrm_dependency_admission_report(),
        LibdrmDependencyAdmissionReport {
            status: LibdrmDependencyAdmissionStatus::TypedPageFlipEventAvailable,
        }
    );
}

#[test]
fn real_libdrm_event_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert!(!skipped.is_requested());
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_libdrm_events_validation_gate().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn real_libdrm_event_validation_smoke_fails_closed_without_device_opening_smoke() {
    let skipped = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            false,
        ),
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::SkippedOptInRequired,
        }
    );

    let requested = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            true,
        ),
    );
    assert_eq!(
        requested,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::BackendUnavailable,
        }
    );

    assert_eq!(
        real_libdrm_events_validation_smoke_report().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn libdrm_fd_authority_is_generation_checked_and_reduced() {
    assert_eq!(LibdrmBackendFdAuthority::new(0), None);

    let authority =
        LibdrmBackendFdAuthority::new(9).expect("nonzero generation should mint authority token");
    assert_eq!(authority.generation(), 9);
    assert_eq!(
        libdrm_fd_authority_report(authority),
        LibdrmBackendFdAuthorityReport {
            status: LibdrmBackendFdAuthorityStatus::BackendOwned,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_skeleton_reports_ready_without_opening_devices() {
    assert_eq!(
        native_libdrm_event_adapter_report(),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_accepts_authority_without_polling() {
    let authority =
        LibdrmBackendFdAuthority::new(12).expect("nonzero generation should mint authority token");

    assert_eq!(
        native_libdrm_event_adapter_report_for_authority(authority),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_page_flip_source_constructs_from_authority_without_reading_events() {
    let authority =
        LibdrmBackendFdAuthority::new(13).expect("nonzero generation should mint authority token");
    let source = LibdrmNativePageFlipSource::from_authority(authority);

    assert_eq!(
        source.report(),
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    );
}

#[test]
fn native_libdrm_read_loop_result_maps_to_reduced_poll_report() {
    assert_eq!(
        LibdrmNativeReadLoopReport::idle().into_poll_report().status,
        LibdrmPageFlipEventPollStatus::Idle
    );
    assert_eq!(
        LibdrmNativeReadLoopReport::would_block()
            .into_poll_report()
            .status,
        LibdrmPageFlipEventPollStatus::Idle
    );

    let decoded =
        LibdrmNativeReadLoopReport::callback_decoded(3).expect("decoded count must be nonzero");
    assert_eq!(decoded.status, LibdrmNativeReadLoopStatus::CallbackDecoded);
    assert_eq!(decoded.into_poll_report().callbacks.emitted, 3);
    assert_eq!(
        decoded.into_poll_report().status,
        LibdrmPageFlipEventPollStatus::Emitted
    );

    assert_eq!(LibdrmNativeReadLoopReport::callback_decoded(0), None);
    let rejected =
        LibdrmNativeReadLoopReport::callbacks_decoded(0, 2).expect("rejection count is observable");
    assert_eq!(
        rejected.status,
        LibdrmNativeReadLoopStatus::CallbackRejected
    );
    assert_eq!(rejected.decoded_callbacks, 0);
    assert_eq!(rejected.rejected_callbacks, 2);
    assert_eq!(
        rejected.into_poll_report().status,
        LibdrmPageFlipEventPollStatus::Idle
    );

    let mixed = LibdrmNativeReadLoopReport::callbacks_decoded(2, 1)
        .expect("decoded or rejected counts should produce a report");
    assert_eq!(mixed.status, LibdrmNativeReadLoopStatus::CallbackDecoded);
    assert_eq!(mixed.decoded_callbacks, 2);
    assert_eq!(mixed.rejected_callbacks, 1);
    assert_eq!(mixed.into_poll_report().callbacks.emitted, 2);

    assert_eq!(LibdrmNativeReadLoopReport::callbacks_decoded(0, 0), None);
    assert_eq!(
        LibdrmNativeReadLoopReport::read_failed()
            .into_poll_report()
            .status,
        LibdrmPageFlipEventPollStatus::Disconnected
    );
}

#[test]
fn native_libdrm_poller_skeleton_reports_idle_without_emitting_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(14).expect("nonzero generation should mint authority token");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller = NativeLibdrmPageFlipEventPoller::new(source);
    let (sender, receiver) = mpsc::sync_channel(1);

    assert_eq!(
        poller.source_report(),
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    );
    assert_eq!(
        poller.poll_page_flip_events(&sender, 4).status,
        LibdrmPageFlipEventPollStatus::Idle
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_libdrm_poller_drains_injected_callback_batch_without_fd_polling() {
    let authority =
        LibdrmBackendFdAuthority::new(15).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(4);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);
    assert_eq!(poller.pending_callback_count(), 3);

    let report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(report.callbacks.emitted, 2);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        poller.last_read_loop_report().status,
        LibdrmNativeReadLoopStatus::CallbackDecoded
    );
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 2);
    assert_eq!(poller.last_read_loop_report().rejected_callbacks, 1);
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("second callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 82,
        }
    );
}

#[test]
fn native_libdrm_reader_reads_bounded_callbacks_without_kms_identity() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let mut reader = FakeLibdrmNativePageFlipReader::new([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);

    let first = reader.read_ready_page_flip_callbacks(1);
    assert_eq!(
        first,
        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callback_decoded(1)
                .expect("one callback should produce a read report"),
            callbacks: vec![LibdrmNativePageFlipCallback::new(slot, 81)],
        }
    );
    assert_eq!(reader.queued_len(), 1);

    let second = reader.read_ready_page_flip_callbacks(4);
    assert_eq!(second.report.decoded_callbacks, 1);
    assert_eq!(
        second.callbacks,
        vec![LibdrmNativePageFlipCallback::new(slot, 82)]
    );
    assert_eq!(reader.queued_len(), 0);

    let empty = reader.read_ready_page_flip_callbacks(4);
    assert_eq!(empty.report, LibdrmNativeReadLoopReport::would_block());
    assert!(empty.callbacks.is_empty());
}

#[test]
fn native_libdrm_page_flip_event_reducer_uses_private_crtc_routes() {
    let crtc = drm::control::from_u32::<drm::control::crtc::Handle>(44)
        .expect("nonzero crtc handle should be constructible");
    let other_crtc = drm::control::from_u32::<drm::control::crtc::Handle>(45)
        .expect("nonzero crtc handle should be constructible");
    let slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let event = drm::control::PageFlipEvent {
        frame: 91,
        duration: std::time::Duration::from_millis(16),
        crtc,
    };

    assert_eq!(
        reduce_native_page_flip_event(&event, &[LibdrmNativeCrtcRoute::new(crtc, slot)]),
        Some(LibdrmNativePageFlipCallback::new(slot, 91))
    );
    assert_eq!(
        reduce_native_page_flip_event(&event, &[LibdrmNativeCrtcRoute::new(other_crtc, slot)]),
        None
    );
}

#[test]
fn native_libdrm_page_flip_event_reader_owns_device_and_private_crtc_routes() {
    let crtc = drm::control::from_u32::<drm::control::crtc::Handle>(44)
        .expect("nonzero crtc handle should be constructible");
    let slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let reader = NativeLibdrmPageFlipEventReader::new(())
        .with_crtc_routes([LibdrmNativeCrtcRoute::new(crtc, slot)]);

    assert_eq!(reader.crtc_route_count(), 1);
}

#[test]
fn native_libdrm_poller_reads_and_polls_bounded_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(24).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let mut reader = FakeLibdrmNativePageFlipReader::new([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);
    let (sender, receiver) = mpsc::sync_channel(4);

    let report = poller.read_and_poll_page_flip_events(&mut reader, &sender, 1, 4);

    assert_eq!(
        report,
        LibdrmNativeReadAndPollReport {
            read_loop: LibdrmNativeReadLoopReport::callback_decoded(1)
                .expect("one callback should produce a read report"),
            poll: LibdrmPageFlipEventPollReport {
                status: LibdrmPageFlipEventPollStatus::Emitted,
                callbacks: LivePageFlipCallbackSourceReport {
                    emitted: 1,
                    queued_remaining: 0,
                    backpressure: false,
                    disconnected: false,
                    max_reached: false,
                },
            },
        }
    );
    assert_eq!(reader.queued_len(), 1);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        receiver
            .try_recv()
            .expect("callback should be reduced and queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
}

#[test]
fn native_libdrm_poller_reports_read_failure_without_dropping_pending_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(25).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 81)]);
    reader.fail_next_read();
    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(slot, 80)]);
    let (sender, receiver) = mpsc::sync_channel(4);

    let report = poller.read_and_poll_page_flip_events(&mut reader, &sender, 4, 4);

    assert_eq!(report.read_loop, LibdrmNativeReadLoopReport::read_failed());
    assert_eq!(
        report.poll.status,
        LibdrmPageFlipEventPollStatus::Disconnected
    );
    assert_eq!(poller.pending_callback_count(), 1);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_libdrm_poller_retains_injected_callbacks_on_backpressure() {
    let authority =
        LibdrmBackendFdAuthority::new(16).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(1);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);

    let first = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(first.status, LibdrmPageFlipEventPollStatus::Backpressure);
    assert_eq!(first.callbacks.emitted, 1);
    assert_eq!(first.callbacks.queued_remaining, 1);
    assert_eq!(poller.pending_callback_count(), 1);
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 2);
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );

    let second = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(second.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(second.callbacks.emitted, 1);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 1);
    assert_eq!(
        receiver
            .try_recv()
            .expect("retained callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 82,
        }
    );
}

#[test]
fn native_libdrm_poller_retains_injected_callbacks_on_disconnected_queue() {
    let authority =
        LibdrmBackendFdAuthority::new(17).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(1);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);
    drop(receiver);

    let report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Disconnected);
    assert_eq!(report.callbacks.emitted, 0);
    assert_eq!(report.callbacks.queued_remaining, 2);
    assert_eq!(poller.pending_callback_count(), 2);
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 1);
}

#[test]
fn native_libdrm_poller_replaces_routes_without_dropping_pending_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(18).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(slot, 81)]);
    poller.replace_routes([LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(9),
    }]);

    assert_eq!(poller.route_count(), 1);
    assert_eq!(poller.pending_callback_count(), 1);

    let report = poller.poll_page_flip_events(&sender, 2);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        receiver
            .try_recv()
            .expect("callback should use replaced route"),
        LivePageFlipCallback {
            output: OutputId::from_raw(9),
            frame_serial: 81,
        }
    );
}

#[test]
fn native_libdrm_poller_rejects_pending_callbacks_after_route_removal() {
    let authority =
        LibdrmBackendFdAuthority::new(19).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(slot, 81)]);
    poller.replace_routes([]);

    let report = poller.poll_page_flip_events(&sender, 2);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Idle);
    assert_eq!(report.callbacks.emitted, 0);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        poller.last_read_loop_report().status,
        LibdrmNativeReadLoopStatus::CallbackRejected
    );
    assert_eq!(poller.last_read_loop_report().rejected_callbacks, 1);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_libdrm_poller_diagnostics_report_only_reduced_counts() {
    let authority =
        LibdrmBackendFdAuthority::new(20).expect("nonzero generation should mint authority token");
    let first_slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let second_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot: first_slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(first_slot, 81),
        LibdrmNativePageFlipCallback::new(second_slot, 82),
    ]);

    assert_eq!(
        poller.diagnostics(),
        LibdrmNativePollerDiagnostics {
            route_count: 1,
            pending_callbacks: 2,
            last_read_loop: LibdrmNativeReadLoopReport::idle(),
        }
    );

    poller.replace_routes([
        LibdrmNativeOutputRoute {
            slot: first_slot,
            output: OutputId::from_raw(7),
        },
        LibdrmNativeOutputRoute {
            slot: second_slot,
            output: OutputId::from_raw(9),
        },
    ]);
    let report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(
        poller.diagnostics(),
        LibdrmNativePollerDiagnostics {
            route_count: 2,
            pending_callbacks: 0,
            last_read_loop: LibdrmNativeReadLoopReport::callback_decoded(2)
                .expect("decoded count should build a report"),
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("second callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(9),
            frame_serial: 82,
        }
    );
}

#[test]
fn live_runtime_assembly_reports_reduced_native_libdrm_poller_diagnostics() {
    let root = ready_drm_sysfs_fixture("native-libdrm-runtime-diagnostics");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(21).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
    ]);
    let poll_report = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll_report.status, LibdrmPageFlipEventPollStatus::Emitted);

    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_native_libdrm_poller_diagnostics(poller.diagnostics())
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));

    assert_eq!(
        assembly.libdrm_poller_diagnostics(),
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::CallbackDecoded,
            route_count: 1,
            pending_callbacks: 0,
            decoded_callbacks: 1,
            rejected_callbacks: 1,
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain callback and report diagnostics");

    assert_eq!(
        tick.libdrm_poller,
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::CallbackDecoded,
            route_count: 1,
            pending_callbacks: 0,
            decoded_callbacks: 1,
            rejected_callbacks: 1,
        }
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(81),
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_poller_constructs_routes_from_discovered_outputs_without_kms_identity() {
    let root = multi_output_drm_sysfs_fixture("native-libdrm-discovered-routes");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(22).expect("nonzero generation should mint authority token");
    let routes = report.native_libdrm_output_routes();

    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].slot.raw(), 1);
    assert_eq!(routes[0].output, OutputId::from_raw(1));
    assert_eq!(routes[1].slot.raw(), 2);
    assert_eq!(routes[1].output, OutputId::from_raw(2));
    assert_eq!(
        report.native_libdrm_poller_startup_report(),
        LiveLibdrmPollerStartupReport {
            status: LiveLibdrmPollerStartupStatus::Ready,
            route_count: 2,
        }
    );

    let mut poller = report
        .native_libdrm_poller_from_authority(authority)
        .expect("ready discovery should construct native poller");
    let (sender, receiver) = mpsc::sync_channel(2);

    assert_eq!(poller.diagnostics().route_count, 2);
    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(routes[1].slot, 90)]);

    let poll_report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(poll_report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(
        receiver
            .try_recv()
            .expect("callback should map through reduced output route"),
        LivePageFlipCallback {
            output: OutputId::from_raw(2),
            frame_serial: 90,
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_poller_construction_fails_closed_without_outputs() {
    let root = std::env::temp_dir().join("sophia-backend-live-native-libdrm-no-routes");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(23).expect("nonzero generation should mint authority token");

    assert!(report.native_libdrm_output_routes().is_empty());
    assert_eq!(
        report.native_libdrm_poller_startup_report(),
        LiveLibdrmPollerStartupReport {
            status: LiveLibdrmPollerStartupStatus::NoOutputs,
            route_count: 0,
        }
    );
    assert!(
        report
            .native_libdrm_poller_from_authority(authority)
            .is_none()
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_page_flip_callback_decodes_without_native_resource_identity() {
    assert_eq!(LibdrmNativeOutputSlot::new(0), None);
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    assert_eq!(slot.raw(), 2);

    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
    }];
    let callback = LibdrmNativePageFlipCallback::new(slot, 81);

    assert_eq!(
        callback.decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::Decoded,
            callback: Some(LivePageFlipCallback {
                output: OutputId::from_raw(7),
                frame_serial: 81,
            }),
        }
    );

    let unknown_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    assert_eq!(
        LibdrmNativePageFlipCallback::new(unknown_slot, 82).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::UnknownOutputSlot,
            callback: None,
        }
    );
    assert_eq!(
        LibdrmNativePageFlipCallback::new(slot, 0).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::InvalidFrameSerial,
            callback: None,
        }
    );
}

#[test]
fn native_libdrm_page_flip_decode_batch_is_bounded_and_reduced() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let unknown_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
    }];
    let callbacks = [
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
        LibdrmNativePageFlipCallback::new(unknown_slot, 82),
        LibdrmNativePageFlipCallback::new(slot, 83),
    ];
    let (sender, receiver) = mpsc::sync_channel(4);

    let report = decode_native_page_flip_batch(&callbacks, &routes, &sender, 4);

    assert_eq!(
        report.read_loop.status,
        LibdrmNativeReadLoopStatus::CallbackDecoded
    );
    assert_eq!(report.read_loop.decoded_callbacks, 2);
    assert_eq!(report.read_loop.rejected_callbacks, 2);
    assert_eq!(report.poll.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(report.poll.callbacks.emitted, 2);
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("second callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 83,
        }
    );
    assert!(receiver.try_recv().is_err());

    let (sender, _receiver) = mpsc::sync_channel(4);
    let limited = decode_native_page_flip_batch(&callbacks, &routes, &sender, 1);
    assert_eq!(limited.read_loop.decoded_callbacks, 1);
    assert_eq!(limited.poll.callbacks.emitted, 1);
    assert_eq!(limited.poll.callbacks.max_reached, true);
    assert_eq!(limited.poll.callbacks.queued_remaining, 3);
}

#[test]
fn native_libdrm_page_flip_decode_batch_reports_backpressure_without_native_identity() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
    }];
    let callbacks = [
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ];
    let (sender, _receiver) = mpsc::sync_channel(1);

    let report = decode_native_page_flip_batch(&callbacks, &routes, &sender, 4);

    assert_eq!(report.read_loop.decoded_callbacks, 2);
    assert_eq!(report.read_loop.rejected_callbacks, 0);
    assert_eq!(
        report.poll.status,
        LibdrmPageFlipEventPollStatus::Backpressure
    );
    assert_eq!(report.poll.callbacks.emitted, 1);
    assert_eq!(report.poll.callbacks.queued_remaining, 1);
}

#[test]
fn libdrm_event_poll_report_projects_source_state_without_native_identity() {
    assert_eq!(
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        })
        .status,
        LibdrmPageFlipEventPollStatus::Idle
    );

    assert_eq!(
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 2,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        })
        .status,
        LibdrmPageFlipEventPollStatus::Emitted
    );

    assert_eq!(
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 1,
            backpressure: true,
            disconnected: false,
            max_reached: false,
        })
        .status,
        LibdrmPageFlipEventPollStatus::Backpressure
    );
}

#[test]
fn fake_libdrm_page_flip_poller_feeds_runtime_queue() {
    let root = ready_drm_sysfs_fixture("fake-libdrm-page-flip-poller");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut poller = FakeLibdrmPageFlipEventPoller::new([
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 61,
        },
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 62,
        },
    ]);

    let poll = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll.status, LibdrmPageFlipEventPollStatus::Backpressure);
    assert_eq!(poll.callbacks.emitted, 1);
    assert_eq!(poller.queued_len(), 1);

    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first callback");
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(61),
        }
    );

    let poll = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(poller.queued_len(), 0);
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain second callback");
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(62),
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

fn ready_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let connector = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    root
}

fn multi_output_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let first = root.join("card0-DP-1");
    let second = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    write_fixture_file(&first, "status", "connected\n");
    write_fixture_file(&first, "modes", "1920x1080\n");
    write_fixture_file(&first, "connector_id", "1234\n");
    write_fixture_file(&first, "crtc_id", "2234\n");
    write_fixture_file(&second, "status", "connected\n");
    write_fixture_file(&second, "modes", "2560x1440\n");
    write_fixture_file(&second, "connector_id", "9876\n");
    write_fixture_file(&second, "crtc_id", "8876\n");
    root
}

fn write_fixture_file(root: &std::path::Path, name: &str, contents: &str) {
    std::fs::write(root.join(name), contents).unwrap();
}

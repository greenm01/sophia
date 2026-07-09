#![cfg(feature = "libdrm-events")]

use std::sync::mpsc;

use sophia_backend_live::{
    CompositorBackendTickInput, FakeLibdrmPageFlipEventPoller, LibdrmBackendFdAuthority,
    LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
    LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus,
    LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus, LibdrmNativeOutputRoute,
    LibdrmNativeOutputSlot, LibdrmNativePageFlipCallback, LibdrmNativePageFlipDecodeReport,
    LibdrmNativePageFlipDecodeStatus, LibdrmNativePageFlipSource, LibdrmNativePageFlipSourceReport,
    LibdrmNativePageFlipSourceStatus, LibdrmNativeReadLoopReport, LibdrmNativeReadLoopStatus,
    LibdrmPageFlipEventPollReport, LibdrmPageFlipEventPollStatus, LibdrmPageFlipEventPoller,
    LiveBackendConfig, LivePageFlipCallback, LivePageFlipCallbackQueue,
    LivePageFlipCallbackSourceReport, LivePageFlipEvent, LivePageFlipEventStatus,
    NativeLibdrmPageFlipEventPoller, OutputId, QueuedInputPoller, decode_native_page_flip_batch,
    discover_live_backend, libdrm_dependency_admission_report, libdrm_fd_authority_report,
    native_libdrm_event_adapter_report, native_libdrm_event_adapter_report_for_authority,
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

fn write_fixture_file(root: &std::path::Path, name: &str, contents: &str) {
    std::fs::write(root.join(name), contents).unwrap();
}

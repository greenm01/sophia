#![cfg(feature = "libinput-events")]

use std::fs;
use std::path::{Path, PathBuf};

use sophia_backend_live::{
    CompositorBackendTickInput, DeviceId, FakeLiveLibinputEventReader, InputEventPacket,
    LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventIngest, LibinputEventSource,
    LibinputNativeEventAdapterReport, LibinputNativeEventAdapterStatus,
    LibinputNativeEventReadReport, LibinputNativeEventReadStatus, LibinputPhysicalInputAdapter,
    LiveBackendConfig, LiveHardwareValidationGateReport, LiveHardwareValidationGateStatus,
    LiveHardwareValidationSmokeReport, LiveHardwareValidationSmokeStatus,
    LiveHardwareValidationTarget, NativeLibinputEventPoller, NonBlockingInputPoller, SeatId,
    discover_live_backend, native_libinput_event_adapter_report,
    real_libinput_events_validation_gate, real_libinput_events_validation_smoke_report,
};
use sophia_protocol::{InputEventKind, Point};

#[test]
fn native_libinput_event_adapter_skeleton_reports_ready_without_opening_devices() {
    assert_eq!(
        native_libinput_event_adapter_report(),
        LibinputNativeEventAdapterReport {
            status: LibinputNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn real_libinput_event_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibinputEvents,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::LibinputEvents,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert!(!skipped.is_requested());
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibinputEvents,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_libinput_events_validation_gate().target,
        LiveHardwareValidationTarget::LibinputEvents
    );
}

#[test]
fn real_libinput_event_validation_smoke_fails_closed_without_native_reader() {
    let skipped = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibinputEvents,
            false,
        ),
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibinputEvents,
            status: LiveHardwareValidationSmokeStatus::SkippedOptInRequired,
        }
    );

    let requested = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibinputEvents,
            true,
        ),
    );
    assert_eq!(
        requested,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibinputEvents,
            status: LiveHardwareValidationSmokeStatus::BackendUnavailable,
        }
    );

    assert_eq!(
        real_libinput_events_validation_smoke_report().target,
        LiveHardwareValidationTarget::LibinputEvents
    );
}

#[test]
fn native_libinput_event_poller_reads_bounded_events() {
    let mut poller = NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([
            motion_event(1, 10.0, 20.0),
            motion_event(2, 11.0, 21.0),
        ]),
        1,
    );

    let first = poller.poll_ready().expect("fake input read should succeed");
    assert_eq!(first, vec![motion_event(1, 10.0, 20.0)]);
    assert_eq!(
        poller.last_read_report(),
        LibinputNativeEventReadReport {
            status: LibinputNativeEventReadStatus::EventsRead,
            events_read: 1,
            queued_remaining: 1,
        }
    );
    assert_eq!(poller.reader().queued_len(), 1);

    let second = poller.poll_ready().expect("fake input read should succeed");
    assert_eq!(second, vec![motion_event(2, 11.0, 21.0)]);
    assert_eq!(poller.reader().queued_len(), 0);

    let empty = poller.poll_ready().expect("empty fake read should succeed");
    assert!(empty.is_empty());
    assert_eq!(
        poller.last_read_report(),
        LibinputNativeEventReadReport::idle()
    );
}

#[test]
fn native_libinput_event_poller_reports_reduced_read_failure() {
    let mut poller = NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([motion_event(1, 10.0, 20.0)]),
        4,
    );
    poller.reader_mut().fail_next_read();

    assert!(poller.poll_ready().is_err());
    assert_eq!(
        poller.last_read_report(),
        LibinputNativeEventReadReport::read_failed()
    );
    assert_eq!(poller.reader().queued_len(), 1);
}

#[test]
fn native_libinput_event_poller_feeds_engine_input_adapter_contract() {
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let poller = NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([
            motion_event(1, 10.0, 20.0),
            unknown_device_motion_event(2, 11.0, 21.0),
        ]),
        4,
    );
    let mut adapter = LibinputPhysicalInputAdapter::new(poller, source);

    let report = adapter
        .poll_once()
        .expect("fake native poller should feed adapter");

    assert_eq!(report.polled, 2);
    assert_eq!(report.accepted, 1);
    assert_eq!(report.rejected, vec![LibinputEventIngest::UnknownDevice]);
    assert_eq!(adapter.source().pending_len(), 1);
}

#[test]
fn live_runtime_assembly_runs_tick_with_native_shaped_input_poller() {
    let root = ready_drm_sysfs_fixture("native-input-runtime");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let poller = NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([motion_event(1, 10.0, 20.0)]),
        4,
    );
    let mut assembly = discover_live_backend(&config)
        .into_live_runtime_assembly(poller)
        .expect("ready startup should accept native-shaped input poller");

    let report = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("native-shaped input poller should drive runtime tick");

    assert_eq!(report.engine.input_poll.polled, 1);
    assert_eq!(report.engine.input_poll.accepted, 1);
    assert!(report.engine.input_poll.rejected.is_empty());
    assert_eq!(assembly.assembly().input().source().pending_len(), 1);

    fs::remove_dir_all(root).unwrap();
}

fn motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial * 10,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x, y }),
        target_surface: None,
        target_window: None,
        local_position: None,
    }
}

fn unknown_device_motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        device: DeviceId::from_raw(99),
        ..motion_event(serial, x, y)
    }
}

fn ready_drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sophia-backend-live-libinput-events-{name}-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    root
}

fn write_fixture_file(root: &Path, name: &str, value: &str) {
    fs::write(root.join(name), value).unwrap();
}

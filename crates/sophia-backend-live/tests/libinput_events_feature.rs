#![cfg(feature = "libinput-events")]

use sophia_backend_live::{
    DeviceId, FakeLiveLibinputEventReader, InputEventPacket, LibinputDeviceDescriptor,
    LibinputDeviceKind, LibinputEventIngest, LibinputEventSource, LibinputNativeEventAdapterReport,
    LibinputNativeEventAdapterStatus, LibinputNativeEventReadReport, LibinputNativeEventReadStatus,
    LibinputPhysicalInputAdapter, NativeLibinputEventPoller, NonBlockingInputPoller, SeatId,
    native_libinput_event_adapter_report,
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

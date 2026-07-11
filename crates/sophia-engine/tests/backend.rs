mod support;
use support::*;

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
fn drm_kms_output_registry_tracks_connector_mode_and_scale() {
    let descriptor = DrmKmsOutputDescriptor {
        output: OutputId::from_raw(7),
        connector_id: 42,
        crtc_id: 99,
        mode: DrmKmsMode::new(1920, 1080, 60_000),
        scale: 2,
    };
    let mut registry = DrmKmsOutputRegistry::new();

    registry.upsert(descriptor);

    assert_eq!(registry.get(OutputId::from_raw(7)), Some(&descriptor));
    assert_eq!(registry.outputs().count(), 1);
    assert_eq!(
        registry.primary_engine_output(),
        Some(descriptor.as_engine_output())
    );
    assert_eq!(
        descriptor.as_engine_output().size,
        sophia_protocol::Size {
            width: 1920,
            height: 1080,
        }
    );
    assert_eq!(descriptor.as_engine_output().scale, 2);
    assert_eq!(registry.remove(OutputId::from_raw(7)), Some(descriptor));
    assert!(registry.primary_engine_output().is_none());
}

#[test]
fn drm_kms_output_registry_rejects_unbounded_output_growth() {
    let mut registry = DrmKmsOutputRegistry::new();
    for index in 0..sophia_engine::MAX_DRM_KMS_OUTPUTS {
        let raw = u64::try_from(index + 1).unwrap();
        assert_eq!(
            registry.upsert(DrmKmsOutputDescriptor {
                output: OutputId::from_raw(raw),
                connector_id: u32::try_from(raw).unwrap(),
                crtc_id: u32::try_from(raw + 100).unwrap(),
                mode: DrmKmsMode::new(1920, 1080, 60_000),
                scale: 1,
            }),
            DrmKmsOutputRegistryUpdate::Inserted
        );
    }

    assert_eq!(registry.len(), sophia_engine::MAX_DRM_KMS_OUTPUTS);
    assert_eq!(
        registry.upsert(DrmKmsOutputDescriptor {
            output: OutputId::from_raw(99),
            connector_id: 99,
            crtc_id: 199,
            mode: DrmKmsMode::new(1920, 1080, 60_000),
            scale: 1,
        }),
        DrmKmsOutputRegistryUpdate::CapacityExceeded
    );
}

#[test]
fn drm_kms_descriptor_can_seed_engine_output() {
    let descriptor = DrmKmsOutputDescriptor {
        output: OutputId::from_raw(8),
        connector_id: 43,
        crtc_id: 100,
        mode: DrmKmsMode::new(2560, 1440, 144_000),
        scale: 1,
    };
    let engine = HeadlessEngine::new(descriptor.as_engine_output());
    let frame = engine
        .plan_frame(
            FramePlanRequest {
                output: OutputId::from_raw(8),
                frame_serial: 1,
            },
            Vec::new(),
        )
        .unwrap();

    assert_eq!(frame.output_size, descriptor.mode.size);
    assert_eq!(frame.output_scale, descriptor.scale);
}

#[test]
fn drm_kms_sysfs_discovery_finds_connected_outputs() {
    let root = drm_sysfs_fixture("connected");
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n1280x720\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    write_fixture_file(&connector, "scale", "2\n");

    let registry = discover_drm_kms_outputs_from_sysfs(&root).unwrap();
    let output = registry.get(OutputId::from_raw(1)).unwrap();

    assert_eq!(registry.outputs().count(), 1);
    assert_eq!(output.connector_id, 42);
    assert_eq!(output.crtc_id, 99);
    assert_eq!(output.mode, DrmKmsMode::new(1920, 1080, 60_000));
    assert_eq!(output.scale, 2);
    assert_eq!(
        registry.primary_engine_output(),
        Some(output.as_engine_output())
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn drm_kms_sysfs_discovery_ignores_disconnected_or_modeless_outputs() {
    let root = drm_sysfs_fixture("filtered");
    let disconnected = root.join("card0-DP-1");
    let modeless = root.join("card0-HDMI-A-1");
    let connected = root.join("card0-eDP-1");
    fs::create_dir_all(&disconnected).unwrap();
    fs::create_dir_all(&modeless).unwrap();
    fs::create_dir_all(&connected).unwrap();
    write_fixture_file(&disconnected, "status", "disconnected\n");
    write_fixture_file(&disconnected, "modes", "3840x2160\n");
    write_fixture_file(&modeless, "status", "connected\n");
    write_fixture_file(&modeless, "modes", "\n");
    write_fixture_file(&connected, "status", "connected\n");
    write_fixture_file(&connected, "modes", "2560x1440\n");

    let registry = discover_drm_kms_outputs_from_sysfs(&root).unwrap();
    let output = registry.get(OutputId::from_raw(3)).unwrap();

    assert_eq!(registry.outputs().count(), 1);
    assert_eq!(output.connector_id, 3);
    assert_eq!(output.crtc_id, 0);
    assert_eq!(output.mode, DrmKmsMode::new(2560, 1440, 60_000));
    assert_eq!(output.scale, 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn libinput_event_source_accepts_registered_device_events_in_order() {
    let mut source = LibinputEventSource::new();
    let device = LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    };
    source.register_device(device);

    assert_eq!(source.device(DeviceId::from_raw(2)), Some(&device));
    assert_eq!(source.devices().count(), 1);
    assert_eq!(
        source.push_event(motion_event(1, 10.0, 20.0)),
        LibinputEventIngest::Accepted
    );
    assert_eq!(
        source.push_event(motion_event(2, 11.0, 21.0)),
        LibinputEventIngest::Accepted
    );

    let events = source.drain_events();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].serial, 1);
    assert_eq!(events[1].serial, 2);
    assert_eq!(source.pending_len(), 0);
    assert_eq!(source.remove_device(DeviceId::from_raw(2)), Some(device));
}

#[test]
fn libinput_event_source_rejects_unknown_or_wrong_seat_events() {
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(9),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Keyboard,
    });

    assert_eq!(
        source.push_event(motion_event(1, 0.0, 0.0)),
        LibinputEventIngest::SeatMismatch
    );

    let mut unknown_device_event = motion_event(2, 0.0, 0.0);
    unknown_device_event.device = DeviceId::from_raw(99);
    assert_eq!(
        source.push_event(unknown_device_event),
        LibinputEventIngest::UnknownDevice
    );
    assert_eq!(source.pending_len(), 0);
}

#[test]
fn libinput_physical_input_adapter_polls_ready_events_without_blocking() {
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let poller = QueuedInputPoller::new(vec![
        motion_event(1, 10.0, 20.0),
        motion_event(2, 11.0, 21.0),
    ]);
    let mut adapter = LibinputPhysicalInputAdapter::new(poller, source);

    let report = adapter.poll_once().unwrap();

    assert_eq!(report.polled, 2);
    assert_eq!(report.accepted, 2);
    assert!(report.rejected.is_empty());
    assert_eq!(adapter.source().pending_len(), 2);
    let events = adapter.source_mut().drain_events();
    assert_eq!(events[0].serial, 1);
    assert_eq!(events[1].serial, 2);

    let empty_report = adapter.poll_once().unwrap();
    assert_eq!(empty_report.polled, 0);
    assert_eq!(empty_report.accepted, 0);
    assert!(empty_report.rejected.is_empty());
}

#[test]
fn libinput_physical_input_adapter_reports_rejected_events() {
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(9),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let mut unknown_device_event = motion_event(2, 0.0, 0.0);
    unknown_device_event.device = DeviceId::from_raw(99);
    let poller = QueuedInputPoller::new(vec![motion_event(1, 0.0, 0.0), unknown_device_event]);
    let mut adapter = LibinputPhysicalInputAdapter::new(poller, source);

    let report = adapter.poll_once().unwrap();

    assert_eq!(report.polled, 2);
    assert_eq!(report.accepted, 0);
    assert_eq!(
        report.rejected,
        vec![
            LibinputEventIngest::SeatMismatch,
            LibinputEventIngest::UnknownDevice,
        ]
    );
    assert_eq!(adapter.source().pending_len(), 0);
}

#[test]
fn live_backend_discovery_can_seed_headless_assembly_without_policy_changes() {
    let root = drm_sysfs_fixture("live-ready");
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    let output_backend = SysfsDrmKmsOutputBackend::new(&root);
    let input_backend = StaticInputDiscoveryBackend::new(vec![LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    }]);

    let report = discover_live_compositor_backend(&output_backend, &input_backend);

    assert!(report.is_ready());
    assert_eq!(report.status, LiveCompositorBackendDiscoveryStatus::Ready);
    assert_eq!(
        report.selected_output,
        Some(HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1920,
                height: 1080,
            },
            scale: 1,
        })
    );
    assert_eq!(report.input_source.devices().count(), 1);

    let assembly = report
        .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
        .expect("ready backend discovery should create a deterministic assembly");
    assert_eq!(
        assembly.outputs().primary_engine_output(),
        Some(HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1920,
                height: 1080,
            },
            scale: 1,
        })
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_discovery_fails_closed_when_no_outputs_exist() {
    let root = drm_sysfs_fixture("live-no-outputs");
    let output_backend = SysfsDrmKmsOutputBackend::new(&root);
    let input_backend = StaticInputDiscoveryBackend::new(vec![LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    }]);

    let report = discover_live_compositor_backend(&output_backend, &input_backend);

    assert_eq!(
        report.status,
        LiveCompositorBackendDiscoveryStatus::NoOutputs
    );
    assert!(!report.is_ready());
    assert_eq!(report.selected_output, None);
    assert_eq!(report.input_source.devices().count(), 0);
    assert!(
        report
            .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

#[derive(Clone, Debug)]
struct FailingOutputBackend;

impl OutputDiscoveryBackend for FailingOutputBackend {
    fn discover_outputs(&self) -> IoResult<DrmKmsOutputRegistry> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ))
    }
}

#[test]
fn live_backend_discovery_reports_output_errors_without_starting_assembly() {
    let input_backend = StaticInputDiscoveryBackend::new(Vec::new());

    let report = discover_live_compositor_backend(&FailingOutputBackend, &input_backend);

    assert_eq!(
        report.status,
        LiveCompositorBackendDiscoveryStatus::OutputDiscoveryFailed {
            message: "denied".to_owned(),
        }
    );
    assert_eq!(report.outputs.outputs().count(), 0);
    assert_eq!(report.selected_output, None);
    assert!(
        report
            .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
            .is_none()
    );
}

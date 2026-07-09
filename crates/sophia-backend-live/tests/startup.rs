use std::fs;
use std::path::{Path, PathBuf};

use sophia_backend_live::{
    DeviceId, HeadlessOutput, LibinputDeviceDescriptor, LibinputDeviceKind, LiveBackendConfig,
    LiveCompositorBackendDiscoveryStatus, OutputId, QueuedInputPoller, RendererSelection, SeatId,
    Size, discover_live_backend,
};

#[test]
fn live_backend_startup_can_seed_headless_assembly_from_sysfs_and_static_input() {
    let root = drm_sysfs_fixture("ready");
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });

    let report = discover_live_backend(&config);

    assert_eq!(
        report.status(),
        &LiveCompositorBackendDiscoveryStatus::Ready
    );
    assert_eq!(
        report.selected_output(),
        Some(HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1920,
                height: 1080,
            },
            scale: 1,
        })
    );
    let assembly = report
        .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
        .expect("ready startup should seed assembly");
    assert_eq!(assembly.input().source().devices().count(), 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_startup_fails_closed_without_connected_outputs() {
    let root = drm_sysfs_fixture("no-output");
    let config = LiveBackendConfig::new(&root);

    let report = discover_live_backend(&config);

    assert_eq!(
        report.status(),
        &LiveCompositorBackendDiscoveryStatus::NoOutputs
    );
    assert_eq!(report.selected_output(), None);
    assert!(
        report
            .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

fn drm_sysfs_fixture(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("sophia-backend-live-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

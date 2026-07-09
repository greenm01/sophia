use std::fs;
use std::path::{Path, PathBuf};

use sophia_backend_live::{
    DeviceId, HeadlessOutput, LibinputDeviceDescriptor, LibinputDeviceKind, LiveBackendConfig,
    LiveBackendDependencyDecision, LiveBackendDependencyKind, LiveBackendDependencyUse,
    LiveCompositorBackendDiscoveryStatus, LiveRendererImportBoundary, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportStartupStatus, OutputId, QueuedInputPoller,
    RendererSelection, SeatId, Size, discover_live_backend, live_backend_dependency_decision,
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

#[test]
fn live_backend_startup_uses_cpu_renderer_until_native_import_is_configured() {
    let root = ready_drm_sysfs_fixture("renderer-default");
    let config = LiveBackendConfig::new(&root);

    let report = discover_live_backend(&config);

    assert_eq!(report.renderer_selection(), RendererSelection::CpuFallback);
    assert_eq!(
        report.renderer_import_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
    let assembly = report
        .into_configured_headless_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed assembly");
    assert_eq!(assembly.renderer(), RendererSelection::CpuFallback);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_startup_admits_native_renderer_import_only_when_configured() {
    let root = ready_drm_sysfs_fixture("renderer-native");
    let config = LiveBackendConfig::new(&root).with_renderer_import_boundary(
        LiveRendererImportBoundary::with_native_imports(true, false),
    );

    let report = discover_live_backend(&config);

    assert_eq!(
        report.renderer_selection(),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        }
    );
    assert_eq!(
        report.renderer_import_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
    let assembly = report
        .into_configured_headless_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed assembly");
    assert_eq!(
        assembly.renderer(),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dependency_policy_allows_libdrm_and_libinput_at_live_backend_seams() {
    assert!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::LibDrm,
            LiveBackendDependencyUse::Discovery,
        )
        .is_allowed()
    );
    assert!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::LibInput,
            LiveBackendDependencyUse::RuntimePolling,
        )
        .is_allowed()
    );
}

#[test]
fn dependency_policy_defers_gpu_and_shared_memory_imports() {
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Gbm,
            LiveBackendDependencyUse::RendererImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "live renderer import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::DmaBuf,
            LiveBackendDependencyUse::Discovery,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "live renderer import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::MitShm,
            LiveBackendDependencyUse::SharedMemoryImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "bounded shared-memory import boundary",
        }
    );
}

fn drm_sysfs_fixture(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("sophia-backend-live-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn ready_drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = drm_sysfs_fixture(name);
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1280x720\n");
    root
}

fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

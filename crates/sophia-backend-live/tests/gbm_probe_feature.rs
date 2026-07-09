#![cfg(feature = "gbm-probe")]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sophia_backend_live::{
    LiveBackendConfig, LiveRenderDeviceDiscoveryReport, LiveRenderDeviceDiscoveryStatus,
    LiveRendererImportBoundary, LiveRendererImportHealth, LiveRendererImportPathStatus,
    LiveRendererImportStartupStatus, LiveRendererPreference, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation, QueuedInputPoller, RenderDeviceDiscoveryBackend,
    RendererSelection, discover_live_backend,
};

struct MissingRenderDevice;

impl RenderDeviceDiscoveryBackend for MissingRenderDevice {
    type Device = std::fs::File;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        Err(io::Error::from_raw_os_error(19))
    }
}

struct UnexpectedRenderDeviceOpen;

impl RenderDeviceDiscoveryBackend for UnexpectedRenderDeviceOpen {
    type Device = std::fs::File;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        panic!("CPU fallback startup must not open a render device");
    }
}

#[test]
fn gbm_probe_keeps_default_startup_on_cpu_fallback() {
    let config = LiveBackendConfig::new("/does/not/matter");
    let report = discover_live_backend(&config);

    let probe = report.renderer_probe_report_with_gbm_device(&UnexpectedRenderDeviceOpen);

    assert_eq!(
        probe.render_device,
        LiveRenderDeviceDiscoveryReport {
            status: LiveRenderDeviceDiscoveryStatus::NotRequested,
        }
    );
    assert_eq!(
        probe.renderer_import,
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
}

#[test]
fn gbm_probe_cpu_only_never_opens_render_device_even_when_imports_are_configured() {
    let config = LiveBackendConfig::new("/does/not/matter")
        .with_renderer_import_boundary(LiveRendererImportBoundary::with_native_imports(false, true))
        .with_renderer_preference(LiveRendererPreference::CpuOnly);
    let report = discover_live_backend(&config);
    let probe = report.renderer_probe_report_with_gbm_device(&UnexpectedRenderDeviceOpen);

    assert_eq!(
        probe.render_device,
        LiveRenderDeviceDiscoveryReport {
            status: LiveRenderDeviceDiscoveryStatus::NotRequested,
        }
    );
    assert_eq!(
        probe.renderer_import,
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
}

#[test]
fn gbm_probe_degrades_dmabuf_without_leaking_device_error() {
    let config = LiveBackendConfig::new("/does/not/matter").with_renderer_import_boundary(
        LiveRendererImportBoundary::with_native_imports(false, true),
    );
    let report = discover_live_backend(&config);
    let probe = report.renderer_probe_report_with_gbm_device(&MissingRenderDevice);

    assert_eq!(
        probe.render_device,
        LiveRenderDeviceDiscoveryReport {
            status: LiveRenderDeviceDiscoveryStatus::Unavailable,
        }
    );
    assert_eq!(
        probe.renderer_import,
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

#[test]
fn gbm_probe_preserves_xpixmap_status_when_dmabuf_degrades() {
    let config = LiveBackendConfig::new("/does/not/matter")
        .with_renderer_import_boundary(LiveRendererImportBoundary::with_native_imports(true, true));
    let report = discover_live_backend(&config);

    assert_eq!(
        report.renderer_import_status_with_gbm_device(&MissingRenderDevice),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

#[test]
fn gpu_preferred_degraded_gbm_assembles_cpu_fallback_with_reduced_observation() {
    let root = ready_drm_sysfs_fixture("gpu-preferred-degraded");
    let config = LiveBackendConfig::new(&root)
        .with_renderer_import_boundary(LiveRendererImportBoundary::with_native_imports(false, true))
        .with_renderer_preference(LiveRendererPreference::GpuPreferred);
    let report = discover_live_backend(&config);

    let assembly = report
        .into_live_runtime_assembly_with_gbm_device(
            QueuedInputPoller::default(),
            &MissingRenderDevice,
        )
        .expect("GPU-preferred degraded GBM should assemble CPU fallback");

    assert_eq!(
        assembly.assembly().renderer(),
        RendererSelection::CpuFallback
    );
    assert_eq!(
        assembly.renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
            selection: LiveRendererSelectionObservation::CpuFallback,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gpu_required_degraded_gbm_fails_closed_without_runtime_assembly() {
    let root = ready_drm_sysfs_fixture("gpu-required-degraded");
    let config = LiveBackendConfig::new(&root)
        .with_renderer_import_boundary(LiveRendererImportBoundary::with_native_imports(false, true))
        .with_renderer_preference(LiveRendererPreference::GpuRequired);
    let report = discover_live_backend(&config);

    assert!(
        report
            .into_live_runtime_assembly_with_gbm_device(
                QueuedInputPoller::default(),
                &MissingRenderDevice,
            )
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

fn ready_drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sophia-backend-live-gbm-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1280x720\n");
    root
}

fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

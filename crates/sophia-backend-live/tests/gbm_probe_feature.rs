#![cfg(feature = "gbm-probe")]

use std::io;

use sophia_backend_live::{
    LiveBackendConfig, LiveRenderDeviceDiscoveryReport, LiveRenderDeviceDiscoveryStatus,
    LiveRendererImportBoundary, LiveRendererImportHealth, LiveRendererImportPathStatus,
    LiveRendererImportStartupStatus, RenderDeviceDiscoveryBackend, discover_live_backend,
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

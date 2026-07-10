#![cfg(feature = "gbm-probe")]

use sophia_renderer_live::{
    FakeGbmCapabilityProbe, GbmCapabilityProbeReport, GbmCapabilityProbeStatus,
    GbmRenderDeviceToken, LiveRendererImportHealth, LiveRendererImportPathStatus,
    LiveRendererImportStartupStatus, LiveRendererScanoutBufferExportStatus,
    NativeGbmCapabilityProbe, NativeGbmScanoutBufferExporter,
};

#[test]
fn gbm_probe_uses_reduced_render_device_tokens() {
    assert_eq!(GbmRenderDeviceToken::from_raw(0), None);
    assert_eq!(
        GbmRenderDeviceToken::from_raw(42),
        Some(GbmRenderDeviceToken { raw: 42 })
    );
}

#[test]
fn fake_gbm_probe_reports_native_capability_from_reduced_device_token() {
    assert_eq!(
        FakeGbmCapabilityProbe::new(GbmRenderDeviceToken::from_raw(42)).startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Enabled,
        }
    );
}

#[test]
fn fake_gbm_probe_reports_degraded_health_when_unavailable() {
    assert_eq!(
        FakeGbmCapabilityProbe::new(None).startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

#[test]
fn fake_gbm_probe_reports_reduced_capability_reason() {
    assert_eq!(
        FakeGbmCapabilityProbe::new(None).probe_report(),
        GbmCapabilityProbeReport {
            status: GbmCapabilityProbeStatus::ReducedDeviceUnavailable,
            startup_status: LiveRendererImportStartupStatus {
                health: LiveRendererImportHealth::Degraded,
                xpixmap: LiveRendererImportPathStatus::Disabled,
                dmabuf: LiveRendererImportPathStatus::Degraded,
            },
        }
    );
}

#[test]
fn native_gbm_probe_maps_missing_reduced_device_to_degraded_health() {
    assert_eq!(
        NativeGbmCapabilityProbe::new(None).startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

#[test]
fn native_gbm_probe_stays_reduced_at_public_boundary() {
    assert_eq!(
        NativeGbmCapabilityProbe::new(GbmRenderDeviceToken::from_raw(42)).startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Enabled,
        }
    );
}

#[test]
fn native_gbm_probe_maps_backend_device_open_failure_to_degraded_health() {
    let missing_device = Err(std::io::Error::from_raw_os_error(19));

    assert_eq!(
        NativeGbmCapabilityProbe::startup_status_from_backend_device_result::<std::fs::File>(
            missing_device,
        ),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

#[test]
fn native_gbm_probe_degrades_when_private_buffer_allocation_fails() {
    let invalid_render_device = std::fs::File::open("/dev/null").unwrap();

    let probe_report =
        NativeGbmCapabilityProbe::probe_report_from_backend_device(invalid_render_device);

    assert_eq!(
        probe_report,
        GbmCapabilityProbeReport {
            status: GbmCapabilityProbeStatus::PrivateAllocationUnavailable,
            startup_status: LiveRendererImportStartupStatus {
                health: LiveRendererImportHealth::Degraded,
                xpixmap: LiveRendererImportPathStatus::Disabled,
                dmabuf: LiveRendererImportPathStatus::Degraded,
            },
        }
    );
}

#[test]
fn native_gbm_scanout_exporter_fails_closed_without_backend_device() {
    let missing_device = Err(std::io::Error::from_raw_os_error(19));

    assert_eq!(
        NativeGbmScanoutBufferExporter::export_owned_scanout_buffer_from_backend_device_result::<
            std::fs::File,
        >(
            missing_device,
            sophia_renderer_live::LiveGbmEglFrameTargetRecord::new(sophia_renderer_live::Size {
                width: 1280,
                height: 720,
            }),
        )
        .status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
}

#[test]
fn native_rendered_gbm_scanout_exporter_fails_closed_without_backend_device() {
    let missing_device = Err(std::io::Error::from_raw_os_error(19));

    assert_eq!(
        NativeGbmScanoutBufferExporter::export_rendered_owned_scanout_buffer_from_backend_device_result::<
            std::fs::File,
        >(
            missing_device,
            sophia_renderer_live::LiveGbmEglFrameTargetRecord::new(sophia_renderer_live::Size {
                width: 1280,
                height: 720,
            }),
        )
        .status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
}

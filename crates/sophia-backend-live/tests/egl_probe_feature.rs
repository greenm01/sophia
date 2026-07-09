#![cfg(feature = "egl-probe")]

use sophia_backend_live::{
    EglContextProbeStatus, EglDrawSmokeStatus, EglPlatformStatus, LiveBackendConfig,
    LiveEglStartupReport, LiveEglStartupStatus, discover_live_backend,
};

#[test]
fn egl_probe_projects_ready_platform_and_context_to_native_drawing_status() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

    assert_eq!(
        report.egl_probe_report(
            EglPlatformStatus::NativePlatformCapable,
            EglContextProbeStatus::Available,
        ),
        LiveEglStartupReport {
            status: LiveEglStartupStatus::NativeDrawingCapable,
        }
    );
}

#[test]
fn egl_probe_reports_platform_unavailable_without_native_details() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

    assert_eq!(
        report.egl_probe_report(
            EglPlatformStatus::PlatformUnavailable,
            EglContextProbeStatus::Available,
        ),
        LiveEglStartupReport {
            status: LiveEglStartupStatus::PlatformUnavailable,
        }
    );
}

#[test]
fn egl_probe_reports_context_unavailable_after_platform_is_ready() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

    assert_eq!(
        report.egl_probe_report(
            EglPlatformStatus::NativePlatformCapable,
            EglContextProbeStatus::Unavailable,
        ),
        LiveEglStartupReport {
            status: LiveEglStartupStatus::ContextUnavailable,
        }
    );
}

#[test]
fn native_egl_probe_reports_only_reduced_startup_status() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
    let egl = report.native_egl_probe_report();

    assert!(matches!(
        egl.status,
        LiveEglStartupStatus::NativeDrawingCapable
            | LiveEglStartupStatus::PlatformUnavailable
            | LiveEglStartupStatus::PlatformDegraded
            | LiveEglStartupStatus::ContextUnavailable
    ));
}

#[test]
fn native_egl_draw_smoke_reports_only_reduced_status() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
    let smoke = report.native_egl_draw_smoke_report();

    assert!(matches!(
        smoke.status,
        EglDrawSmokeStatus::ClearColorReady
            | EglDrawSmokeStatus::PlatformUnavailable
            | EglDrawSmokeStatus::PlatformDegraded
            | EglDrawSmokeStatus::ContextUnavailable
            | EglDrawSmokeStatus::SurfaceUnavailable
            | EglDrawSmokeStatus::MakeCurrentUnavailable
            | EglDrawSmokeStatus::GlUnavailable
    ));
}

#[cfg(feature = "gbm-probe")]
mod gbm_projection {
    use super::*;
    use sophia_backend_live::{
        LiveGbmBackedEglPlatformReport, LiveGpuStartupReport, LiveGpuStartupStatus,
    };

    #[test]
    fn gbm_backed_egl_platform_report_uses_native_gbm_as_ready_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.gbm_backed_egl_platform_report(LiveGpuStartupReport {
                status: LiveGpuStartupStatus::NativeCapable,
            }),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::NativePlatformCapable,
            }
        );
    }

    #[test]
    fn gbm_backed_egl_platform_report_keeps_missing_gbm_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.gbm_backed_egl_platform_report(LiveGpuStartupReport {
                status: LiveGpuStartupStatus::RenderDeviceUnavailable,
            }),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::PlatformUnavailable,
            }
        );
    }

    #[test]
    fn gbm_backed_egl_platform_report_maps_degraded_gbm_to_degraded_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.gbm_backed_egl_platform_report(LiveGpuStartupReport {
                status: LiveGpuStartupStatus::PrivateAllocationUnavailable,
            }),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::PlatformDegraded,
            }
        );
    }

    #[test]
    fn native_gbm_backed_egl_platform_report_maps_open_failure_to_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            report.native_gbm_backed_egl_platform_report_from_device_result::<std::fs::File>(
                missing_device,
            ),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::PlatformUnavailable,
            }
        );
    }

    #[test]
    fn native_gbm_backed_egl_platform_report_stays_reduced_for_invalid_device() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let invalid_render_device = std::fs::File::open("/dev/null");
        let platform =
            report.native_gbm_backed_egl_platform_report_from_device_result(invalid_render_device);

        assert!(matches!(
            platform.status,
            EglPlatformStatus::NativePlatformCapable
                | EglPlatformStatus::PlatformUnavailable
                | EglPlatformStatus::PlatformDegraded
        ));
    }

    #[test]
    fn egl_probe_uses_native_gbm_startup_as_ready_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.egl_probe_report_from_gbm_startup(
                LiveGpuStartupReport {
                    status: LiveGpuStartupStatus::NativeCapable,
                },
                EglContextProbeStatus::Available,
            ),
            LiveEglStartupReport {
                status: LiveEglStartupStatus::NativeDrawingCapable,
            }
        );
    }

    #[test]
    fn egl_probe_maps_degraded_gbm_startup_to_degraded_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.egl_probe_report_from_gbm_startup(
                LiveGpuStartupReport {
                    status: LiveGpuStartupStatus::PrivateAllocationUnavailable,
                },
                EglContextProbeStatus::Available,
            ),
            LiveEglStartupReport {
                status: LiveEglStartupStatus::PlatformDegraded,
            }
        );
    }
}

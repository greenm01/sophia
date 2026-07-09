#![cfg(feature = "egl-probe")]

use sophia_backend_live::{
    EglContextProbeStatus, EglPlatformStatus, LiveBackendConfig, LiveEglStartupReport,
    LiveEglStartupStatus, discover_live_backend,
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

#[cfg(feature = "gbm-probe")]
mod gbm_projection {
    use super::*;
    use sophia_backend_live::{LiveGpuStartupReport, LiveGpuStartupStatus};

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

#![cfg(feature = "egl-probe")]

use sophia_renderer_live::{
    EglCapabilityProbeReport, EglCapabilityProbeStatus, EglContextProbeStatus, EglDrawSmokeReport,
    EglDrawSmokeStatus, EglPlatformStatus, FakeEglCapabilityProbe, FakeEglDrawSmoke,
    NativeEglCapabilityProbe, NativeEglDrawSmoke,
};

#[test]
fn fake_egl_probe_reports_native_drawing_capability_from_ready_platform_and_context() {
    assert_eq!(
        FakeEglCapabilityProbe::new(
            EglPlatformStatus::NativePlatformCapable,
            EglContextProbeStatus::Available,
        )
        .probe_report(),
        EglCapabilityProbeReport {
            status: EglCapabilityProbeStatus::NativeDrawingCapable,
        }
    );
}

#[test]
fn fake_egl_probe_reports_platform_unavailable_before_context_status() {
    assert_eq!(
        FakeEglCapabilityProbe::new(
            EglPlatformStatus::PlatformUnavailable,
            EglContextProbeStatus::Available,
        )
        .probe_report(),
        EglCapabilityProbeReport {
            status: EglCapabilityProbeStatus::PlatformUnavailable,
        }
    );
}

#[test]
fn fake_egl_probe_reports_platform_degraded_before_context_status() {
    assert_eq!(
        FakeEglCapabilityProbe::new(
            EglPlatformStatus::PlatformDegraded,
            EglContextProbeStatus::Available,
        )
        .probe_report(),
        EglCapabilityProbeReport {
            status: EglCapabilityProbeStatus::PlatformDegraded,
        }
    );
}

#[test]
fn fake_egl_probe_reports_context_unavailable_only_after_platform_is_ready() {
    assert_eq!(
        FakeEglCapabilityProbe::new(
            EglPlatformStatus::NativePlatformCapable,
            EglContextProbeStatus::Unavailable,
        )
        .probe_report(),
        EglCapabilityProbeReport {
            status: EglCapabilityProbeStatus::ContextUnavailable,
        }
    );
}

#[test]
fn native_egl_probe_stays_reduced_at_public_boundary() {
    let report = NativeEglCapabilityProbe::probe_report();

    assert!(matches!(
        report.status,
        EglCapabilityProbeStatus::NativeDrawingCapable
            | EglCapabilityProbeStatus::PlatformUnavailable
            | EglCapabilityProbeStatus::PlatformDegraded
            | EglCapabilityProbeStatus::ContextUnavailable
    ));
}

#[test]
fn fake_egl_draw_smoke_reports_reduced_clear_color_status() {
    assert_eq!(
        FakeEglDrawSmoke::new(EglDrawSmokeStatus::ClearColorReady).smoke_report(),
        EglDrawSmokeReport {
            status: EglDrawSmokeStatus::ClearColorReady,
        }
    );
}

#[test]
fn fake_egl_draw_smoke_can_report_reduced_surface_failure() {
    assert_eq!(
        FakeEglDrawSmoke::new(EglDrawSmokeStatus::SurfaceUnavailable).smoke_report(),
        EglDrawSmokeReport {
            status: EglDrawSmokeStatus::SurfaceUnavailable,
        }
    );
}

#[test]
fn native_egl_draw_smoke_stays_reduced_at_public_boundary() {
    let report = NativeEglDrawSmoke::smoke_report();

    assert!(matches!(
        report.status,
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
mod gbm_backed_platform {
    use sophia_renderer_live::{EglPlatformStatus, NativeGbmBackedEglPlatformProbe};

    #[test]
    fn native_gbm_backed_platform_maps_open_failure_to_unavailable() {
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            NativeGbmBackedEglPlatformProbe::platform_status_from_backend_device_result::<
                std::fs::File,
            >(missing_device),
            EglPlatformStatus::PlatformUnavailable,
        );
    }

    #[test]
    fn native_gbm_backed_platform_stays_reduced_for_invalid_device() {
        let invalid_render_device = std::fs::File::open("/dev/null");
        let status = NativeGbmBackedEglPlatformProbe::platform_status_from_backend_device_result(
            invalid_render_device,
        );

        assert!(matches!(
            status,
            EglPlatformStatus::NativePlatformCapable
                | EglPlatformStatus::PlatformUnavailable
                | EglPlatformStatus::PlatformDegraded
        ));
    }
}

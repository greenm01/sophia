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
fn fake_egl_draw_smoke_reports_reduced_offscreen_target_status() {
    assert_eq!(
        FakeEglDrawSmoke::new(EglDrawSmokeStatus::OffscreenTargetReady).smoke_report(),
        EglDrawSmokeReport {
            status: EglDrawSmokeStatus::OffscreenTargetReady,
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
        EglDrawSmokeStatus::OffscreenTargetReady
            | EglDrawSmokeStatus::PlatformUnavailable
            | EglDrawSmokeStatus::PlatformDegraded
            | EglDrawSmokeStatus::ContextUnavailable
            | EglDrawSmokeStatus::SurfaceUnavailable
            | EglDrawSmokeStatus::MakeCurrentUnavailable
    ));
}

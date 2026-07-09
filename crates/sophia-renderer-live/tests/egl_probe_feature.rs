#![cfg(feature = "egl-probe")]

use sophia_renderer_live::{
    EglCapabilityProbeReport, EglCapabilityProbeStatus, EglContextProbeStatus, EglPlatformStatus,
    FakeEglCapabilityProbe,
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

#![cfg(feature = "gbm-probe")]

use sophia_renderer_live::{
    FakeGbmCapabilityProbe, LiveRendererImportHealth, LiveRendererImportPathStatus,
    LiveRendererImportStartupStatus,
};

#[test]
fn fake_gbm_probe_reports_native_capability_without_real_gbm_dependency() {
    assert_eq!(
        FakeGbmCapabilityProbe::new(true).startup_status(),
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
        FakeGbmCapabilityProbe::new(false).startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

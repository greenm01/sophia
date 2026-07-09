use sophia_renderer_live::{
    BufferImportPath, BufferSource, FakeLiveRendererCapabilityProbe, LiveRendererImportBoundary,
    LiveRendererImportDecision, LiveRendererImportHealth, LiveRendererImportPathStatus,
    LiveRendererImportRejection, LiveRendererImportStartupStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation,
};

#[test]
fn cpu_upload_is_the_default_accepted_renderer_import_path() {
    let boundary = LiveRendererImportBoundary::cpu_only();

    assert_eq!(
        boundary.decide(BufferSource::CpuBuffer { handle: 77 }),
        LiveRendererImportDecision::Accepted {
            path: BufferImportPath::CpuReadback,
        }
    );
}

#[test]
fn empty_sources_fail_closed_before_renderer_import() {
    let boundary = LiveRendererImportBoundary::cpu_only();

    assert_eq!(
        boundary.decide(BufferSource::None),
        LiveRendererImportDecision::Rejected {
            reason: LiveRendererImportRejection::EmptySource,
        }
    );
}

#[test]
fn native_import_sources_are_deferred_without_a_live_renderer_boundary() {
    let boundary = LiveRendererImportBoundary::cpu_only();

    assert_eq!(
        boundary.decide(BufferSource::XPixmap { pixmap: 44 }),
        LiveRendererImportDecision::Deferred {
            requested: BufferImportPath::XPixmap,
            required_boundary: "live XPixmap renderer import",
        }
    );
    assert_eq!(
        boundary.decide(BufferSource::DmaBuf { handle: 55 }),
        LiveRendererImportDecision::Deferred {
            requested: BufferImportPath::DmaBuf,
            required_boundary: "live DMA-BUF renderer import",
        }
    );
}

#[test]
fn native_import_sources_can_be_admitted_by_an_explicit_renderer_boundary() {
    let boundary = LiveRendererImportBoundary::with_native_imports(true, true);

    assert_eq!(
        boundary.decide(BufferSource::XPixmap { pixmap: 44 }),
        LiveRendererImportDecision::Accepted {
            path: BufferImportPath::XPixmap,
        }
    );
    assert_eq!(
        boundary.decide(BufferSource::DmaBuf { handle: 55 }),
        LiveRendererImportDecision::Accepted {
            path: BufferImportPath::DmaBuf,
        }
    );
}

#[test]
fn runtime_observation_reports_reduced_renderer_selection_without_handles() {
    let status = LiveRendererImportStartupStatus {
        health: LiveRendererImportHealth::NativeImportCapable,
        xpixmap: LiveRendererImportPathStatus::Enabled,
        dmabuf: LiveRendererImportPathStatus::Disabled,
    };

    assert_eq!(
        LiveRendererRuntimeObservation::from_startup_status(
            status,
            LiveRendererSelectionObservation::NativeImportCapable,
        ),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
            selection: LiveRendererSelectionObservation::NativeImportCapable,
        }
    );
}

#[test]
fn fake_capability_probe_can_report_degraded_native_import_without_real_gpu_deps() {
    let probe = FakeLiveRendererCapabilityProbe::new(
        LiveRendererImportPathStatus::Degraded,
        LiveRendererImportPathStatus::Disabled,
    );

    assert_eq!(
        probe.startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Degraded,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
}

#[test]
fn degraded_path_status_takes_precedence_over_partial_native_import() {
    let probe = FakeLiveRendererCapabilityProbe::new(
        LiveRendererImportPathStatus::Enabled,
        LiveRendererImportPathStatus::Degraded,
    );

    assert_eq!(
        probe.startup_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }
    );
}

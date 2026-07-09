use sophia_renderer_live::{
    FakePresentationSmoke, LiveRendererPresentationReport, LiveRendererPresentationStatus,
};

#[test]
fn fake_presentation_smoke_reports_ready_without_handles() {
    assert_eq!(
        FakePresentationSmoke::new(LiveRendererPresentationStatus::Ready).smoke_report(),
        LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }
    );
}

#[test]
fn fake_presentation_smoke_reports_unavailable_without_native_errors() {
    assert_eq!(
        FakePresentationSmoke::new(LiveRendererPresentationStatus::Unavailable).smoke_report(),
        LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Unavailable,
        }
    );
}

#[test]
fn fake_presentation_smoke_reports_degraded_without_partial_scanout() {
    assert_eq!(
        FakePresentationSmoke::new(LiveRendererPresentationStatus::Degraded).smoke_report(),
        LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Degraded,
        }
    );
}

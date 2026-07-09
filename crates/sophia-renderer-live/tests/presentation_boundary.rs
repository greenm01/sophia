use sophia_renderer_live::{
    FakePresentationSmoke, LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus,
    LiveRendererPresentationReport, LiveRendererPresentationStatus, Size,
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

#[test]
fn gbm_egl_frame_target_record_accepts_positive_size_without_handles() {
    assert_eq!(
        LiveGbmEglFrameTargetRecord::new(Size {
            width: 1920,
            height: 1080,
        }),
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 1920,
                height: 1080,
            },
        }
    );
}

#[test]
fn gbm_egl_frame_target_record_rejects_invalid_size_without_native_errors() {
    assert_eq!(
        LiveGbmEglFrameTargetRecord::new(Size {
            width: 0,
            height: 1080,
        }),
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::InvalidSize,
            size: Size {
                width: 0,
                height: 1080,
            },
        }
    );
}

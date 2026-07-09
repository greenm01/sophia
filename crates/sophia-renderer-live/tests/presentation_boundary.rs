use sophia_renderer_live::{
    FakeGbmEglFrameTargetAllocator, FakePresentationSmoke, LiveGbmEglFrameTargetAllocationReport,
    LiveGbmEglFrameTargetAllocationRequest, LiveGbmEglFrameTargetAllocationStatus,
    LiveGbmEglFrameTargetAllocator, LiveGbmEglFrameTargetLifecycleReport,
    LiveGbmEglFrameTargetLifecycleStatus, LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus,
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

#[test]
fn gbm_egl_frame_target_lifecycle_reports_reduced_transitions_without_handles() {
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1920,
        height: 1080,
    });

    assert_eq!(
        LiveGbmEglFrameTargetLifecycleReport::created(target),
        LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target,
        }
    );
    assert_eq!(
        LiveGbmEglFrameTargetLifecycleReport::from_size_update(Some(target), target).status,
        LiveGbmEglFrameTargetLifecycleStatus::Retained
    );
    assert_eq!(
        LiveGbmEglFrameTargetLifecycleReport::from_size_update(
            Some(target),
            LiveGbmEglFrameTargetRecord::new(Size {
                width: 1280,
                height: 720,
            }),
        )
        .status,
        LiveGbmEglFrameTargetLifecycleStatus::Resized
    );
    assert_eq!(
        LiveGbmEglFrameTargetLifecycleReport::from_size_update(
            Some(target),
            LiveGbmEglFrameTargetRecord::new(Size {
                width: 0,
                height: 720,
            }),
        )
        .status,
        LiveGbmEglFrameTargetLifecycleStatus::Invalidated
    );
    assert_eq!(
        LiveGbmEglFrameTargetLifecycleReport::retired(target).status,
        LiveGbmEglFrameTargetLifecycleStatus::Retired
    );
}

#[test]
fn fake_gbm_egl_frame_target_allocator_reports_ready_without_handles() {
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);
    let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
        width: 1920,
        height: 1080,
    });

    assert_eq!(
        allocator.allocate_frame_target(request),
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::Ready,
            target: LiveGbmEglFrameTargetRecord {
                status: LiveGbmEglFrameTargetStatus::Ready,
                size: Size {
                    width: 1920,
                    height: 1080,
                },
            },
        }
    );
}

#[test]
fn fake_gbm_egl_frame_target_allocator_rejects_invalid_target_without_native_errors() {
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);
    let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
        width: 0,
        height: 1080,
    });

    assert_eq!(
        allocator.allocate_frame_target(request),
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::InvalidTarget,
            target: LiveGbmEglFrameTargetRecord {
                status: LiveGbmEglFrameTargetStatus::InvalidSize,
                size: Size {
                    width: 0,
                    height: 1080,
                },
            },
        }
    );
}

#[test]
fn fake_gbm_egl_frame_target_allocator_reports_reduced_failures_without_handles() {
    for status in [
        LiveGbmEglFrameTargetAllocationStatus::Unavailable,
        LiveGbmEglFrameTargetAllocationStatus::Degraded,
    ] {
        let mut allocator = FakeGbmEglFrameTargetAllocator::new(status);
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 1280,
            height: 720,
        });

        assert_eq!(
            allocator.allocate_frame_target(request),
            LiveGbmEglFrameTargetAllocationReport {
                status,
                target: LiveGbmEglFrameTargetRecord {
                    status: LiveGbmEglFrameTargetStatus::Ready,
                    size: Size {
                        width: 1280,
                        height: 720,
                    },
                },
            }
        );
    }
}

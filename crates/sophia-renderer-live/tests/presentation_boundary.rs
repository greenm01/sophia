use sophia_renderer_live::{
    FakeGbmEglFrameTargetAllocator, FakePresentationSmoke, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
    LiveGbmEglFrameTargetAllocationReport, LiveGbmEglFrameTargetAllocationRequest,
    LiveGbmEglFrameTargetAllocationStatus, LiveGbmEglFrameTargetAllocator,
    LiveGbmEglFrameTargetLifecycleReport, LiveGbmEglFrameTargetLifecycleStatus,
    LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus, LiveRendererPresentationReport,
    LiveRendererPresentationStatus, LiveRendererScanoutBufferDescriptor,
    LiveRendererScanoutBufferExportReport, LiveRendererScanoutBufferExportStatus,
    LiveRendererScanoutBufferExporter, LiveRendererScanoutBufferStatus, Size,
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
fn gbm_egl_frame_target_record_requires_ready_status_and_positive_size() {
    let ready = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1920,
        height: 1080,
    });
    assert!(ready.is_valid_scanout_target());

    assert!(
        !LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 0,
                height: 1080,
            },
        }
        .is_valid_scanout_target()
    );
    assert!(
        !LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::InvalidSize,
            size: Size {
                width: 1920,
                height: 1080,
            },
        }
        .is_valid_scanout_target()
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
fn renderer_scanout_buffer_descriptor_validates_scanout_shape() {
    let ready = LiveRendererScanoutBufferDescriptor::new(
        Size {
            width: 1920,
            height: 1080,
        },
        1920 * 4,
        LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        44,
    );
    assert_eq!(ready.status, LiveRendererScanoutBufferStatus::Ready);

    for descriptor in [
        LiveRendererScanoutBufferDescriptor::new(
            Size {
                width: 0,
                height: 1080,
            },
            1920 * 4,
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            44,
        ),
        LiveRendererScanoutBufferDescriptor::new(
            Size {
                width: 1920,
                height: 1080,
            },
            0,
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            44,
        ),
        LiveRendererScanoutBufferDescriptor::new(
            Size {
                width: 1920,
                height: 1080,
            },
            1920 * 4,
            0,
            44,
        ),
        LiveRendererScanoutBufferDescriptor::new(
            Size {
                width: 1920,
                height: 1080,
            },
            1920 * 4,
            LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            0,
        ),
    ] {
        assert_eq!(descriptor.status, LiveRendererScanoutBufferStatus::Invalid);
    }
}

#[test]
fn fake_renderer_scanout_exporter_reports_reduced_status_without_native_handles() {
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1280,
        height: 720,
    });
    let mut ready = sophia_renderer_live::FakeRendererScanoutBufferExporter::new(
        LiveRendererScanoutBufferExportStatus::Exported,
    )
    .with_descriptor(1280 * 4, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, 55);
    let report = ready.export_scanout_buffer(target);
    assert_eq!(
        report,
        LiveRendererScanoutBufferExportReport {
            status: LiveRendererScanoutBufferExportStatus::Exported,
            descriptor: Some(LiveRendererScanoutBufferDescriptor::new(
                target.size,
                1280 * 4,
                LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                55,
            )),
        }
    );

    let mut unavailable = sophia_renderer_live::FakeRendererScanoutBufferExporter::new(
        LiveRendererScanoutBufferExportStatus::Unavailable,
    );
    assert_eq!(
        unavailable.export_scanout_buffer(target).status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );

    let mut invalid_target = sophia_renderer_live::FakeRendererScanoutBufferExporter::new(
        LiveRendererScanoutBufferExportStatus::Exported,
    )
    .with_descriptor(1280 * 4, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, 55);
    assert_eq!(
        invalid_target
            .export_scanout_buffer(LiveGbmEglFrameTargetRecord::new(Size {
                width: 0,
                height: 720,
            }))
            .status,
        LiveRendererScanoutBufferExportStatus::InvalidTarget
    );

    let malformed_ready_target = LiveGbmEglFrameTargetRecord {
        status: LiveGbmEglFrameTargetStatus::Ready,
        size: Size {
            width: -1,
            height: 720,
        },
    };
    assert_eq!(
        invalid_target
            .export_scanout_buffer(malformed_ready_target)
            .status,
        LiveRendererScanoutBufferExportStatus::InvalidTarget
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

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
    use sophia_renderer_live::{
        EglDrawSmokeStatus, EglPlatformStatus, LiveGbmEglFrameTargetAllocationRequest,
        LiveGbmEglFrameTargetAllocationStatus, LiveGbmEglFrameTargetStatus,
        LiveRendererPresentationStatus, NativeGbmBackedEglDrawSmoke,
        NativeGbmBackedEglFrameTargetAllocator, NativeGbmBackedEglPlatformProbe,
        NativeGbmBackedEglPresentationSmoke, Size,
    };

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

    #[test]
    fn native_gbm_backed_draw_smoke_maps_open_failure_to_platform_unavailable() {
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            NativeGbmBackedEglDrawSmoke::smoke_report_from_backend_device_result::<std::fs::File>(
                missing_device,
            )
            .status,
            EglDrawSmokeStatus::PlatformUnavailable,
        );
    }

    #[test]
    fn native_gbm_backed_draw_smoke_stays_reduced_for_invalid_device() {
        let invalid_render_device = std::fs::File::open("/dev/null");
        let smoke = NativeGbmBackedEglDrawSmoke::smoke_report_from_backend_device_result(
            invalid_render_device,
        );

        assert!(matches!(
            smoke.status,
            EglDrawSmokeStatus::ClearColorReady
                | EglDrawSmokeStatus::PlatformUnavailable
                | EglDrawSmokeStatus::PlatformDegraded
                | EglDrawSmokeStatus::ContextUnavailable
                | EglDrawSmokeStatus::SurfaceUnavailable
                | EglDrawSmokeStatus::MakeCurrentUnavailable
                | EglDrawSmokeStatus::GlUnavailable
        ));
    }

    #[test]
    fn native_gbm_backed_presentation_smoke_maps_open_failure_to_unavailable() {
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            NativeGbmBackedEglPresentationSmoke::smoke_report_from_backend_device_result::<
                std::fs::File,
            >(missing_device)
            .status,
            LiveRendererPresentationStatus::Unavailable,
        );
    }

    #[test]
    fn native_gbm_backed_presentation_smoke_stays_reduced_for_invalid_device() {
        let invalid_render_device = std::fs::File::open("/dev/null");
        let smoke = NativeGbmBackedEglPresentationSmoke::smoke_report_from_backend_device_result(
            invalid_render_device,
        );

        assert!(matches!(
            smoke.status,
            LiveRendererPresentationStatus::Ready
                | LiveRendererPresentationStatus::Unavailable
                | LiveRendererPresentationStatus::Degraded
        ));
    }

    #[test]
    fn native_gbm_backed_frame_target_allocator_maps_open_failure_to_unavailable() {
        let missing_device = Err(std::io::Error::from_raw_os_error(19));
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 1920,
            height: 1080,
        });

        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result::<
                std::fs::File,
            >(missing_device, request);

        assert_eq!(
            report.status,
            LiveGbmEglFrameTargetAllocationStatus::Unavailable
        );
        assert_eq!(report.target, request.target);
    }

    #[test]
    fn native_gbm_backed_frame_target_allocator_rejects_invalid_target_before_native_work() {
        let missing_device = Err(std::io::Error::from_raw_os_error(19));
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 0,
            height: 1080,
        });

        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result::<
                std::fs::File,
            >(missing_device, request);

        assert_eq!(
            report.status,
            LiveGbmEglFrameTargetAllocationStatus::InvalidTarget
        );
        assert_eq!(
            report.target.status,
            LiveGbmEglFrameTargetStatus::InvalidSize
        );
    }

    #[test]
    fn native_gbm_backed_frame_target_allocator_stays_reduced_for_invalid_device() {
        let invalid_render_device = std::fs::File::open("/dev/null");
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 1280,
            height: 720,
        });

        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
                invalid_render_device,
                request,
            );

        assert!(matches!(
            report.status,
            LiveGbmEglFrameTargetAllocationStatus::Ready
                | LiveGbmEglFrameTargetAllocationStatus::Unavailable
                | LiveGbmEglFrameTargetAllocationStatus::Degraded
        ));
        assert_eq!(report.target, request.target);
    }
}

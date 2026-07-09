#![cfg(feature = "egl-probe")]

use sophia_backend_live::{
    EglContextProbeStatus, EglDrawSmokeStatus, EglPlatformStatus, LiveBackendConfig,
    LiveEglStartupReport, LiveEglStartupStatus, discover_live_backend,
};

#[test]
fn egl_probe_projects_ready_platform_and_context_to_native_drawing_status() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

    assert_eq!(
        report.egl_probe_report(
            EglPlatformStatus::NativePlatformCapable,
            EglContextProbeStatus::Available,
        ),
        LiveEglStartupReport {
            status: LiveEglStartupStatus::NativeDrawingCapable,
        }
    );
}

#[test]
fn egl_probe_reports_platform_unavailable_without_native_details() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

    assert_eq!(
        report.egl_probe_report(
            EglPlatformStatus::PlatformUnavailable,
            EglContextProbeStatus::Available,
        ),
        LiveEglStartupReport {
            status: LiveEglStartupStatus::PlatformUnavailable,
        }
    );
}

#[test]
fn egl_probe_reports_context_unavailable_after_platform_is_ready() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

    assert_eq!(
        report.egl_probe_report(
            EglPlatformStatus::NativePlatformCapable,
            EglContextProbeStatus::Unavailable,
        ),
        LiveEglStartupReport {
            status: LiveEglStartupStatus::ContextUnavailable,
        }
    );
}

#[test]
fn native_egl_probe_reports_only_reduced_startup_status() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
    let egl = report.native_egl_probe_report();

    assert!(matches!(
        egl.status,
        LiveEglStartupStatus::NativeDrawingCapable
            | LiveEglStartupStatus::PlatformUnavailable
            | LiveEglStartupStatus::PlatformDegraded
            | LiveEglStartupStatus::ContextUnavailable
    ));
}

#[test]
fn native_egl_draw_smoke_reports_only_reduced_status() {
    let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
    let smoke = report.native_egl_draw_smoke_report();

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

#[cfg(feature = "gbm-probe")]
mod gbm_projection {
    use super::*;
    use std::path::PathBuf;

    use sophia_backend_live::{
        EglDrawSmokeReport, LiveGbmBackedEglPlatformReport, LiveGbmEglFrameTargetAllocationRequest,
        LiveGbmEglFrameTargetAllocationStatus, LiveGbmEglFrameTargetStatus, LiveGpuStartupReport,
        LiveGpuStartupStatus, LiveRealGbmSmokeEvidence, LiveRealGbmSmokeEvidenceStatus,
        LiveRendererPresentationReport, LiveRendererPresentationStatus,
        RenderDeviceDiscoveryBackend, Size,
    };

    struct ExplicitRenderDevice {
        path: PathBuf,
    }

    impl RenderDeviceDiscoveryBackend for ExplicitRenderDevice {
        type Device = std::fs::File;

        fn open_render_device(&self) -> std::io::Result<Self::Device> {
            std::fs::File::open(&self.path)
        }
    }

    #[test]
    fn gbm_backed_egl_platform_report_uses_native_gbm_as_ready_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.gbm_backed_egl_platform_report(LiveGpuStartupReport {
                status: LiveGpuStartupStatus::NativeCapable,
            }),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::NativePlatformCapable,
            }
        );
    }

    #[test]
    fn gbm_backed_egl_platform_report_keeps_missing_gbm_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.gbm_backed_egl_platform_report(LiveGpuStartupReport {
                status: LiveGpuStartupStatus::RenderDeviceUnavailable,
            }),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::PlatformUnavailable,
            }
        );
    }

    #[test]
    fn gbm_backed_egl_platform_report_maps_degraded_gbm_to_degraded_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.gbm_backed_egl_platform_report(LiveGpuStartupReport {
                status: LiveGpuStartupStatus::PrivateAllocationUnavailable,
            }),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::PlatformDegraded,
            }
        );
    }

    #[test]
    fn native_gbm_backed_egl_platform_report_maps_open_failure_to_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            report.native_gbm_backed_egl_platform_report_from_device_result::<std::fs::File>(
                missing_device,
            ),
            LiveGbmBackedEglPlatformReport {
                status: EglPlatformStatus::PlatformUnavailable,
            }
        );
    }

    #[test]
    fn native_gbm_backed_egl_platform_report_stays_reduced_for_invalid_device() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let invalid_render_device = std::fs::File::open("/dev/null");
        let platform =
            report.native_gbm_backed_egl_platform_report_from_device_result(invalid_render_device);

        assert!(matches!(
            platform.status,
            EglPlatformStatus::NativePlatformCapable
                | EglPlatformStatus::PlatformUnavailable
                | EglPlatformStatus::PlatformDegraded
        ));
    }

    #[test]
    fn native_gbm_backed_egl_draw_smoke_maps_open_failure_to_platform_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            report
                .native_gbm_backed_egl_draw_smoke_report_from_device_result::<std::fs::File>(
                    missing_device,
                )
                .status,
            EglDrawSmokeStatus::PlatformUnavailable,
        );
    }

    #[test]
    fn native_gbm_backed_egl_draw_smoke_stays_reduced_for_invalid_device() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let invalid_render_device = std::fs::File::open("/dev/null");
        let smoke = report
            .native_gbm_backed_egl_draw_smoke_report_from_device_result(invalid_render_device);

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
    fn native_gbm_backed_egl_presentation_smoke_maps_open_failure_to_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let missing_device = Err(std::io::Error::from_raw_os_error(19));

        assert_eq!(
            report
                .native_gbm_backed_egl_presentation_smoke_report_from_device_result::<
                    std::fs::File,
                >(missing_device)
                .status,
            LiveRendererPresentationStatus::Unavailable,
        );
    }

    #[test]
    fn native_gbm_backed_egl_presentation_smoke_stays_reduced_for_invalid_device() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let invalid_render_device = std::fs::File::open("/dev/null");
        let smoke = report.native_gbm_backed_egl_presentation_smoke_report_from_device_result(
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
    fn native_gbm_backed_egl_frame_target_allocation_maps_open_failure_to_unavailable() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let missing_device = Err(std::io::Error::from_raw_os_error(19));
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 1920,
            height: 1080,
        });

        let allocation =
            report.native_gbm_backed_egl_frame_target_allocation_report_from_device_result::<
                std::fs::File,
            >(missing_device, request);

        assert_eq!(
            allocation.status,
            LiveGbmEglFrameTargetAllocationStatus::Unavailable
        );
        assert_eq!(allocation.target, request.target);
    }

    #[test]
    fn native_gbm_backed_egl_frame_target_allocation_rejects_invalid_target() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let missing_device = Err(std::io::Error::from_raw_os_error(19));
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 0,
            height: 1080,
        });

        let allocation =
            report.native_gbm_backed_egl_frame_target_allocation_report_from_device_result::<
                std::fs::File,
            >(missing_device, request);

        assert_eq!(
            allocation.status,
            LiveGbmEglFrameTargetAllocationStatus::InvalidTarget
        );
        assert_eq!(
            allocation.target.status,
            LiveGbmEglFrameTargetStatus::InvalidSize
        );
    }

    #[test]
    fn native_gbm_backed_egl_frame_target_allocation_stays_reduced_for_invalid_device() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));
        let invalid_render_device = std::fs::File::open("/dev/null");
        let request = LiveGbmEglFrameTargetAllocationRequest::new(Size {
            width: 1280,
            height: 720,
        });

        let allocation = report
            .native_gbm_backed_egl_frame_target_allocation_report_from_device_result(
                invalid_render_device,
                request,
            );

        assert!(matches!(
            allocation.status,
            LiveGbmEglFrameTargetAllocationStatus::Ready
                | LiveGbmEglFrameTargetAllocationStatus::Unavailable
                | LiveGbmEglFrameTargetAllocationStatus::Degraded
        ));
        assert_eq!(allocation.target, request.target);
    }

    #[test]
    fn real_gbm_smoke_evidence_records_only_reduced_statuses() {
        assert_eq!(
            LiveRealGbmSmokeEvidence::from_reports(
                EglDrawSmokeReport {
                    status: EglDrawSmokeStatus::ClearColorReady,
                },
                LiveRendererPresentationReport {
                    status: LiveRendererPresentationStatus::Ready,
                },
            ),
            LiveRealGbmSmokeEvidence {
                status: LiveRealGbmSmokeEvidenceStatus::Passed,
                draw: EglDrawSmokeStatus::ClearColorReady,
                presentation: LiveRendererPresentationStatus::Ready,
            }
        );

        assert_eq!(
            LiveRealGbmSmokeEvidence::from_reports(
                EglDrawSmokeReport {
                    status: EglDrawSmokeStatus::SurfaceUnavailable,
                },
                LiveRendererPresentationReport {
                    status: LiveRendererPresentationStatus::Unavailable,
                },
            ),
            LiveRealGbmSmokeEvidence {
                status: LiveRealGbmSmokeEvidenceStatus::Failed,
                draw: EglDrawSmokeStatus::SurfaceUnavailable,
                presentation: LiveRendererPresentationStatus::Unavailable,
            }
        );
    }

    #[test]
    fn native_gbm_backed_egl_smokes_real_render_device_when_enabled() {
        if std::env::var_os("SOPHIA_RUN_REAL_GBM_SMOKE").is_none() {
            return;
        }

        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("gbm_projection::native_gbm_backed_egl_real_render_device_child")
            .arg("--nocapture")
            .env("SOPHIA_REAL_GBM_EGL_CHILD", "1")
            .status()
            .expect("real GBM/EGL smoke child should start");

        assert!(
            status.success(),
            "real GBM/EGL smoke child failed with status {status}"
        );
    }

    #[test]
    fn native_gbm_backed_egl_real_render_device_child() {
        if std::env::var_os("SOPHIA_REAL_GBM_EGL_CHILD").is_none() {
            return;
        }

        let Some(render_device_path) = first_openable_render_node() else {
            return;
        };
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        let draw =
            report.native_gbm_backed_egl_draw_smoke_report_with_gbm_device(&ExplicitRenderDevice {
                path: render_device_path.clone(),
            });
        let presentation = report.native_gbm_backed_egl_presentation_smoke_report_with_gbm_device(
            &ExplicitRenderDevice {
                path: render_device_path,
            },
        );

        assert_eq!(draw.status, EglDrawSmokeStatus::ClearColorReady);
        assert_eq!(presentation.status, LiveRendererPresentationStatus::Ready);
    }

    #[test]
    fn egl_probe_uses_native_gbm_startup_as_ready_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.egl_probe_report_from_gbm_startup(
                LiveGpuStartupReport {
                    status: LiveGpuStartupStatus::NativeCapable,
                },
                EglContextProbeStatus::Available,
            ),
            LiveEglStartupReport {
                status: LiveEglStartupStatus::NativeDrawingCapable,
            }
        );
    }

    #[test]
    fn egl_probe_maps_degraded_gbm_startup_to_degraded_platform() {
        let report = discover_live_backend(&LiveBackendConfig::new("/does/not/matter"));

        assert_eq!(
            report.egl_probe_report_from_gbm_startup(
                LiveGpuStartupReport {
                    status: LiveGpuStartupStatus::PrivateAllocationUnavailable,
                },
                EglContextProbeStatus::Available,
            ),
            LiveEglStartupReport {
                status: LiveEglStartupStatus::PlatformDegraded,
            }
        );
    }

    fn first_openable_render_node() -> Option<PathBuf> {
        let entries = std::fs::read_dir("/dev/dri").ok()?;
        let mut candidates = Vec::new();

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.starts_with("renderD") {
                candidates.push(entry.path());
            }
        }

        candidates.sort();
        candidates
            .into_iter()
            .find(|path| std::fs::File::open(path).is_ok())
    }
}

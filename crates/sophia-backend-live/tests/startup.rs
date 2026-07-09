use std::fs;
use std::path::{Path, PathBuf};

use sophia_backend_live::{
    CompositorBackendTickInput, DeviceId, HeadlessOutput, LibinputDeviceDescriptor,
    LibinputDeviceKind, LiveBackendConfig, LiveBackendDependencyDecision,
    LiveBackendDependencyKind, LiveBackendDependencyUse, LiveCompositorBackendDiscoveryStatus,
    LivePageFlipEvent, LivePageFlipEventStatus, LiveRendererImportBoundary,
    LiveRendererImportHealth, LiveRendererImportPathStatus, LiveRendererImportStartupStatus,
    LiveRendererPreference, LiveRendererPresentationReport, LiveRendererPresentationStatus,
    LiveRendererRuntimeObservation, LiveRendererSelectionObservation, LiveScanoutReadinessReport,
    LiveScanoutReadinessStatus, OutputId, PageFlipCommitOutcome, QueuedInputPoller,
    RendererSelection, SeatId, Size, discover_live_backend, live_backend_dependency_decision,
};
use sophia_protocol::{TransactionCommit, TransactionId, TransactionOutcome};

#[test]
fn live_backend_startup_can_seed_headless_assembly_from_sysfs_and_static_input() {
    let root = drm_sysfs_fixture("ready");
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });

    let report = discover_live_backend(&config);

    assert_eq!(
        report.status(),
        &LiveCompositorBackendDiscoveryStatus::Ready
    );
    assert_eq!(
        report.selected_output(),
        Some(HeadlessOutput {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1920,
                height: 1080,
            },
            scale: 1,
        })
    );
    let assembly = report
        .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
        .expect("ready startup should seed assembly");
    assert_eq!(assembly.input().source().devices().count(), 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_startup_fails_closed_without_connected_outputs() {
    let root = drm_sysfs_fixture("no-output");
    let config = LiveBackendConfig::new(&root);

    let report = discover_live_backend(&config);

    assert_eq!(
        report.status(),
        &LiveCompositorBackendDiscoveryStatus::NoOutputs
    );
    assert_eq!(report.selected_output(), None);
    assert!(
        report
            .into_headless_assembly(QueuedInputPoller::default(), RendererSelection::CpuFallback)
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_startup_uses_cpu_renderer_until_native_import_is_configured() {
    let root = ready_drm_sysfs_fixture("renderer-default");
    let config = LiveBackendConfig::new(&root);

    let report = discover_live_backend(&config);

    assert_eq!(report.renderer_selection(), RendererSelection::CpuFallback);
    assert_eq!(
        report.renderer_import_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
    let assembly = report
        .into_configured_headless_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed assembly");
    assert_eq!(assembly.renderer(), RendererSelection::CpuFallback);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_backend_defaults_to_gpu_preferred_policy() {
    let config = LiveBackendConfig::new("/does/not/matter");

    assert_eq!(
        config.renderer_preference,
        LiveRendererPreference::GpuPreferred
    );
}

#[test]
fn renderer_preference_uses_cpu_only_without_native_status() {
    let root = ready_drm_sysfs_fixture("renderer-cpu-only");
    let config = LiveBackendConfig::new(&root)
        .with_renderer_import_boundary(LiveRendererImportBoundary::with_native_imports(true, true))
        .with_renderer_preference(LiveRendererPreference::CpuOnly);

    let report = discover_live_backend(&config);
    let assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed CPU-only live assembly");

    assert_eq!(
        assembly.assembly().renderer(),
        RendererSelection::CpuFallback
    );
    assert_eq!(
        assembly.renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::CpuFallback,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
            selection: LiveRendererSelectionObservation::CpuFallback,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn renderer_preference_requires_gpu_when_requested() {
    let root = ready_drm_sysfs_fixture("renderer-gpu-required");
    let config =
        LiveBackendConfig::new(&root).with_renderer_preference(LiveRendererPreference::GpuRequired);

    let report = discover_live_backend(&config);

    assert!(
        report
            .into_live_runtime_assembly(QueuedInputPoller::default())
            .is_none()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn renderer_preference_selects_import_renderer_only_for_native_capable_status() {
    let config = LiveBackendConfig::new("/does/not/matter");
    let report = discover_live_backend(&config);

    assert_eq!(
        report.renderer_selection_for_status(LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Enabled,
        }),
        Some(RendererSelection::ImportCapable {
            import_xpixmap: false,
            import_dmabuf: true,
        })
    );
    assert_eq!(
        report.renderer_selection_for_status(LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::Degraded,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Degraded,
        }),
        Some(RendererSelection::CpuFallback)
    );
}

#[test]
fn live_backend_startup_admits_native_renderer_import_only_when_configured() {
    let root = ready_drm_sysfs_fixture("renderer-native");
    let config = LiveBackendConfig::new(&root).with_renderer_import_boundary(
        LiveRendererImportBoundary::with_native_imports(true, false),
    );

    let report = discover_live_backend(&config);

    assert_eq!(
        report.renderer_selection(),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        }
    );
    assert_eq!(
        report.renderer_import_status(),
        LiveRendererImportStartupStatus {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
        }
    );
    let assembly = report
        .into_configured_headless_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed assembly");
    assert_eq!(
        assembly.renderer(),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_reduced_renderer_health_on_tick() {
    let root = ready_drm_sysfs_fixture("runtime-renderer-health");
    let config = LiveBackendConfig::new(&root).with_renderer_import_boundary(
        LiveRendererImportBoundary::with_native_imports(true, false),
    );

    let report = discover_live_backend(&config);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assert_eq!(
        assembly.renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Enabled,
            dmabuf: LiveRendererImportPathStatus::Disabled,
            selection: LiveRendererSelectionObservation::NativeImportCapable,
        }
    );
    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should succeed");
    assert_eq!(tick.renderer, assembly.renderer_observation());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dependency_policy_allows_libdrm_and_libinput_at_live_backend_seams() {
    assert!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::LibDrm,
            LiveBackendDependencyUse::Discovery,
        )
        .is_allowed()
    );
    assert!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::LibInput,
            LiveBackendDependencyUse::RuntimePolling,
        )
        .is_allowed()
    );
}

#[test]
fn dependency_policy_defers_gpu_and_shared_memory_imports() {
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Gbm,
            LiveBackendDependencyUse::RendererImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "live renderer import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::DmaBuf,
            LiveBackendDependencyUse::Discovery,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "live renderer import boundary",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::MitShm,
            LiveBackendDependencyUse::SharedMemoryImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "bounded shared-memory import boundary",
        }
    );
}

#[test]
fn scanout_readiness_reports_ready_without_exposing_kms_identity() {
    let root = ready_drm_sysfs_fixture("scanout-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Ready,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanout_readiness_reports_missing_output_before_renderer_status() {
    let root = drm_sysfs_fixture("scanout-no-output");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::OutputUnavailable,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanout_readiness_collapses_unavailable_presentation_without_native_details() {
    let root = ready_drm_sysfs_fixture("scanout-presentation-unavailable");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Unavailable,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::PresentationUnavailable,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scanout_readiness_collapses_degraded_presentation_without_native_details() {
    let root = ready_drm_sysfs_fixture("scanout-degraded");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Degraded,
        }),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Degraded,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn page_flip_event_projects_scanout_readiness_without_kms_identity() {
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::Ready),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::OutputUnavailable),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::OutputUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::PresentationUnavailable,),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::PresentationUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_scanout_status(LiveScanoutReadinessStatus::Degraded),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Degraded,
            frame_serial: None,
        }
    );
}

#[test]
fn page_flip_event_drops_output_transaction_and_surface_identity() {
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(&PageFlipCommitOutcome::WaitingForOutput {
            expected: OutputId::from_raw(4),
            actual: OutputId::from_raw(9),
            transaction: TransactionId::from_raw(55),
        }),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::WaitingForOutput,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(
            &PageFlipCommitOutcome::WaitingForTransactionReadiness {
                transaction: TransactionId::from_raw(56),
                pending_surfaces: vec![sophia_protocol::SurfaceId::new(77, 1)],
            },
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::WaitingForTransactionReadiness,
            frame_serial: None,
        }
    );
}

#[test]
fn page_flip_event_preserves_only_frame_serial_for_terminal_outcomes() {
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(&PageFlipCommitOutcome::Committed {
            frame_serial: 91,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(57),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
            },
        }),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(91),
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_commit_outcome(&PageFlipCommitOutcome::Rejected {
            frame_serial: 92,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(58),
                outcome: TransactionOutcome::RejectedInvalidSurface,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(89, 1)],
            },
        }),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(92),
        }
    );
}

fn drm_sysfs_fixture(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("sophia-backend-live-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn ready_drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = drm_sysfs_fixture(name);
    let connector = root.join("card0-HDMI-A-1");
    fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1280x720\n");
    root
}

fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

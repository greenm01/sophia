use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use sophia_backend_live::{
    BufferSource, CompositorBackendTickInput, DeviceId, FakeAtomicScanoutCommitter,
    FakeGbmEglFrameTargetAllocator, FakePageFlipCallbackSource, HeadlessOutput,
    LibinputDeviceDescriptor, LibinputDeviceKind, LiveAtomicScanoutCommitReport,
    LiveAtomicScanoutCommitStatus, LiveAtomicScanoutCommitter, LiveBackendConfig,
    LiveBackendDependencyDecision, LiveBackendDependencyKind, LiveBackendDependencyUse,
    LiveCompositorBackendDiscoveryStatus, LiveGbmEglFrameTargetAllocationReport,
    LiveGbmEglFrameTargetAllocationStatus, LiveGbmEglFrameTargetLifecycleReport,
    LiveGbmEglFrameTargetLifecycleStatus, LiveGbmEglFrameTargetRecord, LiveGbmEglFrameTargetStatus,
    LiveKmsScanoutTargetReport, LiveKmsScanoutTargetStatus, LiveLibdrmPollerDiagnostics,
    LiveLibdrmPollerDiagnosticsStatus, LivePageFlipCallback, LivePageFlipCallbackDecision,
    LivePageFlipCallbackIntake, LivePageFlipCallbackQueue, LivePageFlipCallbackQueueReport,
    LivePageFlipCallbackReport, LivePageFlipCallbackSourceReport, LivePageFlipEvent,
    LivePageFlipEventStatus, LiveRenderedOutputState, LiveRenderedOutputTable,
    LiveRenderedOutputTableUpdate, LiveRendererImportBoundary, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportStartupStatus, LiveRendererPreference,
    LiveRendererPresentationReport, LiveRendererPresentationStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation, LiveScanoutReadinessReport, LiveScanoutReadinessStatus,
    OutputId, PageFlipCommitOutcome, PhysicalInputRoutingStage, QueuedInputPoller,
    RendererSelection, SeatId, Size, discover_live_backend, live_backend_dependency_decision,
};

#[test]
fn rendered_output_table_is_bounded_and_keeps_output_state_independent() {
    let mut table = LiveRenderedOutputTable::new();
    for raw in 1..=16 {
        assert_eq!(
            table.insert(LiveRenderedOutputState::ready(HeadlessOutput {
                id: OutputId::from_raw(raw),
                size: Size {
                    width: 640 + i32::try_from(raw).unwrap(),
                    height: 480,
                },
                scale: 1,
            })),
            LiveRenderedOutputTableUpdate::Inserted
        );
    }
    assert_eq!(table.len(), 16);
    assert_eq!(
        table.insert(LiveRenderedOutputState::ready(HeadlessOutput {
            id: OutputId::from_raw(17),
            size: Size {
                width: 800,
                height: 600,
            },
            scale: 1,
        })),
        LiveRenderedOutputTableUpdate::CapacityExceeded
    );
    assert_eq!(
        table.get(OutputId::from_raw(1)).unwrap().output_size(),
        Some(Size {
            width: 641,
            height: 480,
        })
    );
    assert_eq!(
        table.get(OutputId::from_raw(16)).unwrap().output_size(),
        Some(Size {
            width: 656,
            height: 480,
        })
    );
}
use sophia_engine::AuthorityTransactionIntake;
use sophia_protocol::{
    AuthorityKind, InputEventKind, InputEventPacket, LayerSnapshot, NamespaceId, Point, Rect,
    Region, ResizeSyncCapability, SurfaceId, SurfaceTransactionReadiness, Transform,
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
    assert_eq!(
        report.selected_gbm_egl_frame_target(),
        Some(LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 1920,
                height: 1080,
            },
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
    assert_eq!(report.selected_gbm_egl_frame_target(), None);
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
    assert_eq!(
        tick.scanout,
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Ready,
        }
    );
    assert_eq!(
        tick.kms_scanout_target,
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_observation(),
        Some(LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 1280,
                height: 720,
            },
        })
    );
    assert_eq!(
        tick.gbm_egl_frame_target,
        assembly.gbm_egl_frame_target_observation()
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        assembly.libdrm_poller_diagnostics(),
        LiveLibdrmPollerDiagnostics::not_configured()
    );
    assert_eq!(
        tick.libdrm_poller,
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::NotConfigured,
            route_count: 0,
            pending_callbacks: 0,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_threads_scanout_and_page_flip_observations() {
    let root = ready_drm_sysfs_fixture("runtime-scanout-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assert_eq!(
        assembly.scanout_readiness_observation(),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::Ready,
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );

    assembly.observe_presentation_report(LiveRendererPresentationReport {
        status: LiveRendererPresentationStatus::Unavailable,
    });
    assert_eq!(
        assembly.scanout_readiness_observation(),
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::PresentationUnavailable,
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::PresentationUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::PresentationUnavailable,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );

    let mut committer = FakeAtomicScanoutCommitter::default();
    let atomic_commit = assembly.commit_atomic_scanout_with(
        &mut committer,
        &PageFlipCommitOutcome::Committed {
            frame_serial: 121,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(71),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(101, 1)],
            },
        },
    );
    assert_eq!(
        atomic_commit,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Committed,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(121),
            },
        }
    );
    assert_eq!(committer.committed_count(), 1);
    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should succeed");

    assert_eq!(
        tick.scanout,
        LiveScanoutReadinessReport {
            status: LiveScanoutReadinessStatus::PresentationUnavailable,
        }
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(121),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_runs_fake_compositor_loop_without_native_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-fake-compositor-loop");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::new(vec![motion_event(1, 10.0, 20.0)]))
        .expect("ready startup should seed live assembly");
    let mut layer = test_layer(42);
    let transaction_id = TransactionId::from_raw(420);
    let transaction = layer.to_surface_transaction(
        transaction_id,
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );

    layer.geometry.x = 25;
    layer.geometry.y = 30;
    let tick = assembly
        .run_tick(CompositorBackendTickInput {
            x_event_count: 1,
            authority_commits: Vec::new(),
            authority_batches: vec![AuthorityTransactionIntake::new(
                transaction_id,
                vec![transaction],
            )],
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 1,
            layer_templates: vec![layer],
            scanout_submit_state: None,
            scanout_lifecycle_states: Vec::new(),
        })
        .expect("fake compositor runtime tick should complete");

    assert_eq!(tick.engine.input_poll.polled, 1);
    assert_eq!(tick.engine.input_poll.accepted, 1);
    assert!(tick.engine.input_poll.rejected.is_empty());
    assert_eq!(tick.engine.physical_input.poll, tick.engine.input_poll);
    assert_eq!(tick.engine.physical_input.pending_events, 1);
    assert_eq!(
        tick.engine.physical_input.routing_stage,
        PhysicalInputRoutingStage::PhysicalIntakeOnly
    );
    assert_eq!(tick.engine.runtime.runtime_state.x_events_polled, 1);
    assert_eq!(
        tick.engine
            .runtime
            .runtime_state
            .authority_transactions_committed,
        1
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.authority_surfaces_applied,
        1
    );
    assert_eq!(tick.engine.runtime.runtime_state.frames_rendered, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.chrome_commands_presented,
        1
    );
    assert!(tick.engine.render.is_some());
    assert_eq!(
        tick.gbm_egl_frame_target_lifecycle
            .expect("startup target lifecycle should be observed")
            .status,
        LiveGbmEglFrameTargetLifecycleStatus::Created,
    );
    assert_eq!(
        tick.kms_scanout_target,
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_blocks_page_flip_readiness_for_invalid_scanout_target() {
    let root = ready_drm_sysfs_fixture("runtime-kms-scanout-target-invalid");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 720,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::InvalidFrameTarget,
            size: Some(Size {
                width: 0,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::InvalidFrameTarget,
            frame_serial: None,
        }
    );

    assembly
        .retire_gbm_egl_frame_target()
        .expect("invalid target should still retire through reduced lifecycle");
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::FrameTargetUnavailable,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetUnavailable,
            frame_serial: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_blocks_page_flip_readiness_for_frame_target_size_mismatch() {
    let root = ready_drm_sysfs_fixture("runtime-kms-scanout-target-size-mismatch");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 1920,
        height: 1080,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
            size: Some(Size {
                width: 1920,
                height: 1080,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetSizeMismatch,
            frame_serial: None,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_blocks_page_flip_until_frame_target_matches_output_resize() {
    let root = ready_drm_sysfs_fixture("runtime-output-size-mismatch");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let resized_output = Size {
        width: 1920,
        height: 1080,
    };

    assembly.observe_output_size(resized_output);

    assert_eq!(assembly.output_size_observation(), Some(resized_output));
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetSizeMismatch,
            frame_serial: None,
        }
    );
    let mismatch_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report output resize mismatch");
    assert_eq!(mismatch_tick.output_size, Some(resized_output));
    assert_eq!(
        mismatch_tick.kms_scanout_target.status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    let frame_target = assembly.observe_gbm_egl_frame_target_size(resized_output);

    assert_eq!(
        frame_target,
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: resized_output,
        }
    );
    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(resized_output),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    let ready_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report matched output and frame target");
    assert_eq!(ready_tick.output_size, Some(resized_output));
    assert_eq!(ready_tick.gbm_egl_frame_target, Some(frame_target));
    assert_eq!(
        ready_tick.kms_scanout_target,
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(resized_output),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_clears_stale_frame_target_allocation_on_output_resize() {
    let root = ready_drm_sysfs_fixture("runtime-output-resize-clears-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);
    let original_size = assembly
        .output_size_observation()
        .expect("ready output size");
    let allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready target should allocate");

    assembly.observe_output_size(original_size);
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    assembly.observe_output_size(Size {
        width: 1920,
        height: 1080,
    });

    assert_eq!(assembly.gbm_egl_frame_target_allocation_observation(), None);
    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_keeps_degraded_scanout_target_reduced() {
    let root = ready_drm_sysfs_fixture("runtime-kms-scanout-target-degraded");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assembly.observe_presentation_report(LiveRendererPresentationReport {
        status: LiveRendererPresentationStatus::Degraded,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation(),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Degraded,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Degraded,
            frame_serial: None,
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced degraded target");
    assert_eq!(
        tick.kms_scanout_target,
        assembly.kms_scanout_target_observation()
    );
    assert_eq!(tick.page_flip, assembly.page_flip_observation());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_updates_reduced_gbm_egl_frame_target_size() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-size");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    let resized = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 2560,
        height: 1440,
    });
    assert_eq!(
        resized,
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::Ready,
            size: Size {
                width: 2560,
                height: 1440,
            },
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report resized target");
    assert_eq!(tick.gbm_egl_frame_target, Some(resized));

    let invalid = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 1440,
    });
    assert_eq!(
        invalid,
        LiveGbmEglFrameTargetRecord {
            status: LiveGbmEglFrameTargetStatus::InvalidSize,
            size: Size {
                width: 0,
                height: 1440,
            },
        }
    );

    let invalid_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report invalid reduced target");
    assert_eq!(invalid_tick.gbm_egl_frame_target, Some(invalid));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_reduced_gbm_egl_frame_target_lifecycle() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-lifecycle");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let startup_target = assembly
        .gbm_egl_frame_target_observation()
        .expect("startup should seed reduced target");

    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target: startup_target,
        })
    );

    let retained = assembly.observe_gbm_egl_frame_target_size(startup_target.size);
    assert_eq!(retained, startup_target);
    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retained,
            target: startup_target,
        })
    );

    let resized = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 1920,
        height: 1080,
    });
    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Resized,
            target: resized,
        })
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced lifecycle");
    assert_eq!(
        tick.gbm_egl_frame_target_lifecycle,
        assembly.gbm_egl_frame_target_lifecycle_observation()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_reduced_gbm_egl_frame_target_allocation() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);

    let allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready startup target should allocate through reduced seam");
    assert_eq!(
        allocation,
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::Ready,
            target: LiveGbmEglFrameTargetRecord {
                status: LiveGbmEglFrameTargetStatus::Ready,
                size: Size {
                    width: 1280,
                    height: 720,
                },
            },
        }
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced allocation");
    assert_eq!(tick.gbm_egl_frame_target_allocation, Some(allocation));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_invalid_and_clears_stale_frame_target_allocation() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-allocation-invalid");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);

    let ready = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready startup target should allocate through reduced seam");
    assert_eq!(ready.status, LiveGbmEglFrameTargetAllocationStatus::Ready);

    let invalid_target = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 720,
    });
    assert_eq!(
        invalid_target.status,
        LiveGbmEglFrameTargetStatus::InvalidSize
    );
    assert_eq!(assembly.gbm_egl_frame_target_allocation_observation(), None);

    let invalid_allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("invalid target still reports a reduced allocation result");
    assert_eq!(
        invalid_allocation,
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::InvalidTarget,
            target: invalid_target,
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced invalid allocation");
    assert_eq!(
        tick.gbm_egl_frame_target_allocation,
        Some(invalid_allocation)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_or_retires_reduced_frame_target_allocation() {
    let root = ready_drm_sysfs_fixture("runtime-frame-target-lifecycle-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut allocator =
        FakeGbmEglFrameTargetAllocator::new(LiveGbmEglFrameTargetAllocationStatus::Ready);

    let allocation = assembly
        .allocate_gbm_egl_frame_target(&mut allocator)
        .expect("ready startup target should allocate through reduced seam");
    let target = allocation.target;

    assembly.observe_gbm_egl_frame_target_size(target.size);
    assert_eq!(
        assembly.gbm_egl_frame_target_lifecycle_observation(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retained,
            target,
        })
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    let retired = assembly
        .retire_gbm_egl_frame_target()
        .expect("ready target should retire");
    assert_eq!(
        retired,
        LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retired,
            target,
        }
    );
    assert_eq!(assembly.gbm_egl_frame_target_observation(), None);
    assert_eq!(assembly.gbm_egl_frame_target_allocation_observation(), None);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced retire lifecycle");
    assert_eq!(tick.gbm_egl_frame_target, None);
    assert_eq!(
        tick.gbm_egl_frame_target_lifecycle,
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retired,
            target,
        })
    );
    assert_eq!(tick.gbm_egl_frame_target_allocation, None);

    fs::remove_dir_all(root).unwrap();
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
struct MissingRenderDevice;

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl sophia_backend_live::RenderDeviceDiscoveryBackend for MissingRenderDevice {
    type Device = std::fs::File;

    fn open_render_device(&self) -> std::io::Result<Self::Device> {
        Err(std::io::Error::from_raw_os_error(19))
    }
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[test]
fn live_runtime_assembly_reports_native_frame_target_allocation_without_handles() {
    let root = ready_drm_sysfs_fixture("runtime-native-frame-target-allocation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    let allocation = assembly
        .allocate_native_gbm_egl_frame_target_with_gbm_device(&MissingRenderDevice)
        .expect("ready startup target should produce reduced native allocation report");
    assert_eq!(
        allocation.status,
        LiveGbmEglFrameTargetAllocationStatus::Unavailable
    );
    assert_eq!(
        assembly.gbm_egl_frame_target_allocation_observation(),
        Some(allocation)
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report reduced native allocation");
    assert_eq!(tick.gbm_egl_frame_target_allocation, Some(allocation));

    fs::remove_dir_all(root).unwrap();
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[test]
fn live_runtime_assembly_rejects_invalid_native_frame_target_before_device_open() {
    let root = ready_drm_sysfs_fixture("runtime-native-frame-target-allocation-invalid");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    let invalid_target = assembly.observe_gbm_egl_frame_target_size(Size {
        width: 0,
        height: 720,
    });
    let allocation = assembly
        .allocate_native_gbm_egl_frame_target_with_gbm_device(&MissingRenderDevice)
        .expect("invalid startup target should produce reduced native allocation report");

    assert_eq!(
        allocation,
        LiveGbmEglFrameTargetAllocationReport {
            status: LiveGbmEglFrameTargetAllocationStatus::InvalidTarget,
            target: invalid_target,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn page_flip_callback_intake_accepts_only_matching_monotonic_callbacks() {
    let mut intake = LivePageFlipCallbackIntake::new(OutputId::from_raw(7));

    assert_eq!(
        intake.observe(LivePageFlipCallback {
            output: OutputId::from_raw(8),
            frame_serial: 41,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedUnexpectedOutput,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::WaitingForOutput,
                frame_serial: None,
            },
        }
    );
    assert_eq!(intake.last_frame_serial(), None);

    assert_eq!(
        intake.observe(LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 41,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(41),
            },
        }
    );
    assert_eq!(intake.last_frame_serial(), Some(41));

    assert_eq!(
        intake.observe(LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 41,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        }
    );
    assert_eq!(intake.last_frame_serial(), Some(41));
}

#[test]
fn live_runtime_assembly_observes_reduced_page_flip_callbacks() {
    let root = ready_drm_sysfs_fixture("runtime-page-flip-callback");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");

    assert_eq!(
        assembly.observe_page_flip_callback(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 17,
        }),
        LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(17),
            },
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should succeed");

    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(17),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_commits_atomic_scanout_after_accepted_page_flip() {
    let root = ready_drm_sysfs_fixture("runtime-atomic-scanout-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut committer = FakeAtomicScanoutCommitter::default();

    let report = assembly.commit_atomic_scanout_after_page_flip_with(
        &mut committer,
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 31,
        },
        &PageFlipCommitOutcome::Committed {
            frame_serial: 31,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(61),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(111, 1)],
            },
        },
    );

    assert_eq!(
        report,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Committed,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(31),
            },
        }
    );
    assert_eq!(committer.committed_count(), 1);
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(31),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_preserves_timed_out_atomic_scanout_status() {
    let root = ready_drm_sysfs_fixture("runtime-atomic-scanout-timed-out");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut committer = FakeAtomicScanoutCommitter::default();

    let report = assembly.commit_atomic_scanout_after_page_flip_with(
        &mut committer,
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 32,
        },
        &PageFlipCommitOutcome::Rejected {
            frame_serial: 32,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(63),
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
        },
    );

    assert_eq!(
        report,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::TimedOut,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(32),
            },
        }
    );
    assert_eq!(committer.committed_count(), 0);
    assert_eq!(
        assembly.page_flip_observation(),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(32),
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_rejects_stale_page_flip_before_atomic_scanout_commit() {
    let root = ready_drm_sysfs_fixture("runtime-atomic-scanout-stale-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly");
    let mut committer = FakeAtomicScanoutCommitter::default();

    assembly.observe_page_flip_callback(LivePageFlipCallback {
        output: OutputId::from_raw(1),
        frame_serial: 41,
    });
    let report = assembly.commit_atomic_scanout_after_page_flip_with(
        &mut committer,
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 41,
        },
        &PageFlipCommitOutcome::Committed {
            frame_serial: 41,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(62),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(112, 1)],
            },
        },
    );

    assert_eq!(
        report,
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Rejected,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        }
    );
    assert_eq!(committer.committed_count(), 0);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_drains_bounded_page_flip_callback_queue() {
    let root = ready_drm_sysfs_fixture("runtime-page-flip-callback-queue");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(2);
    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 22,
        })
        .expect("test channel should accept first callback");
    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 23,
        })
        .expect("test channel should accept second callback");
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 1));

    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first callback");
    assert_eq!(
        first_tick.page_flip_callbacks,
        LivePageFlipCallbackQueueReport {
            drained: 1,
            accepted: 1,
            rejected_unexpected_output: 0,
            rejected_stale_frame_serial: 0,
            last_accepted: Some(LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::Accepted,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Presented,
                    frame_serial: Some(22),
                },
            }),
            disconnected: false,
            max_reached: true,
        }
    );
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(22),
        }
    );

    drop(sender);
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain queued callback");
    assert_eq!(
        second_tick.page_flip_callbacks,
        LivePageFlipCallbackQueueReport {
            drained: 1,
            accepted: 1,
            rejected_unexpected_output: 0,
            rejected_stale_frame_serial: 0,
            last_accepted: Some(LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::Accepted,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::Presented,
                    frame_serial: Some(23),
                },
            }),
            disconnected: false,
            max_reached: true,
        }
    );
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(23),
        }
    );

    let disconnected_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should report disconnected callback queue");
    assert_eq!(
        disconnected_tick.page_flip_callbacks,
        LivePageFlipCallbackQueueReport {
            drained: 0,
            accepted: 0,
            rejected_unexpected_output: 0,
            rejected_stale_frame_serial: 0,
            last_accepted: None,
            disconnected: true,
            max_reached: false,
        }
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fake_page_flip_callback_source_feeds_bounded_runtime_queue() {
    let root = ready_drm_sysfs_fixture("fake-page-flip-source");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut source = FakePageFlipCallbackSource::new([
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 31,
        },
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 32,
        },
    ]);

    assert_eq!(
        source.emit_ready(&sender, 4),
        LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 1,
            backpressure: true,
            disconnected: false,
            max_reached: false,
        }
    );
    assert_eq!(source.queued_len(), 1);

    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first fake callback");
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(31),
        }
    );

    assert_eq!(
        source.emit_ready(&sender, 4),
        LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        }
    );
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain second fake callback");
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(32),
        }
    );

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
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Wgpu,
            LiveBackendDependencyUse::RendererImport,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "validated GBM/EGL startup, drawing, and presentation seams",
        }
    );
    assert_eq!(
        live_backend_dependency_decision(
            LiveBackendDependencyKind::Wgpu,
            LiveBackendDependencyUse::RuntimePolling,
        ),
        LiveBackendDependencyDecision::Deferred {
            required_boundary: "validated GBM/EGL startup, drawing, and presentation seams",
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
fn kms_scanout_target_reports_ready_size_without_kms_identity() {
    let root = ready_drm_sysfs_fixture("kms-scanout-target-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Ready,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
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
fn kms_scanout_target_reports_missing_output_without_kms_identity() {
    let root = drm_sysfs_fixture("kms-scanout-target-no-output");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::OutputUnavailable,
            size: None,
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
fn kms_scanout_target_collapses_presentation_without_native_details() {
    let root = ready_drm_sysfs_fixture("kms-scanout-target-presentation");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));

    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Unavailable,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::PresentationUnavailable,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
        }
    );
    assert_eq!(
        report.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Degraded,
        }),
        LiveKmsScanoutTargetReport {
            status: LiveKmsScanoutTargetStatus::Degraded,
            size: Some(Size {
                width: 1280,
                height: 720,
            }),
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
fn page_flip_event_projects_kms_scanout_target_without_kms_identity() {
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(LiveKmsScanoutTargetStatus::Ready),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Ready,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(
            LiveKmsScanoutTargetStatus::FrameTargetUnavailable,
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetUnavailable,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(
            LiveKmsScanoutTargetStatus::InvalidFrameTarget,
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::InvalidFrameTarget,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(
            LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
        ),
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::FrameTargetSizeMismatch,
            frame_serial: None,
        }
    );
    assert_eq!(
        LivePageFlipEvent::from_kms_scanout_target_status(LiveKmsScanoutTargetStatus::Degraded),
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

#[test]
fn atomic_scanout_commit_report_reduces_page_flip_outcomes() {
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(&PageFlipCommitOutcome::Committed {
            frame_serial: 91,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(57),
                outcome: TransactionOutcome::Committed,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
            },
        }),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Committed,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(91),
            },
        }
    );
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(&PageFlipCommitOutcome::Rejected {
            frame_serial: 92,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(58),
                outcome: TransactionOutcome::RejectedInvalidSurface,
                applied_surfaces: vec![sophia_protocol::SurfaceId::new(89, 1)],
            },
        }),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Rejected,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(92),
            },
        }
    );
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(&PageFlipCommitOutcome::Rejected {
            frame_serial: 93,
            commit: TransactionCommit {
                transaction: TransactionId::from_raw(60),
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
        }),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::TimedOut,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(93),
            },
        }
    );
    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(
            &PageFlipCommitOutcome::WaitingForTransactionReadiness {
                transaction: TransactionId::from_raw(59),
                pending_surfaces: vec![sophia_protocol::SurfaceId::new(90, 1)],
            },
        ),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::WaitingForTransactionReadiness,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::WaitingForTransactionReadiness,
                frame_serial: None,
            },
        }
    );
}

#[test]
fn atomic_scanout_commit_report_rejects_mismatched_page_flip_frame_serial() {
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(90),
        },
    };

    assert_eq!(
        LiveAtomicScanoutCommitReport::from_page_flip_callback_and_outcome(
            &callback,
            &PageFlipCommitOutcome::Committed {
                frame_serial: 91,
                commit: TransactionCommit {
                    transaction: TransactionId::from_raw(57),
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
                },
            },
        ),
        LiveAtomicScanoutCommitReport {
            status: LiveAtomicScanoutCommitStatus::Rejected,
            page_flip: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(90),
            },
        }
    );
}

#[test]
fn fake_atomic_scanout_committer_counts_only_committed_outcomes() {
    let mut committer = FakeAtomicScanoutCommitter::default();

    let committed = committer.commit_atomic_scanout(&PageFlipCommitOutcome::Committed {
        frame_serial: 91,
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(57),
            outcome: TransactionOutcome::Committed,
            applied_surfaces: vec![sophia_protocol::SurfaceId::new(88, 1)],
        },
    });
    assert_eq!(committed.status, LiveAtomicScanoutCommitStatus::Committed);
    assert_eq!(committer.committed_count(), 1);

    let waiting =
        committer.commit_atomic_scanout(&PageFlipCommitOutcome::WaitingForTransactionReadiness {
            transaction: TransactionId::from_raw(59),
            pending_surfaces: vec![sophia_protocol::SurfaceId::new(90, 1)],
        });
    assert_eq!(
        waiting.status,
        LiveAtomicScanoutCommitStatus::WaitingForTransactionReadiness
    );
    assert_eq!(committer.committed_count(), 1);

    let rejected = committer.commit_atomic_scanout(&PageFlipCommitOutcome::Rejected {
        frame_serial: 92,
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(58),
            outcome: TransactionOutcome::RejectedInvalidSurface,
            applied_surfaces: vec![sophia_protocol::SurfaceId::new(89, 1)],
        },
    });
    assert_eq!(rejected.status, LiveAtomicScanoutCommitStatus::Rejected);
    assert_eq!(committer.committed_count(), 1);

    let timed_out = committer.commit_atomic_scanout(&PageFlipCommitOutcome::Rejected {
        frame_serial: 93,
        commit: TransactionCommit {
            transaction: TransactionId::from_raw(60),
            outcome: TransactionOutcome::TimedOut,
            applied_surfaces: Vec::new(),
        },
    });
    assert_eq!(timed_out.status, LiveAtomicScanoutCommitStatus::TimedOut);
    assert_eq!(committer.committed_count(), 1);
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

fn motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial * 10,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x, y }),
        target_surface: None,
        local_position: None,
    }
}

fn test_layer(raw_surface: u32) -> LayerSnapshot {
    LayerSnapshot {
        surface: SurfaceId::new(raw_surface, 1),
        authority_local_id: None,
        namespace: Some(NamespaceId::from_raw(7)),
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 160,
            height: 90,
        },
        source: BufferSource::CpuBuffer { handle: 900 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 160,
            height: 90,
        }),
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

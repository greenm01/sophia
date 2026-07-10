#![cfg(feature = "libdrm-events")]

use std::{io, sync::mpsc};

use sophia_backend_live::{
    CompositorBackendTickInput, FakeLibdrmNativePageFlipReader, FakeLibdrmPageFlipEventPoller,
    LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS,
    LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS, LibdrmBackendFdAuthority,
    LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
    LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus,
    LibdrmNativeAtomicCommitDevice, LibdrmNativeAtomicCommitFlagsReport,
    LibdrmNativeAtomicCommitRequest, LibdrmNativeAtomicCommitRequestScope,
    LibdrmNativeAtomicCommitSubmitReport, LibdrmNativeAtomicCommitSubmitStatus,
    LibdrmNativeAtomicRequestBuildStatus, LibdrmNativeAtomicScanoutPageFlipWaitStatus,
    LibdrmNativeAtomicScanoutSmokeEvidence, LibdrmNativeAtomicScanoutSmokePhase,
    LibdrmNativeAtomicScanoutSmokeStatus, LibdrmNativeConnectorSnapshot, LibdrmNativeCrtcRoute,
    LibdrmNativeEncoderSnapshot, LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus,
    LibdrmNativeKmsSelectionDevice, LibdrmNativeOutputRoute, LibdrmNativeOutputSlot,
    LibdrmNativePageFlipCallback, LibdrmNativePageFlipDecodeReport,
    LibdrmNativePageFlipDecodeStatus, LibdrmNativePageFlipReadResult, LibdrmNativePageFlipReader,
    LibdrmNativePageFlipSource, LibdrmNativePageFlipSourceReport, LibdrmNativePageFlipSourceStatus,
    LibdrmNativePlaneSnapshot, LibdrmNativePollerDiagnostics, LibdrmNativePrimaryPlaneObjects,
    LibdrmNativePrimaryPlanePropertyDiscoveryStatus, LibdrmNativePrimaryPlanePropertyHandles,
    LibdrmNativePrimaryPlaneResourceCreateStatus, LibdrmNativePrimaryPlaneResourceDestroyStatus,
    LibdrmNativePrimaryPlaneResourceDevice, LibdrmNativePrimaryPlaneScanoutRetireResult,
    LibdrmNativePrimaryPlaneScanoutRetireStatus, LibdrmNativePrimaryPlaneScanoutSubmitPolicy,
    LibdrmNativePrimaryPlaneScanoutSubmitStatus, LibdrmNativePrimaryPlaneSelectionResult,
    LibdrmNativePrimaryPlaneSelectionStatus, LibdrmNativePropertyHandleSet,
    LibdrmNativePropertyLookupDevice, LibdrmNativeReadAndPollReport, LibdrmNativeReadLoopReport,
    LibdrmNativeReadLoopStatus, LibdrmNativeRenderedScanoutContextStatus,
    LibdrmPageFlipEventPollReport, LibdrmPageFlipEventPollStatus, LibdrmPageFlipEventPoller,
    LibdrmRendererScanoutBuffer, LiveAtomicScanoutPreflightReport,
    LiveAtomicScanoutPreflightStatus, LiveBackendConfig, LiveHardwareValidationGateReport,
    LiveHardwareValidationGateStatus, LiveHardwareValidationSmokeReport,
    LiveHardwareValidationSmokeStatus, LiveHardwareValidationTarget, LiveKmsScanoutTargetStatus,
    LiveLibdrmPollerDiagnostics, LiveLibdrmPollerDiagnosticsStatus, LiveLibdrmPollerStartupReport,
    LiveLibdrmPollerStartupStatus, LivePageFlipCallback, LivePageFlipCallbackDecision,
    LivePageFlipCallbackQueue, LivePageFlipCallbackReport, LivePageFlipCallbackSourceReport,
    LivePageFlipEvent, LivePageFlipEventStatus, LiveRenderedPrimaryPlaneScanoutBackpressureReport,
    LiveRenderedPrimaryPlaneScanoutBackpressureStatus, LiveRenderedPrimaryPlaneScanoutSubmitStatus,
    LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter,
    LiveRuntimeRenderedScanoutEvidenceFailureReport,
    LiveRuntimeRenderedScanoutEvidenceFailureStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus, NativeLibdrmAtomicScanoutCommitter,
    NativeLibdrmPageFlipEventPoller, NativeLibdrmPageFlipEventReader, OutputId, QueuedInputPoller,
    RealAtomicScanoutCardSelectionStatus, RealAtomicScanoutPageFlipSessionStatus,
    RealAtomicScanoutPageFlipWaitPolicy, RuntimeScanoutState, Size,
    build_native_primary_plane_atomic_request, build_native_primary_plane_page_flip_atomic_request,
    create_native_primary_plane_page_flip_resources, create_native_primary_plane_resources,
    decode_native_page_flip_batch, destroy_native_primary_plane_resources, discover_live_backend,
    discover_native_primary_plane_property_handles, libdrm_dependency_admission_report,
    libdrm_fd_authority_report, native_libdrm_event_adapter_report,
    native_libdrm_event_adapter_report_for_authority, real_atomic_scanout_preflight_report,
    real_atomic_scanout_validation_gate, real_atomic_scanout_validation_smoke_report,
    real_libdrm_events_validation_gate, real_libdrm_events_validation_smoke_report,
    reduce_native_page_flip_event, retire_native_primary_plane_scanout_after_page_flip,
    retire_rendered_primary_plane_scanout_after_page_flip, select_native_primary_plane_target,
    select_real_atomic_scanout_card_from_dev_dri,
    submit_native_primary_plane_scanout_from_renderer_descriptor,
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor,
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy,
};
#[cfg(feature = "libinput-events")]
use sophia_backend_live::{
    DeviceId, FakeLiveLibinputEventReader, InputEventPacket, LibinputDeviceDescriptor,
    LibinputDeviceKind, LiveBackendReadinessCollector, LiveBackendSessionLoop,
    LiveBackendSessionLoopPageFlipBudget, LiveBackendSessionLoopReadiness,
    LiveInputReadinessGateStatus, LiveInputReadinessGatedPoller, NativeLibinputEventPoller, SeatId,
};
#[cfg(feature = "gbm-probe")]
use sophia_backend_live::{
    LiveGbmEglFrameTargetStatus, NativeGbmRenderedScanoutBufferDiscoveryExporter,
    NativeGbmRenderedScanoutContextStatus, RealAtomicScanoutSmokeConfig,
    RenderDeviceDiscoveryBackend,
};
#[cfg(feature = "gbm-probe")]
use sophia_backend_live::{
    LiveRendererImportHealth, LiveRendererImportPathStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation, real_atomic_runtime_rendered_scanout_renderer_observation,
};
#[cfg(feature = "libinput-events")]
use sophia_protocol::{InputEventKind, Point};
use sophia_renderer_live::{
    FakeRendererScanoutBufferExporter, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
    LiveGbmEglFrameTargetRecord, LiveRendererScanoutBufferExportStatus,
    LiveRendererScanoutBufferExporter,
};
#[cfg(feature = "gbm-probe")]
use sophia_renderer_live::{
    LiveGbmEglFrameTargetLifecycleReport, LiveGbmEglFrameTargetLifecycleStatus,
};

#[test]
fn runtime_rendered_scanout_failure_evidence_has_stable_reduced_line() {
    let report = LiveRuntimeRenderedScanoutEvidenceFailureReport::new(
        LiveRuntimeRenderedScanoutEvidenceFailureStatus::RetireTimedOut,
        true,
        false,
    );

    assert_eq!(
        report.reduced_log_line(),
        "sophia_runtime_rendered_scanout_failure schema=1 status=RetireTimedOut submit_seen=true retire_seen=false"
    );
}

#[cfg(feature = "gbm-probe")]
#[test]
fn real_runtime_rendered_scanout_evidence_reports_native_renderer_observation() {
    assert_eq!(
        real_atomic_runtime_rendered_scanout_renderer_observation(),
        LiveRendererRuntimeObservation {
            health: LiveRendererImportHealth::NativeImportCapable,
            xpixmap: LiveRendererImportPathStatus::Disabled,
            dmabuf: LiveRendererImportPathStatus::Enabled,
            selection: LiveRendererSelectionObservation::NativeImportCapable,
        }
    );
}

#[test]
fn libdrm_dependency_is_admitted_without_exposing_native_event_shape() {
    assert_eq!(
        libdrm_dependency_admission_report(),
        LibdrmDependencyAdmissionReport {
            status: LibdrmDependencyAdmissionStatus::TypedPageFlipEventAvailable,
        }
    );
}

#[test]
fn real_libdrm_event_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert!(!skipped.is_requested());
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_libdrm_events_validation_gate().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn runtime_page_flip_observation_prefers_accepted_callback_over_later_stale_rejection() {
    let root = ready_drm_sysfs_fixture("runtime-page-flip-accepted-dominates-stale");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(4);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));

    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 62,
        })
        .expect("test channel should accept first callback");
    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 61,
        })
        .expect("test channel should accept stale callback");

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain callbacks");

    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(62),
        }
    );
    assert_eq!(tick.page_flip_callbacks.drained, 2);
    assert_eq!(tick.page_flip_callbacks.accepted, 1);
    assert_eq!(tick.page_flip_callbacks.rejected_stale_frame_serial, 1);
    assert_eq!(
        tick.page_flip_callbacks
            .last_accepted
            .expect("accepted callback should be retained")
            .event
            .frame_serial,
        Some(62)
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_libdrm_event_validation_smoke_fails_closed_without_device_opening_smoke() {
    let skipped = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            false,
        ),
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::SkippedOptInRequired,
        }
    );

    let requested = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            true,
        ),
    );
    assert_eq!(
        requested,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::BackendUnavailable,
        }
    );

    assert_eq!(
        real_libdrm_events_validation_smoke_report().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn real_atomic_scanout_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::AtomicScanout,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::AtomicScanout,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_atomic_scanout_validation_gate().target,
        LiveHardwareValidationTarget::AtomicScanout
    );
    assert_eq!(
        real_atomic_scanout_validation_smoke_report().target,
        LiveHardwareValidationTarget::AtomicScanout
    );
}

#[test]
fn real_atomic_scanout_card_selection_fails_closed_without_device_identity() {
    let missing_root = std::env::temp_dir().join("sophia-missing-dri-card-selection");
    let _ = std::fs::remove_dir_all(&missing_root);
    let missing = select_real_atomic_scanout_card_from_dev_dri(&missing_root);
    assert_eq!(
        missing.status,
        RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable
    );
    assert_eq!(
        missing.status.failure_evidence().status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert!(missing.card.is_none());
    assert!(missing.selection.is_none());
    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let authority =
        LibdrmBackendFdAuthority::new(31).expect("nonzero authority generation should mint");
    let missing_session = missing.into_page_flip_session(slot, OutputId::from_raw(1), authority);
    assert_eq!(
        missing_session.status,
        RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed
    );
    assert_eq!(
        missing_session.card_selection_status,
        RealAtomicScanoutCardSelectionStatus::DeviceDirectoryUnavailable
    );
    assert_eq!(
        missing_session
            .failure_evidence()
            .expect("failed session should reduce to smoke evidence")
            .status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert!(missing_session.session.is_none());

    let empty_root = std::env::temp_dir().join("sophia-empty-dri-card-selection");
    let _ = std::fs::remove_dir_all(&empty_root);
    std::fs::create_dir_all(&empty_root).unwrap();
    let empty = select_real_atomic_scanout_card_from_dev_dri(&empty_root);
    assert_eq!(
        empty.status,
        RealAtomicScanoutCardSelectionStatus::NoPrimaryCardNodes
    );
    assert_eq!(
        empty.status.failure_evidence().status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert!(empty.card.is_none());
    assert!(empty.selection.is_none());
    std::fs::remove_dir_all(empty_root).unwrap();
}

#[test]
fn real_atomic_scanout_page_flip_wait_policy_matches_hardware_smoke_budget() {
    let policy = RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke();

    assert_eq!(policy.max_read, 4);
    assert_eq!(policy.max_emit, 1);
    assert_eq!(policy.timeout, std::time::Duration::from_secs(2));
    assert_eq!(policy.sleep, std::time::Duration::from_millis(5));
}

#[cfg(feature = "gbm-probe")]
#[test]
fn real_atomic_scanout_smoke_config_rejects_zero_identity_fields() {
    let policy = RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke();
    let config = RealAtomicScanoutSmokeConfig::from_raw(1, 7, 9, policy)
        .expect("nonzero slot and authority should mint smoke config");

    assert_eq!(config.slot.raw(), 1);
    assert_eq!(config.output.raw(), 7);
    assert_eq!(config.authority.generation(), 9);
    assert_eq!(config.wait_policy, policy);
    assert!(RealAtomicScanoutSmokeConfig::from_raw(0, 7, 9, policy).is_none());
    assert!(RealAtomicScanoutSmokeConfig::from_raw(1, 7, 0, policy).is_none());
}

#[test]
fn atomic_scanout_preflight_reduces_host_readiness_without_identity() {
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(false, 2, 2, 2, 2, 2),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::DeviceDirectoryUnavailable,
            primary_card_nodes: 0,
            openable_primary_card_nodes: 0,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 0, 0, 0, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::NoPrimaryCardNodes,
            primary_card_nodes: 0,
            openable_primary_card_nodes: 0,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 0, 0, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::PrimaryCardOpenUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 0,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 1, 0, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::AtomicClientCapabilityUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 1,
            atomic_capable_primary_card_nodes: 0,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 0, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::KmsScanoutTargetUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 2,
            atomic_capable_primary_card_nodes: 1,
            scanout_target_primary_card_nodes: 0,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 1, 0),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::AtomicPropertyDiscoveryUnavailable,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 2,
            atomic_capable_primary_card_nodes: 1,
            scanout_target_primary_card_nodes: 1,
            atomic_property_primary_card_nodes: 0,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 1, 1),
        LiveAtomicScanoutPreflightReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveAtomicScanoutPreflightStatus::CandidatePrimaryCardsAtomicReady,
            primary_card_nodes: 2,
            openable_primary_card_nodes: 2,
            atomic_capable_primary_card_nodes: 1,
            scanout_target_primary_card_nodes: 1,
            atomic_property_primary_card_nodes: 1,
        }
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .openable_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .atomic_capable_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .scanout_target_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(
            true,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX
        )
        .atomic_property_primary_card_nodes,
        LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 1, 2, 2, 2)
            .atomic_capable_primary_card_nodes,
        1
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 1, 2, 2)
            .scanout_target_primary_card_nodes,
        1
    );
    assert_eq!(
        LiveAtomicScanoutPreflightReport::from_primary_card_counts(true, 2, 2, 2, 1, 2)
            .atomic_property_primary_card_nodes,
        1
    );

    let real = real_atomic_scanout_preflight_report();
    println!("{}", real.reduced_log_line());
    assert_eq!(real.target, LiveHardwareValidationTarget::AtomicScanout);
    assert!(real.primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS);
    assert!(real.openable_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS);
    assert!(
        real.atomic_capable_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert!(
        real.scanout_target_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert!(
        real.atomic_property_primary_card_nodes <= LIVE_ATOMIC_SCANOUT_PREFLIGHT_MAX_PRIMARY_CARDS
    );
    assert!(real.atomic_capable_primary_card_nodes <= real.openable_primary_card_nodes);
    assert!(real.scanout_target_primary_card_nodes <= real.atomic_capable_primary_card_nodes);
    assert!(real.atomic_property_primary_card_nodes <= real.scanout_target_primary_card_nodes);
    assert!(
        real.reduced_log_line()
            .starts_with("sophia_atomic_scanout_preflight schema=5 target=AtomicScanout status=")
    );
}

#[test]
fn libdrm_fd_authority_is_generation_checked_and_reduced() {
    assert_eq!(LibdrmBackendFdAuthority::new(0), None);

    let authority =
        LibdrmBackendFdAuthority::new(9).expect("nonzero generation should mint authority token");
    assert_eq!(authority.generation(), 9);
    assert_eq!(
        libdrm_fd_authority_report(authority),
        LibdrmBackendFdAuthorityReport {
            status: LibdrmBackendFdAuthorityStatus::BackendOwned,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_skeleton_reports_ready_without_opening_devices() {
    assert_eq!(
        native_libdrm_event_adapter_report(),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_accepts_authority_without_polling() {
    let authority =
        LibdrmBackendFdAuthority::new(12).expect("nonzero generation should mint authority token");

    assert_eq!(
        native_libdrm_event_adapter_report_for_authority(authority),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_page_flip_source_constructs_from_authority_without_reading_events() {
    let authority =
        LibdrmBackendFdAuthority::new(13).expect("nonzero generation should mint authority token");
    let source = LibdrmNativePageFlipSource::from_authority(authority);

    assert_eq!(
        source.report(),
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    );
}

#[derive(Debug)]
struct FakeNativeAtomicCommitDevice {
    result: io::Result<()>,
}

impl LibdrmNativeAtomicCommitDevice for FakeNativeAtomicCommitDevice {
    fn submit_atomic_commit(
        &self,
        _flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.result
            .as_ref()
            .map(|_| ())
            .map_err(|error| io::Error::new(error.kind(), "synthetic atomic commit failure"))
    }
}

#[derive(Debug)]
struct FakeNativePropertyLookupDevice {
    connector: io::Result<LibdrmNativePropertyHandleSet>,
    crtc: io::Result<LibdrmNativePropertyHandleSet>,
    plane: io::Result<LibdrmNativePropertyHandleSet>,
}

impl LibdrmNativePropertyLookupDevice for FakeNativePropertyLookupDevice {
    fn connector_property_handles(
        &self,
        _connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.connector)
    }

    fn crtc_property_handles(
        &self,
        _crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.crtc)
    }

    fn plane_property_handles(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.plane)
    }
}

#[derive(Debug)]
struct FakeNativeKmsSelectionDevice {
    connectors: io::Result<Vec<drm::control::connector::Handle>>,
    crtcs: io::Result<Vec<drm::control::crtc::Handle>>,
    planes: io::Result<Vec<drm::control::plane::Handle>>,
    connector_snapshot: io::Result<LibdrmNativeConnectorSnapshot>,
    encoder_snapshot: io::Result<LibdrmNativeEncoderSnapshot>,
    plane_snapshot: io::Result<LibdrmNativePlaneSnapshot>,
    plane_type: io::Result<Option<drm::control::PlaneType>>,
}

impl LibdrmNativeKmsSelectionDevice for FakeNativeKmsSelectionDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        clone_io_result(&self.connectors)
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        clone_io_result(&self.crtcs)
    }

    fn connector_snapshot(
        &self,
        _connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        clone_io_result(&self.connector_snapshot)
    }

    fn encoder_snapshot(
        &self,
        _encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        clone_io_result(&self.encoder_snapshot)
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        clone_io_result(&self.planes)
    }

    fn plane_snapshot(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        clone_io_result(&self.plane_snapshot)
    }

    fn plane_type(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        clone_io_result(&self.plane_type)
    }
}

#[derive(Debug)]
struct FakeNativePrimaryPlaneResourceDevice {
    mode_blob: io::Result<u64>,
    framebuffer: io::Result<drm::control::framebuffer::Handle>,
    destroy_framebuffer: io::Result<()>,
    destroy_mode_blob: io::Result<()>,
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        _selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn add_scanout_framebuffer<B>(
        &self,
        _buffer: &B,
        _depth: u32,
        _bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        clone_io_result(&self.framebuffer)
    }

    fn destroy_scanout_framebuffer(
        &self,
        _framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        clone_io_result(&self.destroy_framebuffer)
    }

    fn destroy_mode_blob(&self, _mode_blob: u64) -> io::Result<()> {
        clone_io_result(&self.destroy_mode_blob)
    }
}

#[derive(Debug)]
struct FakeNativePrimaryPlaneScanoutDevice {
    selection: FakeNativeKmsSelectionDevice,
    properties: FakeNativePropertyLookupDevice,
    resources: FakeNativePrimaryPlaneResourceDevice,
    submit: io::Result<()>,
}

impl LibdrmNativeKmsSelectionDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        self.selection.connector_handles()
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        self.selection.crtc_handles()
    }

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        self.selection.connector_snapshot(connector)
    }

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        self.selection.encoder_snapshot(encoder)
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        self.selection.plane_handles()
    }

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        self.selection.plane_snapshot(plane)
    }

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        self.selection.plane_type(plane)
    }
}

impl LibdrmNativePropertyLookupDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.connector_property_handles(connector)
    }

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.crtc_property_handles(crtc)
    }

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.plane_property_handles(plane)
    }
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        self.resources.create_mode_blob_for_selection(selection)
    }

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        self.resources.add_scanout_framebuffer(buffer, depth, bpp)
    }

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.resources.destroy_scanout_framebuffer(framebuffer)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.resources.destroy_mode_blob(mode_blob)
    }
}

impl LibdrmNativeAtomicCommitDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn submit_atomic_commit(
        &self,
        _flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        clone_io_result(&self.submit)
    }
}

fn clone_io_result<T: Clone>(result: &io::Result<T>) -> io::Result<T> {
    result
        .as_ref()
        .cloned()
        .map_err(|error| io::Error::new(error.kind(), "synthetic property lookup failure"))
}

fn property_handle(raw: u32) -> drm::control::property::Handle {
    drm::control::from_u32(raw).expect("test property handle should be nonzero")
}

fn connector_handle() -> drm::control::connector::Handle {
    drm::control::from_u32(11).expect("test connector handle should be nonzero")
}

fn crtc_handle() -> drm::control::crtc::Handle {
    drm::control::from_u32(12).expect("test crtc handle should be nonzero")
}

fn encoder_handle() -> drm::control::encoder::Handle {
    drm::control::from_u32(16).expect("test encoder handle should be nonzero")
}

fn plane_handle() -> drm::control::plane::Handle {
    drm::control::from_u32(13).expect("test plane handle should be nonzero")
}

fn framebuffer_handle() -> drm::control::framebuffer::Handle {
    drm::control::from_u32(14).expect("test framebuffer handle should be nonzero")
}

fn primary_plane_properties() -> LibdrmNativePrimaryPlanePropertyHandles {
    LibdrmNativePrimaryPlanePropertyHandles::new(
        property_handle(101),
        property_handle(102),
        property_handle(103),
        property_handle(104),
        property_handle(105),
        property_handle(106),
        property_handle(107),
        property_handle(108),
        property_handle(109),
        property_handle(110),
        property_handle(111),
        property_handle(112),
        property_handle(113),
    )
}

fn primary_plane_objects(size: Size) -> LibdrmNativePrimaryPlaneObjects {
    LibdrmNativePrimaryPlaneObjects::new(
        connector_handle(),
        crtc_handle(),
        plane_handle(),
        framebuffer_handle(),
        15,
        size,
    )
}

fn full_property_lookup_device() -> FakeNativePropertyLookupDevice {
    FakeNativePropertyLookupDevice {
        connector: Ok(LibdrmNativePropertyHandleSet::new([(
            "CRTC_ID",
            property_handle(101),
        )])),
        crtc: Ok(LibdrmNativePropertyHandleSet::new([
            ("MODE_ID", property_handle(102)),
            ("ACTIVE", property_handle(103)),
        ])),
        plane: Ok(LibdrmNativePropertyHandleSet::new([
            ("FB_ID", property_handle(104)),
            ("CRTC_ID", property_handle(105)),
            ("SRC_X", property_handle(106)),
            ("SRC_Y", property_handle(107)),
            ("SRC_W", property_handle(108)),
            ("SRC_H", property_handle(109)),
            ("CRTC_X", property_handle(110)),
            ("CRTC_Y", property_handle(111)),
            ("CRTC_W", property_handle(112)),
            ("CRTC_H", property_handle(113)),
        ])),
    }
}

fn kms_selection_device_with_mode_size(size: Size) -> FakeNativeKmsSelectionDevice {
    FakeNativeKmsSelectionDevice {
        connectors: Ok(vec![connector_handle()]),
        crtcs: Ok(vec![crtc_handle()]),
        planes: Ok(vec![plane_handle()]),
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            Some(encoder_handle()),
            [encoder_handle()],
            Some(size),
        )),
        encoder_snapshot: Ok(LibdrmNativeEncoderSnapshot::new(
            Some(crtc_handle()),
            [crtc_handle()],
        )),
        plane_snapshot: Ok(LibdrmNativePlaneSnapshot::new([crtc_handle()])),
        plane_type: Ok(Some(drm::control::PlaneType::Primary)),
    }
}

fn full_kms_selection_device() -> FakeNativeKmsSelectionDevice {
    kms_selection_device_with_mode_size(Size {
        width: 1280,
        height: 720,
    })
}

fn full_primary_plane_resource_device() -> FakeNativePrimaryPlaneResourceDevice {
    FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Ok(15),
        framebuffer: Ok(framebuffer_handle()),
        destroy_framebuffer: Ok(()),
        destroy_mode_blob: Ok(()),
    }
}

fn full_primary_plane_scanout_device() -> FakeNativePrimaryPlaneScanoutDevice {
    FakeNativePrimaryPlaneScanoutDevice {
        selection: full_kms_selection_device(),
        properties: full_property_lookup_device(),
        resources: full_primary_plane_resource_device(),
        submit: Ok(()),
    }
}

fn scanout_descriptor(size: Size) -> sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
    let mut exporter =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(
                size.width as u32 * 4,
                LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                17,
            );

    exporter
        .export_scanout_buffer(LiveGbmEglFrameTargetRecord::new(size))
        .descriptor
        .expect("ready fake renderer export should include a scanout descriptor")
}

fn scanout_buffer(size: Size) -> LibdrmRendererScanoutBuffer {
    LibdrmRendererScanoutBuffer::from_descriptor(scanout_descriptor(size))
        .expect("ready renderer descriptor should become a backend-private DRM buffer")
}

struct FakeDrmBuffer {
    size: (u32, u32),
    pitch: u32,
    format: drm::buffer::DrmFourcc,
    handle: drm::buffer::Handle,
}

impl FakeDrmBuffer {
    fn xrgb8888(size: Size) -> Self {
        Self {
            size: (size.width as u32, size.height as u32),
            pitch: size.width as u32 * 4,
            format: drm::buffer::DrmFourcc::Xrgb8888,
            handle: drm::control::from_u32(17).expect("test buffer handle should be nonzero"),
        }
    }

    fn with_pitch(mut self, pitch: u32) -> Self {
        self.pitch = pitch;
        self
    }

    fn with_format(mut self, format: drm::buffer::DrmFourcc) -> Self {
        self.format = format;
        self
    }
}

impl drm::buffer::Buffer for FakeDrmBuffer {
    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn format(&self) -> drm::buffer::DrmFourcc {
        self.format
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn handle(&self) -> drm::buffer::Handle {
        self.handle
    }
}

#[derive(Debug, Eq, PartialEq)]
struct FakeRenderedScanoutOwner {
    raw: u32,
}

struct FakeRenderedScanoutExporter {
    status: LiveRendererScanoutBufferExportStatus,
    descriptor: Option<sophia_renderer_live::LiveRendererScanoutBufferDescriptor>,
    owner: Option<FakeRenderedScanoutOwner>,
    export_attempts: usize,
}

#[cfg(feature = "gbm-probe")]
struct MissingRenderDevice;

#[cfg(feature = "gbm-probe")]
impl RenderDeviceDiscoveryBackend for MissingRenderDevice {
    type Device = std::fs::File;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "test render device unavailable",
        ))
    }
}

impl FakeRenderedScanoutExporter {
    fn exported(size: Size) -> Self {
        Self {
            status: LiveRendererScanoutBufferExportStatus::Exported,
            descriptor: Some(scanout_descriptor(size)),
            owner: Some(FakeRenderedScanoutOwner { raw: 7 }),
            export_attempts: 0,
        }
    }

    fn unavailable() -> Self {
        Self {
            status: LiveRendererScanoutBufferExportStatus::Unavailable,
            descriptor: None,
            owner: None,
            export_attempts: 0,
        }
    }

    fn export_attempts(&self) -> usize {
        self.export_attempts
    }
}

impl LiveRenderedScanoutBufferExporter for FakeRenderedScanoutExporter {
    type Owner = FakeRenderedScanoutOwner;

    fn export_rendered_scanout_buffer(
        &mut self,
        _target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        LiveRenderedScanoutBufferExport {
            status: self.status,
            descriptor: self.descriptor,
            owner: self.owner.take(),
        }
    }
}

#[cfg(feature = "libinput-events")]
fn libinput_motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial * 10,
        kind: InputEventKind::PointerMotion,
        global_position: Some(Point { x, y }),
        target_surface: None,
        target_window: None,
        local_position: None,
    }
}

#[test]
fn native_libdrm_atomic_commit_request_reports_reduced_flags() {
    let default_request =
        LibdrmNativeAtomicCommitRequest::new(drm::control::atomic::AtomicModeReq::new());
    assert_eq!(
        default_request.reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::PageFlip
    );
    assert_eq!(
        default_request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );
    assert_eq!(
        LibdrmNativeAtomicCommitRequest::modeset(drm::control::atomic::AtomicModeReq::new())
            .reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::Modeset
    );

    let explicit_request =
        LibdrmNativeAtomicCommitRequest::new(drm::control::atomic::AtomicModeReq::new())
            .without_page_flip_event()
            .blocking()
            .allow_modeset()
            .test_only();
    assert_eq!(
        explicit_request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: false,
            nonblocking: false,
            allow_modeset: true,
            test_only: true,
        }
    );
}

#[test]
fn native_libdrm_primary_plane_property_discovery_feeds_request_builder() {
    let discovery = discover_native_primary_plane_property_handles(
        &full_property_lookup_device(),
        connector_handle(),
        crtc_handle(),
        plane_handle(),
    );

    assert_eq!(
        discovery.status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered
    );
    let properties = discovery
        .properties
        .expect("complete lookup should produce private property handles");
    let build = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        properties,
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    assert!(build.request.is_some());
}

#[test]
fn native_libdrm_primary_plane_property_discovery_fails_closed_for_missing_groups() {
    let missing_connector = FakeNativePropertyLookupDevice {
        connector: Ok(LibdrmNativePropertyHandleSet::new(Vec::<(
            &str,
            drm::control::property::Handle,
        )>::new())),
        ..full_property_lookup_device()
    };
    assert_eq!(
        discover_native_primary_plane_property_handles(
            &missing_connector,
            connector_handle(),
            crtc_handle(),
            plane_handle(),
        )
        .status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingConnectorProperty
    );

    let missing_crtc = FakeNativePropertyLookupDevice {
        crtc: Ok(LibdrmNativePropertyHandleSet::new([(
            "MODE_ID",
            property_handle(102),
        )])),
        ..full_property_lookup_device()
    };
    assert_eq!(
        discover_native_primary_plane_property_handles(
            &missing_crtc,
            connector_handle(),
            crtc_handle(),
            plane_handle(),
        )
        .status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingCrtcProperty
    );

    let missing_plane = FakeNativePropertyLookupDevice {
        plane: Ok(LibdrmNativePropertyHandleSet::new([
            ("FB_ID", property_handle(104)),
            ("CRTC_ID", property_handle(105)),
        ])),
        ..full_property_lookup_device()
    };
    assert_eq!(
        discover_native_primary_plane_property_handles(
            &missing_plane,
            connector_handle(),
            crtc_handle(),
            plane_handle(),
        )
        .status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingPlaneProperty
    );
}

#[test]
fn native_libdrm_primary_plane_property_discovery_fails_closed_on_read_error() {
    let read_failed = FakeNativePropertyLookupDevice {
        connector: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_property_lookup_device()
    };
    let discovery = discover_native_primary_plane_property_handles(
        &read_failed,
        connector_handle(),
        crtc_handle(),
        plane_handle(),
    );

    assert_eq!(
        discovery.status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed
    );
    assert!(discovery.properties.is_none());
}

#[test]
fn native_libdrm_primary_plane_selection_feeds_request_builder() {
    let selection = select_native_primary_plane_target(&full_kms_selection_device());

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    let selected = selection
        .selection
        .expect("complete KMS path should select a private primary plane target");
    assert_eq!(
        selected.size(),
        Size {
            width: 1280,
            height: 720,
        }
    );
    let resource_create = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        resource_create.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let objects = resource_create
        .resources
        .expect("complete resource device should produce framebuffer and mode blob")
        .into_objects(selected);
    let properties = discover_native_primary_plane_property_handles(
        &full_property_lookup_device(),
        connector_handle(),
        crtc_handle(),
        plane_handle(),
    )
    .properties
    .expect("complete property lookup should produce private property handles");
    let build = build_native_primary_plane_atomic_request(objects, properties);

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    assert!(build.request.is_some());
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_chains_renderer_descriptor_to_atomic_commit() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    assert_eq!(
        result.properties,
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
    );
    assert_eq!(
        result.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
    );
    assert_eq!(
        result.request,
        Some(LibdrmNativeAtomicRequestBuildStatus::Built)
    );
    assert_eq!(
        result.submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Submitted)
    );
    assert_eq!(
        result.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: true,
            test_only: false,
        })
    );
    assert_eq!(
        result.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::Modeset)
    );

    let retired = result
        .submission
        .expect("submitted scanout should retain resource ownership until page flip")
        .retire(&device);
    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_page_flip_policy_disallows_modeset() {
    let device = full_primary_plane_scanout_device();
    assert_eq!(
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip().expected_request_scope(),
        LibdrmNativeAtomicCommitRequestScope::PageFlip
    );
    assert_eq!(
        LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset().expected_request_scope(),
        LibdrmNativeAtomicCommitRequestScope::Modeset
    );

    let selection = select_native_primary_plane_target(&device);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        })
    );
    assert_eq!(
        result.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(
        result
            .submission
            .expect("page-flip submit should retain resources")
            .retire(&device)
            .status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );

    let mode_unavailable = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let selection = select_native_primary_plane_target(&mode_unavailable);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &mode_unavailable,
            selection,
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result
            .submission
            .expect("page-flip submit should not retain a mode blob")
            .retire(&mode_unavailable)
            .status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_retains_cleanup_after_submit_failure() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        submit: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_scanout_device()
    };
    let selection = select_native_primary_plane_target(&device);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed
    );
    assert_eq!(
        result.submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Rejected)
    );
    assert!(result.submission.is_none());
    let cleanup = result
        .cleanup
        .expect("submit failure must retain failed cleanup");
    assert_eq!(
        cleanup.retry(&full_primary_plane_scanout_device()).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_retains_resource_creation_cleanup() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_mode_blob: Err(io::Error::other("test mode blob destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let selection = select_native_primary_plane_target(&device);
    let result =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
            &device,
            selection,
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset(),
        );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ResourceCreationUnavailable
    );
    assert_eq!(
        result.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed)
    );
    assert!(result.request.is_none());
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());
    let cleanup = result
        .cleanup
        .expect("resource creation failure must retain failed mode blob cleanup");
    assert_eq!(
        cleanup.retry(&full_primary_plane_scanout_device()).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_uses_supplied_selection_snapshot() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        &device,
        LibdrmNativePrimaryPlaneSelectionResult {
            status: LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector,
            selection: None,
        },
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
    );
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());

    let selected = select_native_primary_plane_target(&device)
        .selection
        .expect("complete KMS path should produce a selected payload");
    let forged_not_selected =
        submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
            &device,
            LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane,
                selection: Some(selected),
            },
            scanout_descriptor(Size {
                width: 1280,
                height: 720,
            }),
        );

    assert_eq!(
        forged_not_selected.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable
    );
    assert_eq!(
        forged_not_selected.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
    );
    assert!(forged_not_selected.properties.is_none());
    assert!(forged_not_selected.resources.is_none());
    assert!(forged_not_selected.request.is_none());
    assert!(forged_not_selected.submit.is_none());
    assert!(forged_not_selected.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_fails_closed_for_bad_descriptor() {
    let device = full_primary_plane_scanout_device();
    let descriptor = sophia_renderer_live::LiveRendererScanoutBufferDescriptor::new(
        Size {
            width: 1280,
            height: 720,
        },
        0,
        LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        17,
    );
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(&device, descriptor);

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    assert!(result.properties.is_none());
    assert!(result.resources.is_none());
    assert!(result.request.is_none());
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());

    let forged_undersized_pitch = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        size: Size {
            width: 1280,
            height: 720,
        },
        pitch: 1280 * 4 - 1,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        gem_handle: 17,
    };
    let undersized_pitch = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        forged_undersized_pitch,
    );

    assert_eq!(
        undersized_pitch.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        undersized_pitch.scanout_buffer,
        sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid
    );
    assert!(undersized_pitch.properties.is_none());
    assert!(undersized_pitch.resources.is_none());
    assert!(undersized_pitch.request.is_none());
    assert!(undersized_pitch.submit.is_none());
    assert!(undersized_pitch.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_retires_only_after_accepted_page_flip() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = result
        .submission
        .expect("submitted scanout should retain resource ownership");

    let retired = retire_native_primary_plane_scanout_after_page_flip(
        &device,
        submission,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(42),
            },
        },
    );

    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(retired.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_keeps_submission_until_page_flip_is_accepted() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = result
        .submission
        .expect("submitted scanout should retain resource ownership");

    let waiting = retire_native_primary_plane_scanout_after_page_flip(
        &device,
        submission,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        },
    );

    assert_eq!(
        waiting.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert!(waiting.destroy.is_none());
    let submission = waiting
        .submission
        .expect("rejected page flip must return the in-flight resource owner");
    assert_eq!(
        submission.retire(&device).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn live_runtime_assembly_submits_rendered_primary_plane_scanout_through_reduced_seam() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let mut submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Submitted
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Exported)
    );
    assert_eq!(
        submitted.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready)
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
    );
    assert_eq!(
        submitted.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(
        submitted.commit_flags,
        Some(LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        })
    );
    let submission = submitted
        .submission
        .take()
        .expect("rendered scanout submit should retain both owners");
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        retire_rendered_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state(),
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(
        retired.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(retired.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_reports_invalid_rendered_scanout_buffer_status() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-invalid-buffer");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter {
        status: LiveRendererScanoutBufferExportStatus::Exported,
        descriptor: Some(sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
            status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
            size: Size {
                width: -1,
                height: 720,
            },
            pitch: 1280 * 4,
            format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            gem_handle: 17,
        }),
        owner: Some(FakeRenderedScanoutOwner { raw: 9 }),
        export_attempts: 0,
    };

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed
    );
    assert_eq!(
        submitted.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid)
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable)
    );
    assert!(submitted.submission.is_none());
    assert!(submitted.cleanup.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_drops_resources_from_non_exported_rendered_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-non-exported-buffer");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter {
        status: LiveRendererScanoutBufferExportStatus::Unavailable,
        descriptor: Some(scanout_descriptor(Size {
            width: 1280,
            height: 720,
        })),
        owner: Some(FakeRenderedScanoutOwner { raw: 11 }),
        export_attempts: 0,
    };

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );
    assert_eq!(submitted.scanout_buffer, None);
    assert_eq!(submitted.submit, None);
    assert!(submitted.submission.is_none());
    assert!(submitted.cleanup.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_tracks_rendered_scanout_until_accepted_page_flip() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-tracked-retire");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle,
            in_flight: false,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(
        submitted.request_scope,
        Some(LibdrmNativeAtomicCommitRequestScope::PageFlip)
    );
    assert_eq!(submitted.in_flight, true);
    assert_eq!(submitted.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Submitted)
    );

    let blocked =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(blocked.in_flight, true);
    assert_eq!(blocked.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let aged_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should age in-flight scanout ownership");
    assert_eq!(
        aged_tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Deferred]
    );
    assert_eq!(aged_tick.rendered_primary_plane_scanout_in_flight_ticks, 1);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 1);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 1,
            threshold_ticks: 2,
        }
    );

    let stalled_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should classify old in-flight scanout ownership");
    assert_eq!(
        stalled_tick.rendered_primary_plane_scanout_in_flight_ticks,
        2
    );
    assert_eq!(
        stalled_tick.rendered_primary_plane_scanout_backpressure,
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(0),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: 0,
        }
    );

    let stale = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(54),
        },
    };
    let waiting =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &stale);

    assert_eq!(
        waiting.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state, None);
    assert_eq!(waiting.in_flight, true);
    assert_eq!(waiting.in_flight_ticks, 2);
    assert_eq!(
        waiting.reduced_log_line(),
        "sophia_runtime_rendered_scanout_retire schema=1 status=WaitingForAcceptedPageFlip destroy=none runtime_scanout_state=none in_flight=true in_flight_ticks=2 cleanup_pending=false"
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(
        retired.reduced_log_line(),
        "sophia_runtime_rendered_scanout_retire schema=1 status=RetiredAfterPageFlip destroy=Destroyed runtime_scanout_state=Retired in_flight=false in_flight_ticks=0 cleanup_pending=false"
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle,
            in_flight: false,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe retired scanout state");

    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Retired]
    );
    assert_eq!(tick.rendered_primary_plane_scanout_in_flight_ticks, 0);
    assert_eq!(tick.engine.runtime.runtime_state.scanout_retirements, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_rejects_page_flip_replay_at_submission_baseline() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-baseline-replay");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let baseline = assembly.observe_page_flip_callback(LivePageFlipCallback {
        output: OutputId::from_raw(1),
        frame_serial: 55,
    });
    assert_eq!(baseline.decision, LivePageFlipCallbackDecision::Accepted);

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let replay = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let waiting =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &replay);

    assert_eq!(
        waiting.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state, None);
    assert_eq!(waiting.in_flight, true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    let newer = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(56),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &newer);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_does_not_track_failed_rendered_scanout_submit() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-tracked-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(submitted.in_flight, false);
    assert_eq!(submitted.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe rejected scanout submit state");

    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected]
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission
    );
    assert_eq!(retired.runtime_scanout_state, None);
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.in_flight_ticks, 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_submit_failure_cleanup_for_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-cleanup-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        submit: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::PrimaryPlaneSubmitFailed
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::AtomicSubmitFailed)
    );
    assert_eq!(
        submitted.properties,
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
    );
    assert_eq!(
        submitted.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
    );
    assert_eq!(
        submitted.request,
        Some(LibdrmNativeAtomicRequestBuildStatus::Built)
    );
    assert_eq!(
        submitted.commit_submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Rejected)
    );
    assert_eq!(
        submitted.reduced_log_line(),
        "sophia_runtime_rendered_scanout_submit schema=3 status=PrimaryPlaneSubmitFailed scanout_target=Ready output_size=1280x720 target=Ready target_size=1280x720 export=Exported scanout_buffer=Ready properties=Discovered resources=Created request=Built submit=AtomicSubmitFailed request_scope=PageFlip commit_page_flip_event=true commit_nonblocking=true commit_allow_modeset=false commit_test_only=false commit_submit=Rejected runtime_scanout_state=Rejected in_flight=false in_flight_ticks=0 cleanup_pending=true"
    );
    assert_eq!(
        submitted.scanout_buffer,
        Some(sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready)
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(submitted.in_flight, false);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let mut blocked_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let blocked = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&retry_device, &mut blocked_exporter);
    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 2);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe rejected then deferred scanout states");
    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected, RuntimeScanoutState::Deferred]
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    let cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);
    assert_eq!(
        cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(cleanup.cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_failed_rendered_scanout_cleanup_for_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(submitted.in_flight, true);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.cleanup_pending, true);
    assert_eq!(
        retired.reduced_log_line(),
        "sophia_runtime_rendered_scanout_retire schema=1 status=ResourceRetireFailed destroy=FramebufferDestroyFailed runtime_scanout_state=Rejected in_flight=false in_flight_ticks=0 cleanup_pending=true"
    );
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe cleanup failure as rejected scanout state");
    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected]
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, true);

    let mut blocked_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let blocked = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&retry_device, &mut blocked_exporter);
    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(blocked.in_flight, false);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);

    assert_eq!(
        cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(
        cleanup.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert_eq!(
        cleanup.reduced_log_line(),
        "sophia_runtime_rendered_scanout_cleanup schema=1 status=CleanedUp destroy=Destroyed cleanup_pending=false"
    );
    assert_eq!(cleanup.cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let no_cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);
    assert_eq!(
        no_cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending
    );
    assert_eq!(no_cleanup.destroy, None);
    assert_eq!(
        no_cleanup.reduced_log_line(),
        "sophia_runtime_rendered_scanout_cleanup schema=1 status=NoCleanupPending destroy=none cleanup_pending=false"
    );
    assert_eq!(no_cleanup.cleanup_pending, false);

    let clean_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe cleared cleanup state");
    assert_eq!(
        clean_tick.rendered_primary_plane_scanout_cleanup_pending,
        false
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_retries_pending_rendered_scanout_cleanup_before_submit() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-auto-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);
    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &retry_device,
            &mut next_exporter,
        )
        .expect("device-backed tick should retry pending cleanup and submit next scanout");

    assert_eq!(
        tick.rendered_primary_plane_scanout_cleanup_retry
            .expect("pending cleanup should be retried")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("runtime should still submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_reports_failed_rendered_scanout_cleanup_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-auto-retry-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);
    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &failing_device,
            &mut next_exporter,
        )
        .expect("device-backed tick should report failed cleanup retry");

    assert_eq!(
        tick.rendered_primary_plane_scanout_cleanup_retry
            .expect("pending cleanup should be retried")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanupFailed
    );
    assert_eq!(
        tick.rendered_primary_plane_scanout_cleanup_retry
            .expect("pending cleanup should be retried")
            .reduced_log_line(),
        "sophia_runtime_rendered_scanout_cleanup schema=1 status=CleanupFailed destroy=FramebufferDestroyFailed cleanup_pending=true"
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, true);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("failed cleanup retry should defer the next submit")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_submits_rendered_scanout_when_runtime_requests_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-command");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should use rendered primary-plane submit");

    let submit = tick
        .rendered_primary_plane_scanout_submit
        .expect("active scanout submit should be reported");
    assert_eq!(
        submit.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submit.properties,
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
    );
    assert_eq!(
        submit.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
    );
    assert_eq!(
        submit.request,
        Some(LibdrmNativeAtomicRequestBuildStatus::Built)
    );
    assert_eq!(
        submit.commit_submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Submitted)
    );
    assert_eq!(
        submit.reduced_log_line(),
        "sophia_runtime_rendered_scanout_submit schema=3 status=SubmittedWaitingForPageFlip scanout_target=Ready output_size=1280x720 target=Ready target_size=1280x720 export=Exported scanout_buffer=Ready properties=Discovered resources=Created request=Built submit=SubmittedWaitingForPageFlip request_scope=PageFlip commit_page_flip_event=true commit_nonblocking=true commit_allow_modeset=false commit_test_only=false commit_submit=Submitted runtime_scanout_state=Submitted in_flight=true in_flight_ticks=0 cleanup_pending=false"
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_submissions, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_frame_serial,
        Some(tick.engine.tick.frame_serial)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);
    assert_eq!(tick.rendered_primary_plane_scanout_in_flight_ticks, 0);
    assert_eq!(
        tick.rendered_primary_plane_scanout_backpressure,
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 0,
            threshold_ticks: LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
        }
    );

    let deferred_tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should defer while previous submit is in flight");

    assert_eq!(
        deferred_tick
            .rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight
    );
    assert_eq!(
        deferred_tick.rendered_primary_plane_scanout_in_flight_ticks,
        1
    );
    assert_eq!(
        deferred_tick
            .rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .in_flight_ticks,
        1
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .scanout_submissions,
        1
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .scanout_rejections,
        0
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .in_flight_scanouts,
        1
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .last_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 1);
    assert_eq!(
        deferred_tick.rendered_primary_plane_scanout_backpressure,
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 1,
            threshold_ticks: LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
        }
    );

    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 99,
        })
        .expect("test channel should accept page-flip callback");
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let retire_and_submit_tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut next_exporter,
        )
        .expect("accepted page flip should retire previous submit and allow next submit");

    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_retire
            .expect("accepted page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_retire
            .expect("accepted page flip should retire in-flight scanout")
            .destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_retire
            .expect("accepted page flip should retire in-flight scanout")
            .in_flight_ticks,
        0
    );
    assert_eq!(
        retire_and_submit_tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Retired]
    );
    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next rendered scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        retire_and_submit_tick
            .engine
            .runtime
            .runtime_state
            .scanout_retirements,
        1
    );
    assert_eq!(
        retire_and_submit_tick
            .engine
            .runtime
            .runtime_state
            .scanout_submissions,
        2
    );
    assert_eq!(
        retire_and_submit_tick
            .engine
            .runtime
            .runtime_state
            .in_flight_scanouts,
        1
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        retire_and_submit_tick.rendered_primary_plane_scanout_in_flight_ticks,
        0
    );
    assert_eq!(
        retire_and_submit_tick.rendered_primary_plane_scanout_backpressure,
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 0,
            threshold_ticks: LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_reads_native_page_flip_events_before_rendered_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-native-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(33).expect("nonzero authority should mint"),
    );
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 99)]);
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let report = assembly
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut next_exporter,
            &mut reader,
            &mut poller,
            &sender,
            4,
            4,
        )
        .expect("native page-flip tick should retire and submit");

    assert_eq!(
        report.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("one callback should produce read evidence")
    );
    assert_eq!(
        report.native_page_flip.poll.status,
        LibdrmPageFlipEventPollStatus::Emitted
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("native page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        report.tick.libdrm_poller,
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::CallbackDecoded,
            route_count: 1,
            pending_callbacks: 0,
            decoded_callbacks: 1,
            rejected_callbacks: 0,
        }
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_runtime_tick_polls_libinput_shaped_input_while_retiring_and_submitting_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-input-native-page-flip-rendered-scanout");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(2);
    let poller = NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([libinput_motion_event(1, 42.0, 24.0)]),
        4,
    );
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(35).expect("nonzero authority should mint"),
    );
    let mut page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 101)]);
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let report = assembly
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut next_exporter,
            &mut reader,
            &mut page_flip_poller,
            &sender,
            4,
            4,
        )
        .expect("input, page-flip retirement, and scanout submit should share one tick");

    assert_eq!(report.tick.engine.input_poll.polled, 1);
    assert_eq!(report.tick.engine.input_poll.accepted, 1);
    assert!(report.tick.engine.input_poll.rejected.is_empty());
    assert_eq!(assembly.assembly().input().source().pending_len(), 1);
    assert_eq!(
        report.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("one callback should produce read evidence")
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("native page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_session_loop_tick_leaves_input_idle_until_reduced_readiness() {
    let root = ready_drm_sysfs_fixture("session-loop-idle-input-rendered-scanout");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(2);
    let poller = LiveInputReadinessGatedPoller::new(NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([libinput_motion_event(1, 42.0, 24.0)]),
        4,
    ));
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(36).expect("nonzero authority should mint"),
    );
    let page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut session_loop = LiveBackendSessionLoop::new(
        page_flip_poller,
        LiveBackendSessionLoopPageFlipBudget::new(4, 4),
    );
    let mut reader = FakeLibdrmNativePageFlipReader::new([]);

    let report = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::idle(),
            &device,
            &mut exporter,
            &mut reader,
            &sender,
        )
        .expect("session loop tick should keep moving when input is idle");

    assert_eq!(report.input_gate.status, LiveInputReadinessGateStatus::Idle);
    assert_eq!(report.tick.engine.input_poll.polled, 0);
    assert_eq!(assembly.assembly().input().source().pending_len(), 0);
    assert_eq!(
        assembly
            .assembly()
            .input()
            .poller()
            .inner()
            .reader()
            .queued_len(),
        1
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("idle input must not block scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        report.native_page_flip.read_loop.status,
        LibdrmNativeReadLoopStatus::Idle
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_readiness_collector_drains_reduced_readiness_without_identity() {
    let mut collector = LiveBackendReadinessCollector::new();

    assert_eq!(
        collector.snapshot(),
        LiveBackendSessionLoopReadiness::idle()
    );

    collector.observe_input_ready();
    assert_eq!(
        collector.snapshot(),
        LiveBackendSessionLoopReadiness::input_ready()
    );

    collector.observe_page_flip_ready();
    assert_eq!(
        collector.drain(),
        LiveBackendSessionLoopReadiness::all_ready()
    );
    assert_eq!(
        collector.snapshot(),
        LiveBackendSessionLoopReadiness::idle()
    );
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_session_loop_tick_skips_page_flip_read_until_reduced_ready() {
    let root = ready_drm_sysfs_fixture("session-loop-page-flip-readiness-gate");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(2);
    let poller = LiveInputReadinessGatedPoller::new(NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([libinput_motion_event(1, 42.0, 24.0)]),
        4,
    ));
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(38).expect("nonzero authority should mint"),
    );
    let page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut session_loop = LiveBackendSessionLoop::new(
        page_flip_poller,
        LiveBackendSessionLoopPageFlipBudget::new(4, 4),
    );
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 104)]);
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let report = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::input_ready(),
            &device,
            &mut next_exporter,
            &mut reader,
            &sender,
        )
        .expect("session loop tick should skip native page-flip read without readiness");

    assert_eq!(
        report.input_gate.status,
        LiveInputReadinessGateStatus::Polled
    );
    assert_eq!(
        report.native_page_flip.read_loop.status,
        LibdrmNativeReadLoopStatus::Idle
    );
    assert_eq!(reader.queued_len(), 1);
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_retire
            .map(|retire| retire.status),
        None
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("in-flight owner should defer next submit")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_session_loop_tick_observes_readiness_then_retire_and_submit() {
    let root = ready_drm_sysfs_fixture("session-loop-ready-input-page-flip-scanout");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(2);
    let poller = LiveInputReadinessGatedPoller::new(NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([libinput_motion_event(1, 42.0, 24.0)]),
        4,
    ));
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(37).expect("nonzero authority should mint"),
    );
    let page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut session_loop = LiveBackendSessionLoop::new(
        page_flip_poller,
        LiveBackendSessionLoopPageFlipBudget::new(4, 4),
    );
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 103)]);
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let report = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::all_ready(),
            &device,
            &mut next_exporter,
            &mut reader,
            &sender,
        )
        .expect("session loop tick should ingest readiness, retire, and submit");

    assert_eq!(
        report.input_gate.status,
        LiveInputReadinessGateStatus::Polled
    );
    assert_eq!(report.tick.engine.input_poll.polled, 1);
    assert_eq!(report.tick.engine.input_poll.accepted, 1);
    assert_eq!(assembly.assembly().input().source().pending_len(), 1);
    assert_eq!(
        report.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("one callback should produce read evidence")
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("native page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "libinput-events")]
#[test]
fn live_session_loop_tick_drains_pending_page_flip_callbacks_without_new_readiness() {
    let root = ready_drm_sysfs_fixture("session-loop-pending-page-flip-without-readiness");
    let config = LiveBackendConfig::new(&root).with_input_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let report = discover_live_backend(&config);
    let (sender, receiver) = mpsc::sync_channel(4);
    let poller = LiveInputReadinessGatedPoller::new(NativeLibinputEventPoller::new(
        FakeLiveLibinputEventReader::new([]),
        4,
    ));
    let mut assembly = report
        .into_live_runtime_assembly(poller)
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let device = full_primary_plane_scanout_device();
    let mut initial_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&device, &mut initial_exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(39).expect("nonzero authority should mint"),
    );
    let page_flip_poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut session_loop = LiveBackendSessionLoop::new(
        page_flip_poller,
        LiveBackendSessionLoopPageFlipBudget::new(4, 1),
    );
    let mut reader = FakeLibdrmNativePageFlipReader::new([
        LibdrmNativePageFlipCallback::new(slot, 201),
        LibdrmNativePageFlipCallback::new(slot, 202),
    ]);
    let mut first_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let first = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::page_flip_ready(),
            &device,
            &mut first_exporter,
            &mut reader,
            &sender,
        )
        .expect("first tick should read native page flips and emit one callback");

    assert_eq!(
        first.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(2)
            .expect("two native callbacks should decode")
    );
    assert_eq!(
        first.native_page_flip.poll.status,
        LibdrmPageFlipEventPollStatus::EmitLimitReached
    );
    assert_eq!(first.native_page_flip.poll.callbacks.emitted, 1);
    assert_eq!(first.native_page_flip.poll.callbacks.queued_remaining, 1);
    assert_eq!(reader.queued_len(), 0);
    assert_eq!(session_loop.page_flip_poller().pending_callback_count(), 1);
    assert_eq!(
        first
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("first emitted callback should retire initial scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        first
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let mut second_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let second = session_loop
        .run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            &mut assembly,
            CompositorBackendTickInput::default(),
            LiveBackendSessionLoopReadiness::idle(),
            &device,
            &mut second_exporter,
            &mut reader,
            &sender,
        )
        .expect("pending decoded callback should drain without fresh readiness");

    assert_eq!(
        second.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("pending callback emission should be reduced as decoded")
    );
    assert_eq!(
        second.native_page_flip.poll.status,
        LibdrmPageFlipEventPollStatus::Emitted
    );
    assert_eq!(second.native_page_flip.poll.callbacks.emitted, 1);
    assert_eq!(second.native_page_flip.poll.callbacks.queued_remaining, 0);
    assert_eq!(reader.queued_len(), 0);
    assert_eq!(session_loop.page_flip_poller().pending_callback_count(), 0);
    assert_eq!(
        second
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("pending callback should retire the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        second
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should continue with the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_rejects_rendered_scanout_when_kms_target_is_not_ready() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-kms-not-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 640,
        height: 480,
    });
    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should fail closed before export");
    let submit = tick
        .rendered_primary_plane_scanout_submit
        .expect("active scanout submit should be reported");

    assert_eq!(
        submit.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submit.scanout_target,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );
    assert_eq!(submit.export, None);
    assert_eq!(submit.submit, None);
    assert_eq!(submit.in_flight, false);
    assert_eq!(exporter.export_attempts(), 0);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_direct_rendered_scanout_submit_rejects_not_ready_kms_target() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-direct-kms-not-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 640,
        height: 480,
    });

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 640,
        height: 480,
    });

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submitted.scanout_target,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );
    assert_eq!(submitted.export, None);
    assert_eq!(submitted.submit, None);
    assert!(submitted.submission.is_none());
    assert_eq!(exporter.export_attempts(), 0);
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Rejected
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_direct_rendered_scanout_submit_rechecks_native_kms_snapshot_before_export() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-native-kms-drift");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = FakeNativePrimaryPlaneScanoutDevice {
        selection: FakeNativeKmsSelectionDevice {
            connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
                false,
                Some(encoder_handle()),
                [encoder_handle()],
                Some(Size {
                    width: 1280,
                    height: 720,
                }),
            )),
            ..full_kms_selection_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::Ready
    );

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submitted.scanout_target,
        LiveKmsScanoutTargetStatus::OutputUnavailable
    );
    assert_eq!(submitted.export, None);
    assert_eq!(submitted.submit, None);
    assert_eq!(exporter.export_attempts(), 0);
    assert!(submitted.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_defers_rendered_scanout_when_previous_submit_is_in_flight() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-command-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should fail closed through reduced state");

    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[test]
fn live_runtime_tick_native_gbm_rendered_scanout_fails_closed_when_render_device_is_unavailable() {
    let root = ready_drm_sysfs_fixture("runtime-native-gbm-rendered-primary-plane-unavailable");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);

    let tick = assembly
        .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("native GBM rendered scanout path should fail closed through runtime state");

    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(exporter.export_attempts(), 1);
    assert_eq!(exporter.context_open_attempts(), 1);
    assert_eq!(
        exporter.context_status(),
        Some(NativeGbmRenderedScanoutContextStatus::Unavailable)
    );
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );

    let second_tick = assembly
        .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("reusable native GBM exporter should survive another runtime tick");

    assert_eq!(
        second_tick
            .rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(exporter.export_attempts(), 2);
    assert_eq!(exporter.context_open_attempts(), 2);
    assert_eq!(
        exporter.context_status(),
        Some(NativeGbmRenderedScanoutContextStatus::Unavailable)
    );
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[test]
fn live_runtime_tick_native_gbm_rendered_scanout_reads_native_page_flips_before_persistent_export()
{
    let root = ready_drm_sysfs_fixture("runtime-native-gbm-rendered-scanout-native-page-flip");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut initial_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&device, &mut initial_exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(
        LibdrmBackendFdAuthority::new(34).expect("nonzero authority should mint"),
    );
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 100)]);
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);

    let report = assembly
        .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
            &mut reader,
            &mut poller,
            &sender,
            4,
            4,
        )
        .expect("native page-flip intake should run before persistent GBM export");

    assert_eq!(
        report.native_page_flip.read_loop,
        LibdrmNativeReadLoopReport::callback_decoded(1)
            .expect("one callback should produce read evidence")
    );
    assert_eq!(
        report.native_page_flip.poll.status,
        LibdrmPageFlipEventPollStatus::Emitted
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_retire
            .expect("native page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        report
            .tick
            .rendered_primary_plane_scanout_submit
            .expect("persistent native GBM export should be attempted")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        report.tick.libdrm_poller,
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::CallbackDecoded,
            route_count: 1,
            pending_callbacks: 0,
            decoded_callbacks: 1,
            rejected_callbacks: 0,
        }
    );
    assert_eq!(
        report.tick.engine.runtime.runtime_state.scanout_retirements,
        1
    );
    assert_eq!(
        report.tick.engine.runtime.runtime_state.scanout_rejections,
        1
    );
    assert_eq!(
        report.tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(exporter.export_attempts(), 1);
    assert_eq!(exporter.context_open_attempts(), 1);
    assert_eq!(
        exporter.context_status(),
        Some(NativeGbmRenderedScanoutContextStatus::Unavailable)
    );
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[test]
fn native_gbm_rendered_scanout_exporter_rejects_invalid_target_before_device_open() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 0,
        height: 720,
    });

    let export = exporter.export_rendered_scanout_buffer(target);

    assert_eq!(
        export.status,
        LiveRendererScanoutBufferExportStatus::InvalidTarget
    );
    assert_eq!(exporter.export_attempts(), 1);
    assert_eq!(exporter.context_open_attempts(), 0);
    assert_eq!(exporter.context_status(), None);
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::InvalidTarget)
    );
    assert_eq!(exporter.last_target(), Some(target));
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Invalidated,
            target,
        })
    );
}

#[cfg(feature = "gbm-probe")]
#[test]
fn native_gbm_rendered_scanout_exporter_rejects_forged_ready_target_before_device_open() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    let target = LiveGbmEglFrameTargetRecord {
        status: LiveGbmEglFrameTargetStatus::Ready,
        size: Size {
            width: -1,
            height: 720,
        },
    };

    let export = exporter.export_rendered_scanout_buffer(target);

    assert_eq!(
        export.status,
        LiveRendererScanoutBufferExportStatus::InvalidTarget
    );
    assert_eq!(exporter.export_attempts(), 1);
    assert_eq!(exporter.context_open_attempts(), 0);
    assert_eq!(exporter.context_status(), None);
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::InvalidTarget)
    );
    assert_eq!(exporter.last_target(), Some(target));
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target,
        })
    );
}

#[cfg(feature = "gbm-probe")]
#[test]
fn native_gbm_rendered_scanout_exporter_tracks_reduced_target_reuse_and_resize() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    let first = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1280,
        height: 720,
    });
    let resized = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1920,
        height: 1080,
    });

    let first_export = exporter.export_rendered_scanout_buffer(first);
    assert_eq!(
        first_export.status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target: first,
        })
    );

    let retained_export = exporter.export_rendered_scanout_buffer(first);
    assert_eq!(
        retained_export.status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retained,
            target: first,
        })
    );

    let resized_export = exporter.export_rendered_scanout_buffer(resized);
    assert_eq!(
        resized_export.status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Resized,
            target: resized,
        })
    );
    assert_eq!(exporter.last_target(), Some(resized));
    assert_eq!(exporter.export_attempts(), 3);
    assert_eq!(exporter.context_open_attempts(), 3);
}

#[test]
fn live_runtime_assembly_keeps_rendered_scanout_owner_until_page_flip_is_accepted() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-wait");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let mut submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);
    let submission = submitted
        .submission
        .take()
        .expect("rendered scanout submit should retain both owners");
    let rejected = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(54),
        },
    };

    let waiting =
        retire_rendered_primary_plane_scanout_after_page_flip(&device, submission, &rejected);

    assert_eq!(
        waiting.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state(), None);
    assert!(waiting.destroy.is_none());
    let owner = waiting
        .submission
        .expect("waiting retirement must keep rendered scanout owner")
        .into_scanout_buffer();
    assert_eq!(owner, FakeRenderedScanoutOwner { raw: 7 });

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_fails_rendered_scanout_submit_before_kms_on_export_failure() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-export-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Rejected
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );
    assert!(submitted.submit.is_none());
    assert!(submitted.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[path = "libdrm_events_feature/atomic_scanout_evidence.rs"]
mod atomic_scanout_evidence;

#[test]
fn native_libdrm_primary_plane_resources_validate_size_and_lifetime() {
    let oversized_size = Size {
        width: 65_536,
        height: 720,
    };
    let oversized_selected =
        select_native_primary_plane_target(&kms_selection_device_with_mode_size(oversized_size))
            .selection
            .expect("oversized fake KMS target should still select before resource validation");
    let oversized_buffer = FakeDrmBuffer::xrgb8888(oversized_size);

    let oversized_modeset = create_native_primary_plane_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        },
        oversized_selected,
        &oversized_buffer,
    );
    assert_eq!(
        oversized_modeset.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize
    );
    assert!(oversized_modeset.resources.is_none());
    assert!(oversized_modeset.cleanup.is_none());

    let oversized_page_flip = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Ok(15),
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        oversized_selected,
        &oversized_buffer,
    );
    assert_eq!(
        oversized_page_flip.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidSelectionSize
    );
    assert!(oversized_page_flip.resources.is_none());
    assert!(oversized_page_flip.cleanup.is_none());

    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let mismatched = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(Size {
            width: 1920,
            height: 1080,
        }),
    );
    assert_eq!(
        mismatched.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch
    );
    assert!(mismatched.resources.is_none());

    let invalid_pitch = create_native_primary_plane_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size()).with_pitch(1280 * 4 - 1),
    );
    assert_eq!(
        invalid_pitch.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer
    );
    assert!(invalid_pitch.resources.is_none());
    assert!(invalid_pitch.cleanup.is_none());

    let invalid_format = create_native_primary_plane_page_flip_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Ok(15),
            framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &FakeDrmBuffer::xrgb8888(selected.size()).with_format(drm::buffer::DrmFourcc::Argb8888),
    );
    assert_eq!(
        invalid_format.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::InvalidBuffer
    );
    assert!(invalid_format.resources.is_none());
    assert!(invalid_format.cleanup.is_none());

    let zero_mode_blob = create_native_primary_plane_resources(
        &FakeNativePrimaryPlaneResourceDevice {
            mode_blob: Ok(0),
            framebuffer: Ok(framebuffer_handle()),
            destroy_framebuffer: Ok(()),
            destroy_mode_blob: Ok(()),
        },
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        zero_mode_blob.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );
    assert!(zero_mode_blob.resources.is_none());
    assert!(zero_mode_blob.cleanup.is_none());

    let created = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let destroyed = destroy_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        created
            .resources
            .expect("created resources should be destroyable"),
    );
    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_page_flip_resources_do_not_require_mode_blob() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let mode_unavailable = FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        destroy_mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };

    let modeset = create_native_primary_plane_resources(
        &mode_unavailable,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        modeset.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );

    let page_flip = create_native_primary_plane_page_flip_resources(
        &mode_unavailable,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        page_flip.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let destroyed = destroy_native_primary_plane_resources(
        &mode_unavailable,
        page_flip
            .resources
            .expect("page-flip resources should carry only a framebuffer"),
    );
    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_renderer_scanout_buffer_rejects_invalid_renderer_descriptors() {
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1280,
        height: 720,
    });
    let mut invalid_exporter =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(0, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, 17);
    let invalid_descriptor = invalid_exporter.export_scanout_buffer(target).descriptor;
    assert!(invalid_descriptor.is_none());

    let mut unsupported_format =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(1280 * 4, 0, 17);
    assert!(
        unsupported_format
            .export_scanout_buffer(target)
            .descriptor
            .and_then(LibdrmRendererScanoutBuffer::from_descriptor)
            .is_none()
    );

    let forged_ready = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        size: Size {
            width: -1,
            height: 720,
        },
        pitch: 1280 * 4,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        gem_handle: 17,
    };
    assert!(LibdrmRendererScanoutBuffer::from_descriptor(forged_ready).is_none());

    let forged_undersized_pitch = sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
        status: sophia_renderer_live::LiveRendererScanoutBufferStatus::Ready,
        size: Size {
            width: 1280,
            height: 720,
        },
        pitch: 1280 * 4 - 1,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        gem_handle: 17,
    };
    assert!(LibdrmRendererScanoutBuffer::from_descriptor(forged_undersized_pitch).is_none());

    let submit = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        &full_primary_plane_scanout_device(),
        select_native_primary_plane_target(&full_kms_selection_device()),
        forged_ready,
    );
    assert_eq!(
        submit.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        submit.scanout_buffer,
        sophia_renderer_live::LiveRendererScanoutBufferStatus::Invalid
    );
}

#[test]
fn native_libdrm_primary_plane_resource_creation_fails_closed() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");

    let mode_failed = FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &mode_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );
    assert!(created.resources.is_none());

    let mode_missing = FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Err(io::Error::from(io::ErrorKind::InvalidInput)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &mode_missing,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode
    );
    assert!(created.resources.is_none());

    let framebuffer_failed = FakeNativePrimaryPlaneResourceDevice {
        framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &framebuffer_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert!(created.resources.is_none());
    assert!(created.cleanup.is_none());

    let framebuffer_failed_and_cleanup_failed = FakeNativePrimaryPlaneResourceDevice {
        framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        destroy_mode_blob: Err(io::Error::other("test mode blob destroy failed")),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &framebuffer_failed_and_cleanup_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert!(created.resources.is_none());
    let cleanup = created
        .cleanup
        .expect("failed framebuffer registration must retain failed mode blob cleanup");
    assert_eq!(
        cleanup.retry(&full_primary_plane_resource_device()).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_selection_reduces_missing_resource_groups() {
    let disconnected = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            false,
            Some(encoder_handle()),
            [encoder_handle()],
            Some(Size {
                width: 1280,
                height: 720,
            }),
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&disconnected).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
    );

    let modeless = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            Some(encoder_handle()),
            [encoder_handle()],
            None,
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&modeless).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoUsableMode
    );

    let no_encoder = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            None,
            [],
            Some(Size {
                width: 1280,
                height: 720,
            }),
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&no_encoder).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoUsableEncoder
    );

    let incompatible_crtc = FakeNativeKmsSelectionDevice {
        encoder_snapshot: Ok(LibdrmNativeEncoderSnapshot::new(None, [])),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&incompatible_crtc).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatibleCrtc
    );

    let no_primary_plane = FakeNativeKmsSelectionDevice {
        plane_type: Ok(Some(drm::control::PlaneType::Overlay)),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&no_primary_plane).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
    );
}

#[test]
fn native_libdrm_primary_plane_selection_fails_closed_on_read_error() {
    let read_failed = FakeNativeKmsSelectionDevice {
        connectors: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_kms_selection_device()
    };
    let selection = select_native_primary_plane_target(&read_failed);

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed
    );
    assert!(selection.selection.is_none());

    let plane_read_failed = FakeNativeKmsSelectionDevice {
        plane_snapshot: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_kms_selection_device()
    };
    let selection = select_native_primary_plane_target(&plane_read_failed);

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed
    );
    assert!(selection.selection.is_none());
}

#[test]
fn native_libdrm_primary_plane_builder_creates_submit_ready_request() {
    let build = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    let request = build.request.expect("valid objects should build request");
    assert_eq!(
        request.reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::Modeset
    );
    assert_eq!(
        request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );

    let mut committer =
        NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice { result: Ok(()) });
    assert_eq!(
        committer.submit_native_atomic_commit(request),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        }
    );
}

#[test]
fn native_libdrm_primary_plane_page_flip_builder_creates_plane_only_request() {
    let build = build_native_primary_plane_page_flip_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    let request = build
        .request
        .expect("valid objects should build page-flip request");
    assert_eq!(
        request.reduced_scope(),
        LibdrmNativeAtomicCommitRequestScope::PageFlip
    );
    assert_eq!(
        request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );
}

#[test]
fn native_libdrm_primary_plane_modeset_builder_requires_mode_blob() {
    let objects = LibdrmNativePrimaryPlaneObjects::new_with_optional_mode_blob(
        connector_handle(),
        crtc_handle(),
        plane_handle(),
        framebuffer_handle(),
        None,
        Size {
            width: 1280,
            height: 720,
        },
    );

    let modeset = build_native_primary_plane_atomic_request(objects, primary_plane_properties());
    assert_eq!(
        modeset.status,
        LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob
    );
    assert!(modeset.request.is_none());

    let zero_mode_blob_objects = LibdrmNativePrimaryPlaneObjects::new_with_optional_mode_blob(
        connector_handle(),
        crtc_handle(),
        plane_handle(),
        framebuffer_handle(),
        Some(0),
        Size {
            width: 1280,
            height: 720,
        },
    );
    let zero_mode_blob = build_native_primary_plane_atomic_request(
        zero_mode_blob_objects,
        primary_plane_properties(),
    );
    assert_eq!(
        zero_mode_blob.status,
        LibdrmNativeAtomicRequestBuildStatus::MissingModeBlob
    );
    assert!(zero_mode_blob.request.is_none());

    let page_flip =
        build_native_primary_plane_page_flip_atomic_request(objects, primary_plane_properties());
    assert_eq!(
        page_flip.status,
        LibdrmNativeAtomicRequestBuildStatus::Built
    );
    assert!(page_flip.request.is_some());
}

#[test]
fn native_libdrm_primary_plane_builder_rejects_invalid_size() {
    let zero_width = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 0,
            height: 720,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        zero_width.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(zero_width.request.is_none());

    let negative_height = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: -1,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        negative_height.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(negative_height.request.is_none());

    let oversized_width = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 65_536,
            height: 720,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        oversized_width.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(oversized_width.request.is_none());

    let oversized_height = build_native_primary_plane_page_flip_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 65_536,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        oversized_height.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(oversized_height.request.is_none());
}

#[test]
fn native_libdrm_atomic_committer_reduces_submit_results() {
    let mut committer =
        NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice { result: Ok(()) });
    assert_eq!(
        committer.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        }
    );
    assert_eq!(committer.submitted_count(), 1);
    assert_eq!(committer.rejected_count(), 0);

    let mut would_block = NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice {
        result: Err(io::Error::from(io::ErrorKind::WouldBlock)),
    });
    assert_eq!(
        would_block.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::WouldBlock,
        }
    );
    assert_eq!(would_block.submitted_count(), 0);
    assert_eq!(would_block.rejected_count(), 0);

    let mut rejected = NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice {
        result: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
    });
    assert_eq!(
        rejected.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Rejected,
        }
    );
    assert_eq!(rejected.submitted_count(), 0);
    assert_eq!(rejected.rejected_count(), 1);
}

#[path = "libdrm_events_feature/native_page_flip.rs"]
mod native_page_flip;

fn ready_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let connector = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1280x720\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    root
}

fn multi_output_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let first = root.join("card0-DP-1");
    let second = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    write_fixture_file(&first, "status", "connected\n");
    write_fixture_file(&first, "modes", "1920x1080\n");
    write_fixture_file(&first, "connector_id", "1234\n");
    write_fixture_file(&first, "crtc_id", "2234\n");
    write_fixture_file(&second, "status", "connected\n");
    write_fixture_file(&second, "modes", "2560x1440\n");
    write_fixture_file(&second, "connector_id", "9876\n");
    write_fixture_file(&second, "crtc_id", "8876\n");
    root
}

fn write_fixture_file(root: &std::path::Path, name: &str, contents: &str) {
    std::fs::write(root.join(name), contents).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[path = "libdrm_events_feature/atomic_scanout_hardware_smoke.rs"]
mod atomic_scanout_hardware_smoke;

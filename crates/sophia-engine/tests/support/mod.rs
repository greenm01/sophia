#![allow(dead_code, unused_imports)]

pub use sophia_engine::{
    AuthorityTransactionInbox, AuthorityTransactionIntake, BufferImportPath, ChromeActionDecision,
    ChromeActionRejectReason, ChromeBroker, CompositorBackendTickInput, DeterministicFrameClock,
    DrmKmsMode, DrmKmsOutputDescriptor, DrmKmsOutputRegistry, EngineError, FrameClock,
    FramePlanRequest, FrameScheduleDecision, HeadlessCompositorBackendAssembly, HeadlessEngine,
    HeadlessOutput, HeadlessRuntimeAdapter, HeadlessSessionDriver, HeadlessSessionDriverTick,
    ImportCapableRenderer, ImportedBufferHandle, LastCommittedLayout, LayoutEpochState,
    LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventIngest, LibinputEventSource,
    LibinputPhysicalInputAdapter, LiveBrokerRuntimeAdapter, LiveChromeRuntimeAdapter,
    LivePortalRuntimeAdapter, LiveRendererRuntimeAdapter, LiveRuntimeDriverAdapter,
    LiveRuntimeDriverIntake, LiveWmRuntimeAdapter, LiveXRuntimeAdapter, MetadataChromeRejectReason,
    MetadataChromeUpdate, NotificationChromePresenter, NotificationChromeRejectReason,
    NotificationChromeUpdate, PageFlipCommitGate, PageFlipCommitOutcome, QueuedInputPoller,
    RendererSelection, RoutedInputCoalescer, RoutedInputFlushReason, RoutedInputQueueAction,
    RoutedInputRequestError, SanitizedChromeMetadata, SessionCommand, SessionEvent,
    SessionLayerSource, SessionTickRequest, SlowClientVisualDecision, SurfaceTimeoutPolicy,
    SurfaceTransactionCommitReadiness, SurfaceVisualStateTable, WmIpcError, WmRestartReason,
    WmRuntimeAction, WmTransactionUpdate, discover_drm_kms_outputs_from_sysfs,
    explicit_sync_surfaces, hit_test_scene_for_input, layout_epoch_for_explicit_sync,
    measure_resize_behavior, notification_chrome_command_from_portal, request_wm_over_stream,
    routed_input_request_from_physical_event, routed_input_requests_from_flush,
    runtime_observation_from_authority_transaction_commit,
    runtime_observation_from_metadata_chrome_updates,
    runtime_observation_from_notification_chrome_updates, runtime_observation_from_portal_commands,
    runtime_observation_from_render_frame_report, runtime_observation_from_session_tick_report,
    runtime_observation_from_slow_client_visual_decisions,
    runtime_observation_from_wm_transaction_update, schedule_frame_from_damage,
    surface_transaction_readiness_for_epoch, update_wm_supervisor_from_runtime_action,
};
pub use sophia_portal::{NotificationRequest, NotificationUrgency, PortalCommand};
pub use sophia_protocol::{
    AttentionState, AuthorityKind, BrokerHealthPacket, BrokerHealthState, BrokerKind, BufferSource,
    ChromeActionKind, ChromeActionRequest, ChromeDescriptor, CommittedSurfaceState, DamageFrame,
    DeviceId, DisplayLabel, IconTokenId, InputEventKind, InputEventPacket, InputRoute,
    InputRouteOutcome, IpcCodecError, LayerSnapshot, LayoutNodeCapabilities, LayoutNodeKind,
    LayoutNodeSnapshot, LayoutNodeState, LayoutTransaction, NamespaceId, OutputId, Point,
    PortalTransferId, Rect, Region, ResizeSyncCapability, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAGIC,
    SOPHIA_IPC_MAX_PAYLOAD_LEN, SOPHIA_IPC_VERSION, SeatId, Size, SurfaceConstraints, SurfaceId,
    SurfacePlacement, SurfaceTransaction, SurfaceTransactionReadiness, TransactionCommit,
    TransactionId, TransactionOutcome, Transform, TrustLevel, WmCommand, WmRequestKind,
    WmRequestPacket, WmResponsePacket, WorkspaceId, XWindowId, decode_wm_request_frame,
    encode_wm_response_frame,
};
pub use sophia_runtime::{
    RestartPolicy, SessionRuntimeCommand, SessionRuntimeObservation, SessionRuntimePhase,
    SupervisedProcessKind, SupervisorCommand, SupervisorState,
};
pub use std::fs;
pub use std::io::{Cursor, Read, Result as IoResult, Write};
pub use std::path::{Path, PathBuf};
pub use std::time::Duration;

pub fn test_layer(surface_index: u32, stack_rank: u32, x: i32, damage: Region) -> LayerSnapshot {
    LayerSnapshot {
        surface: SurfaceId::new(surface_index, 1),
        window: None,
        namespace: None,
        stack_rank,
        geometry: Rect {
            x,
            y: 0,
            width: 100,
            height: 100,
        },
        source: BufferSource::CpuBuffer {
            handle: u64::from(surface_index) + 1,
        },
        damage,
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

pub fn motion_event(serial: u64, x: f64, y: f64) -> InputEventPacket {
    input_event(serial, InputEventKind::PointerMotion, x, y)
}

pub fn input_event(serial: u64, kind: InputEventKind, x: f64, y: f64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: serial * 10,
        kind,
        global_position: Some(Point { x, y }),
        target_surface: Some(SurfaceId::new(1, 1)),
        target_window: Some(XWindowId::new(0x30, 1)),
        local_position: Some(Point { x, y }),
    }
}

pub fn route(serial: u64, target_window: u32, x: f64, y: f64) -> InputRoute {
    InputRoute {
        input_serial: serial,
        target_surface: Some(SurfaceId::new(1, 1)),
        target_window: Some(XWindowId::new(target_window, 1)),
        global_position: Point { x, y },
        local_position: Some(Point { x, y }),
        transform: Transform::IDENTITY,
        outcome: InputRouteOutcome::Routed,
    }
}

pub fn scale_translate_transform(scale: f32, x: f32, y: f32) -> Transform {
    Transform {
        matrix: [
            scale, 0.0, x, //
            0.0, scale, y, //
            0.0, 0.0, 1.0,
        ],
    }
}

pub fn frame_tick(frame_serial: u64) -> sophia_engine::FrameClockTick {
    sophia_engine::FrameClockTick {
        output: OutputId::from_raw(1),
        frame_serial,
        target_msec: frame_serial * 16,
    }
}

pub fn damage_frame(frame_serial: u64, affected_surfaces: &[SurfaceId]) -> DamageFrame {
    DamageFrame {
        output: OutputId::from_raw(1),
        frame_serial,
        buffer_age: 1,
        root_generation: frame_serial,
        affected_surfaces: affected_surfaces.to_vec(),
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        }),
    }
}

pub fn wm_request(transaction: TransactionId) -> WmRequestPacket {
    WmRequestPacket {
        transaction,
        kind: WmRequestKind::SurfaceRemoved {
            surface: SurfaceId::new(1, 1),
            workspace: WorkspaceId::from_raw(1),
        },
    }
}

pub fn metadata(surface: SurfaceId, label: &str, generation: u64) -> SanitizedChromeMetadata {
    SanitizedChromeMetadata {
        surface,
        label: Some(label.to_owned()),
        label_redacted: true,
        icon: None,
        trust_level: TrustLevel::Unknown,
        attention: AttentionState::None,
        generation,
    }
}

pub fn notification_request(raw_transfer: u64) -> NotificationRequest {
    NotificationRequest {
        transfer: PortalTransferId::from_raw(raw_transfer),
        source_namespace: NamespaceId::from_raw(1),
        target_namespace: NamespaceId::from_raw(2),
        summary: "Build finished".to_owned(),
        body: Some("Sophia smoke completed".to_owned()),
        urgency: NotificationUrgency::Normal,
        actions: vec!["Open log".to_owned()],
        generation: 7,
    }
}

pub struct TestDuplex {
    read: Cursor<Vec<u8>>,
    pub written: Vec<u8>,
}

impl TestDuplex {
    pub fn new(read: Vec<u8>) -> Self {
        Self {
            read: Cursor::new(read),
            written: Vec::new(),
        }
    }
}

impl Read for TestDuplex {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.read.read(buf)
    }
}

impl Write for TestDuplex {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

pub fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub fn layout_node(surface: SurfaceId, generation: u64, closable: bool) -> LayoutNodeSnapshot {
    let mut capabilities = LayoutNodeCapabilities::STANDARD_TOPLEVEL;
    capabilities.closable = closable;

    LayoutNodeSnapshot {
        surface,
        workspace: WorkspaceId::from_raw(1),
        kind: LayoutNodeKind::Toplevel,
        capabilities,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        generation,
    }
}

pub fn drm_sysfs_fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-drm-sysfs-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

pub fn write_fixture_file(root: &Path, name: &str, contents: &str) {
    fs::write(root.join(name), contents).unwrap();
}

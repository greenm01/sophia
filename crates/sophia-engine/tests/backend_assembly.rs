mod support;
use support::*;

#[test]
fn headless_backend_assembly_drains_input_commits_authority_and_renders_cpu_frame() {
    let output = HeadlessOutput {
        id: OutputId::from_raw(11),
        size: Size {
            width: 1024,
            height: 768,
        },
        scale: 1,
    };
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let input = LibinputPhysicalInputAdapter::new(
        QueuedInputPoller::new(vec![motion_event(1, 10.0, 20.0)]),
        source,
    );
    let mut outputs = DrmKmsOutputRegistry::new();
    let descriptor = DrmKmsOutputDescriptor {
        output: output.id,
        connector_id: 11,
        crtc_id: 0,
        mode: DrmKmsMode {
            size: output.size,
            refresh_millihz: 60_000,
        },
        scale: output.scale,
    };
    outputs.upsert(descriptor);
    let mut assembly = HeadlessCompositorBackendAssembly::from_parts(
        output,
        outputs,
        DeterministicFrameClock::default(),
        input,
        RendererSelection::CpuFallback,
    );
    let mut template = test_layer(3, 0, 0, Region::empty());
    template.surface = SurfaceId::new(3, 1);
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(90),
        authority: AuthorityKind::SophiaX,
        surface: template.surface,
        namespace: Some(NamespaceId::from_raw(4)),
        target_geometry: Rect {
            x: 25,
            y: 30,
            width: 160,
            height: 90,
        },
        target_buffer: BufferSource::CpuBuffer { handle: 900 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 160,
            height: 90,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 0,
    };

    let report = assembly
        .run_tick(CompositorBackendTickInput {
            x_event_count: 1,
            authority_commits: Vec::new(),
            authority_batches: vec![AuthorityTransactionIntake::new(
                TransactionId::from_raw(90),
                vec![transaction],
            )],
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 0,
            layer_templates: vec![template],
            scanout_submit_state: None,
            scanout_lifecycle_states: Vec::new(),
        })
        .expect("deterministic backend tick should complete");

    assert_eq!(assembly.outputs().primary_engine_output(), Some(output));
    assert_eq!(report.tick.output, output.id);
    assert_eq!(report.tick.frame_serial, 1);
    assert_eq!(report.input_poll.polled, 1);
    assert_eq!(report.input_poll.accepted, 1);
    assert!(report.input_poll.rejected.is_empty());
    assert_eq!(report.physical_input.poll, report.input_poll);
    assert_eq!(report.physical_input.pending_events, 1);
    assert_eq!(
        report.physical_input.routing_stage,
        PhysicalInputRoutingStage::PhysicalIntakeOnly
    );
    assert_eq!(assembly.input().source().pending_len(), 1);
    assert_eq!(assembly.committed_surfaces().len(), 1);
    assert_eq!(assembly.committed_surfaces()[0].geometry.x, 25);
    assert_eq!(
        assembly.committed_surfaces()[0].buffer,
        BufferSource::CpuBuffer { handle: 900 }
    );
    assert_eq!(
        report
            .runtime
            .runtime_state
            .authority_transactions_committed,
        1
    );
    assert_eq!(report.runtime.runtime_state.authority_surfaces_applied, 1);

    let session_tick = report.runtime.session_tick.as_ref().unwrap();
    assert_eq!(session_tick.frame.output, output.id);
    assert_eq!(session_tick.frame.frame_serial, 1);
    assert_eq!(session_tick.frame.layers[0].geometry.x, 25);
    assert_eq!(
        session_tick.frame.layers[0].source,
        BufferSource::CpuBuffer { handle: 900 }
    );

    let render = report.render.unwrap();
    assert_eq!(render.imports.len(), 1);
    assert_eq!(render.imports[0].requested, BufferImportPath::CpuReadback);
    assert_eq!(render.imports[0].used, BufferImportPath::CpuReadback);
    assert!(!render.imports[0].used_fallback);
}

#[test]
fn headless_backend_assembly_accepts_an_authoritative_committed_snapshot() {
    let output = HeadlessOutput::deterministic();
    let mut assembly = HeadlessCompositorBackendAssembly::new(output);
    let snapshot = CommittedSurfaceState {
        surface: SurfaceId::new(71, 1),
        committed_generation: 9,
        geometry: Rect {
            x: 12,
            y: 24,
            width: 320,
            height: 200,
        },
        buffer: BufferSource::CpuBuffer { handle: 710 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        }),
    };

    assembly.replace_committed_surfaces(vec![snapshot.clone()]);

    assert_eq!(assembly.committed_surfaces(), [snapshot]);
}

#[test]
fn headless_backend_tick_keeps_physical_input_separate_from_routed_input() {
    let output = HeadlessOutput::deterministic();
    let mut source = LibinputEventSource::new();
    source.register_device(LibinputDeviceDescriptor {
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        kind: LibinputDeviceKind::Pointer,
    });
    let event = motion_event(1, 10.0, 20.0);
    let input = LibinputPhysicalInputAdapter::new(QueuedInputPoller::new(vec![event]), source);
    let mut assembly = HeadlessCompositorBackendAssembly::from_parts(
        output,
        DrmKmsOutputRegistry::new(),
        DeterministicFrameClock::default(),
        input,
        RendererSelection::CpuFallback,
    );

    let report = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("deterministic backend tick should complete");

    assert_eq!(
        report.physical_input,
        PhysicalInputIntakeReport {
            poll: LibinputPollReport {
                polled: 1,
                accepted: 1,
                rejected: Vec::new(),
            },
            pending_events: 1,
            routing_stage: PhysicalInputRoutingStage::PhysicalIntakeOnly,
        }
    );
    assert_eq!(assembly.input().source().pending_len(), 1);

    let event = assembly
        .input_mut()
        .source_mut()
        .drain_events()
        .pop()
        .expect("physical event should remain queued for routing layer");
    let route = hit_test_scene_for_input(&event, &[]);
    assert_eq!(route.outcome, InputRouteOutcome::NoTarget);
    assert_eq!(
        routed_input_request_from_physical_event(&event, &route),
        Err(RoutedInputRequestError::RouteNotAccepted)
    );
}

#[test]
fn renderer_selection_uses_xpixmap_imports_and_falls_back_for_unsupported_paths() {
    let output = HeadlessOutput::deterministic();
    let mut assembly = HeadlessCompositorBackendAssembly::from_parts(
        output,
        DrmKmsOutputRegistry::new(),
        DeterministicFrameClock::default(),
        LibinputPhysicalInputAdapter::new(QueuedInputPoller::default(), LibinputEventSource::new()),
        RendererSelection::ImportCapable {
            import_xpixmap: true,
            import_dmabuf: false,
        },
    );
    let mut xpixmap_template = test_layer(4, 0, 0, Region::empty());
    xpixmap_template.source = BufferSource::XPixmap { pixmap: 44 };
    let mut dmabuf_template = test_layer(5, 1, 100, Region::empty());
    dmabuf_template.source = BufferSource::DmaBuf { handle: 99 };
    let transaction = TransactionId::from_raw(91);
    let xpixmap_transaction = xpixmap_template.to_surface_transaction(
        transaction,
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );
    let dmabuf_transaction = dmabuf_template.to_surface_transaction(
        transaction,
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );

    let report = assembly
        .run_tick(CompositorBackendTickInput {
            authority_batches: vec![AuthorityTransactionIntake::new(
                transaction,
                vec![xpixmap_transaction, dmabuf_transaction],
            )],
            layer_templates: vec![xpixmap_template, dmabuf_template],
            ..CompositorBackendTickInput::default()
        })
        .expect("import-capable backend tick should complete");

    let render = report.render.unwrap();
    let xpixmap_import = render
        .imports
        .iter()
        .find(|import| import.surface == SurfaceId::new(4, 1))
        .expect("xpixmap surface should be imported");
    let dmabuf_import = render
        .imports
        .iter()
        .find(|import| import.surface == SurfaceId::new(5, 1))
        .expect("dmabuf surface should be imported");

    assert_eq!(xpixmap_import.requested, BufferImportPath::XPixmap);
    assert_eq!(xpixmap_import.used, BufferImportPath::XPixmap);
    assert_eq!(
        xpixmap_import.handle,
        ImportedBufferHandle::XPixmap { pixmap: 44 }
    );
    assert!(!xpixmap_import.used_fallback);
    assert_eq!(dmabuf_import.requested, BufferImportPath::DmaBuf);
    assert_eq!(dmabuf_import.used, BufferImportPath::CpuReadback);
    assert_eq!(
        dmabuf_import.handle,
        ImportedBufferHandle::CpuReadback {
            source: BufferSource::DmaBuf { handle: 99 }
        }
    );
    assert!(dmabuf_import.used_fallback);
}

#[test]
fn backend_assembly_drains_bounded_authority_inbox_before_runtime_tick() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let transaction = TransactionId::from_raw(92);
    let mut template = test_layer(6, 0, 0, Region::empty());
    template.source = BufferSource::XPixmap { pixmap: 66 };
    let surface_transaction = template.to_surface_transaction(
        transaction,
        AuthorityKind::SophiaX,
        SurfaceTransactionReadiness::Ready,
        250,
        0,
    );
    sender
        .try_send(AuthorityTransactionIntake::new(
            transaction,
            vec![surface_transaction],
        ))
        .expect("test channel should accept one authority batch");
    let inbox = AuthorityTransactionInbox::new(receiver, 4);
    let mut assembly = HeadlessCompositorBackendAssembly::new(HeadlessOutput::deterministic())
        .with_authority_inbox(inbox);

    let report = assembly
        .run_tick(CompositorBackendTickInput {
            layer_templates: vec![template],
            ..CompositorBackendTickInput::default()
        })
        .expect("backend tick should drain authority inbox");

    assert_eq!(report.authority_inbox.drained, 1);
    assert!(!report.authority_inbox.disconnected);
    assert!(!report.authority_inbox.max_reached);
    assert_eq!(
        report
            .runtime
            .runtime_state
            .authority_transactions_committed,
        1
    );
    assert_eq!(report.runtime.runtime_state.authority_surfaces_applied, 1);
    assert_eq!(assembly.committed_surfaces().len(), 1);
    assert_eq!(
        assembly.committed_surfaces()[0].buffer,
        BufferSource::XPixmap { pixmap: 66 }
    );
}

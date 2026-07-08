use sophia_engine::HeadlessEngine;
use sophia_protocol::{
    BufferSource, LayerSnapshot, LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot,
    LayoutNodeState, OutputId, Rect, Region, ResizeSyncCapability, SurfaceConstraints, SurfaceId,
    TransactionId, TransactionOutcome, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket,
    WorkspaceId,
};
use sophia_wm_demo::ExternalWmClient;

#[test]
fn external_wm_restarts_without_losing_engine_state() {
    let wm = env!("CARGO_BIN_EXE_sophia-wm-demo");
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let workspace = WorkspaceId::from_raw(1);
    let mut layers = vec![layer(0), layer(1)];

    let first_response = ExternalWmClient::new(wm)
        .request(&relayout_request(
            TransactionId::from_raw(1),
            output.id,
            workspace,
            output.size.width,
            &layers,
        ))
        .unwrap();
    let first_transaction = first_response.into_layout_transaction();
    let first_commit = engine.commit_layout_transaction(&first_transaction, &mut layers);

    assert_eq!(first_commit.outcome, TransactionOutcome::Committed);
    assert_eq!(layers[0].geometry.width, 640);
    assert_eq!(layers[1].geometry.x, 640);

    let preserved = layers.clone();
    let absent_commit = engine.preserve_layout_on_wm_absent(TransactionId::from_raw(2), &layers);

    assert_eq!(absent_commit.outcome, TransactionOutcome::TimedOut);
    assert_eq!(layers, preserved);

    let second_response = ExternalWmClient::new(wm)
        .request(&relayout_request(
            TransactionId::from_raw(3),
            output.id,
            workspace,
            1000,
            &layers,
        ))
        .unwrap();
    let second_transaction = second_response.into_layout_transaction();
    let second_commit = engine.commit_layout_transaction(&second_transaction, &mut layers);

    assert_eq!(second_commit.outcome, TransactionOutcome::Committed);
    assert_eq!(layers[0].geometry.width, 500);
    assert_eq!(layers[1].geometry.x, 500);
}

fn relayout_request(
    transaction: TransactionId,
    output: OutputId,
    workspace: WorkspaceId,
    width: i32,
    layers: &[LayerSnapshot],
) -> WmRequestPacket {
    WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output,
            workspace,
            bounds: Rect {
                x: 0,
                y: 0,
                width,
                height: 720,
            },
            nodes: layers
                .iter()
                .map(|layer| LayoutNodeSnapshot {
                    surface: layer.surface,
                    workspace,
                    kind: LayoutNodeKind::Toplevel,
                    capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
                    state: LayoutNodeState::NORMAL,
                    constraints: SurfaceConstraints {
                        min_size: None,
                        max_size: None,
                    },
                    geometry: layer.geometry,
                    generation: layer.generation,
                })
                .collect(),
        }),
    }
}

fn layer(index: u32) -> LayerSnapshot {
    LayerSnapshot {
        surface: SurfaceId::new(index, 1),
        window: None,
        namespace: None,
        stack_rank: index,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        source: BufferSource::CpuBuffer {
            handle: u64::from(index) + 1,
        },
        damage: Region::empty(),
        opacity: 1.0,
        crop: None,
        transform: sophia_protocol::Transform::IDENTITY,
        generation: 1,
        resize_sync: ResizeSyncCapability::ImplicitOnly,
    }
}

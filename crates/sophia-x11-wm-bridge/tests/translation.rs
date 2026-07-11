use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, NamespaceId,
    OutputId, Rect, Size, SurfaceConstraints, SurfaceId, TransactionId, WmCommand,
    WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WorkspaceId,
};
use sophia_x11_wm_bridge::{LegacyWmRequest, SyntheticXEvent, X11WmBridgeState};

fn node(raw: u32) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: SurfaceId::new(raw, 1),
        workspace: WorkspaceId::from_raw(1),
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        geometry: Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
        generation: 1,
    }
}

#[test]
fn translates_two_synthetic_xmonad_tiles_without_metadata() {
    let transaction = TransactionId::from_raw(71);
    let request = WmRequestPacket {
        transaction,
        kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
            output: OutputId::from_raw(1),
            workspace: WorkspaceId::from_raw(1),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1200,
                height: 800,
            },
            nodes: vec![node(10), node(11)],
        }),
    };
    let mut bridge = X11WmBridgeState::new();
    let update = bridge.apply_engine_request(&request).unwrap();
    assert_eq!(update.events.len(), 3);
    assert!(matches!(
        update.events[1],
        SyntheticXEvent::MapRequest { .. }
    ));
    assert!(matches!(
        update.events[2],
        SyntheticXEvent::MapRequest { .. }
    ));

    let left = bridge.synthetic_window(SurfaceId::new(10, 1)).unwrap();
    let right = bridge.synthetic_window(SurfaceId::new(11, 1)).unwrap();
    let response = bridge
        .translate_legacy_requests(
            transaction,
            &[
                LegacyWmRequest::ConfigureWindow {
                    window: left,
                    geometry: Rect {
                        x: 0,
                        y: 0,
                        width: 600,
                        height: 800,
                    },
                    z_index: 0,
                },
                LegacyWmRequest::ConfigureWindow {
                    window: right,
                    geometry: Rect {
                        x: 600,
                        y: 0,
                        width: 600,
                        height: 800,
                    },
                    z_index: 1,
                },
                LegacyWmRequest::FocusWindow { window: left },
            ],
            300,
        )
        .unwrap();

    assert_eq!(response.commands.len(), 5);
    assert!(
        response
            .commands
            .contains(&WmCommand::FocusSurface(SurfaceId::new(10, 1)))
    );
    assert!(!format!("{response:?}").contains(&format!("{:?}", NamespaceId::from_raw(99))));
}

mod tests {
    use sophia_protocol::*;
    use sophia_wm_demo::*;

    fn node(index: u32, workspace: WorkspaceId) -> LayoutNodeSnapshot {
        LayoutNodeSnapshot {
            surface: SurfaceId::new(index, 1),
            workspace,
            kind: LayoutNodeKind::Toplevel,
            capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
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
            generation: 1,
        }
    }

    #[test]
    fn tiles_opaque_layout_nodes_without_metadata() {
        let workspace = WorkspaceId::from_raw(1);
        let transaction = tile_workspace(
            TransactionId::from_raw(10),
            workspace,
            Rect {
                x: 0,
                y: 0,
                width: 1000,
                height: 500,
            },
            &[node(0, workspace), node(1, workspace)],
        );

        assert_eq!(transaction.focus, Some(SurfaceId::new(0, 1)));
        assert_eq!(transaction.render_positions.len(), 2);
        assert_eq!(transaction.render_positions[0].geometry.width, 500);
        assert_eq!(transaction.render_positions[1].geometry.x, 500);
        assert_eq!(transaction.requested_sizes[0].size.width, 500);
    }

    #[test]
    fn ignores_nodes_from_other_workspaces() {
        let workspace = WorkspaceId::from_raw(1);
        let other_workspace = WorkspaceId::from_raw(2);
        let transaction = tile_workspace(
            TransactionId::from_raw(11),
            workspace,
            Rect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
            &[node(0, other_workspace), node(1, workspace)],
        );

        assert_eq!(transaction.render_positions.len(), 1);
        assert_eq!(
            transaction.render_positions[0].surface,
            SurfaceId::new(1, 1)
        );
    }

    #[test]
    fn handles_manage_request_with_first_external_wm_sequence() {
        let workspace = WorkspaceId::from_raw(1);
        let surface = SurfaceId::new(3, 1);
        let request = WmRequestPacket {
            transaction: TransactionId::from_raw(12),
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                node: node(3, workspace),
                output: sophia_protocol::OutputId::from_raw(1),
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 800,
                    height: 600,
                },
            }),
        };

        let response = handle_wm_request(request);
        let transaction = response.clone().into_layout_transaction();

        assert_eq!(response.transaction, TransactionId::from_raw(12));
        assert!(
            response
                .commands
                .contains(&WmCommand::AssignWorkspace { surface, workspace })
        );
        assert!(
            response
                .commands
                .contains(&WmCommand::FocusSurface(surface))
        );
        assert_eq!(transaction.render_positions.len(), 1);
        assert_eq!(transaction.render_positions[0].geometry.width, 800);
        assert_eq!(transaction.render_positions[0].crop, None);
    }

    #[test]
    fn handles_relayout_request_without_workspace_assignment() {
        let workspace = WorkspaceId::from_raw(1);
        let request = WmRequestPacket {
            transaction: TransactionId::from_raw(13),
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: sophia_protocol::OutputId::from_raw(1),
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1000,
                    height: 500,
                },
                nodes: vec![node(0, workspace), node(1, workspace)],
            }),
        };

        let response = handle_wm_request(request);
        let transaction = response.clone().into_layout_transaction();

        assert_eq!(
            response
                .commands
                .iter()
                .filter(|command| matches!(command, WmCommand::AssignWorkspace { .. }))
                .count(),
            0
        );
        assert_eq!(transaction.render_positions.len(), 2);
        assert_eq!(transaction.render_positions[1].geometry.x, 500);
    }

    #[test]
    fn process_response_codec_preserves_workspace_assignment() {
        let workspace = WorkspaceId::from_raw(7);
        let response = handle_wm_request(WmRequestPacket {
            transaction: TransactionId::from_raw(14),
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                node: node(4, workspace),
                output: sophia_protocol::OutputId::from_raw(1),
                workspace,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 900,
                    height: 600,
                },
            }),
        });

        let line = encode_process_response(&response);
        let decoded = decode_process_response(&line).unwrap();

        assert!(decoded.commands.contains(&WmCommand::AssignWorkspace {
            surface: SurfaceId::new(4, 1),
            workspace,
        }));
        assert!(
            decoded
                .commands
                .contains(&WmCommand::FocusSurface(SurfaceId::new(4, 1)))
        );
    }
}

use sophia_protocol::{
    LayoutNodeSnapshot, LayoutTransaction, Rect, Size, SurfacePlacement, SurfaceSizeRequest,
    TransactionId, Transform, WmCommand, WmRequestKind, WmRequestPacket, WmResponsePacket,
    WmSessionAction, WorkspaceId,
};

pub fn empty_transaction(transaction: TransactionId) -> LayoutTransaction {
    LayoutTransaction {
        transaction,
        requested_sizes: Vec::new(),
        focus: None,
        render_positions: Vec::new(),
        timeout_msec: 300,
    }
}

pub fn tile_workspace(
    transaction: TransactionId,
    workspace: WorkspaceId,
    bounds: Rect,
    nodes: &[LayoutNodeSnapshot],
) -> LayoutTransaction {
    let visible_nodes = nodes
        .iter()
        .filter(|node| node.workspace == workspace && node.state.visible)
        .collect::<Vec<_>>();

    if visible_nodes.is_empty() || bounds.is_empty() {
        return empty_transaction(transaction);
    }

    let width = bounds.width / i32::try_from(visible_nodes.len()).expect("visible node overflow");
    let mut render_positions = Vec::with_capacity(visible_nodes.len());
    let mut requested_sizes = Vec::with_capacity(visible_nodes.len());
    let mut focus = None;

    for (index, node) in visible_nodes.iter().enumerate() {
        let index = i32::try_from(index).expect("visible node index overflow");
        let is_last =
            usize::try_from(index + 1).expect("visible node index overflow") == visible_nodes.len();
        let x = bounds.x + width * index;
        let tile_width = if is_last {
            bounds.x + bounds.width - x
        } else {
            width
        };
        let geometry = Rect {
            x,
            y: bounds.y,
            width: tile_width.max(1),
            height: bounds.height,
        };
        let requested = clamp_size(
            Size {
                width: geometry.width,
                height: geometry.height,
            },
            node.constraints.min_size,
            node.constraints.max_size,
        );

        if focus.is_none() && node.capabilities.focusable {
            focus = Some(node.surface);
        }

        requested_sizes.push(SurfaceSizeRequest {
            surface: node.surface,
            size: requested,
        });
        render_positions.push(SurfacePlacement {
            surface: node.surface,
            geometry,
            z_index: index,
            crop: None,
            transform: Transform::IDENTITY,
        });
    }

    LayoutTransaction {
        transaction,
        requested_sizes,
        focus,
        render_positions,
        timeout_msec: 300,
    }
}

pub fn handle_wm_request(request: WmRequestPacket) -> WmResponsePacket {
    match request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let transaction = tile_workspace(
                request.transaction,
                manage.workspace,
                manage.bounds,
                &[manage.node],
            );
            response_from_layout_transaction(transaction, Some(manage.workspace))
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let transaction = tile_workspace(
                request.transaction,
                relayout.workspace,
                relayout.bounds,
                &relayout.nodes,
            );
            response_from_layout_transaction(transaction, None)
        }
        WmRequestKind::SurfaceRemoved { .. } => WmResponsePacket {
            transaction: request.transaction,
            commands: Vec::new(),
            timeout_msec: 300,
        },
        WmRequestKind::ActionActivated(activation) => {
            let commands = match activation.action.raw() {
                1 => {
                    let focus = activation
                        .focused_surface
                        .and_then(|focused| {
                            activation
                                .nodes
                                .iter()
                                .position(|node| node.surface == focused)
                        })
                        .map_or(0, |index| (index + 1) % activation.nodes.len().max(1));
                    activation
                        .nodes
                        .get(focus)
                        .filter(|node| node.capabilities.focusable)
                        .map(|node| vec![WmCommand::FocusSurface(node.surface)])
                        .unwrap_or_default()
                }
                2 => vec![WmCommand::ActivateWorkspace {
                    output: activation.output,
                    workspace: WorkspaceId::from_raw(2),
                }],
                3 => vec![WmCommand::RequestSessionAction {
                    action: WmSessionAction::LaunchTerminal,
                    target: None,
                }],
                _ => Vec::new(),
            };
            WmResponsePacket {
                transaction: request.transaction,
                commands,
                timeout_msec: 300,
            }
        }
    }
}

pub fn response_from_layout_transaction(
    transaction: LayoutTransaction,
    assigned_workspace: Option<WorkspaceId>,
) -> WmResponsePacket {
    let mut commands = Vec::new();

    if let Some(workspace) = assigned_workspace {
        for placement in &transaction.render_positions {
            commands.push(WmCommand::AssignWorkspace {
                surface: placement.surface,
                workspace,
            });
        }
    }

    commands.extend(
        transaction
            .requested_sizes
            .iter()
            .copied()
            .map(WmCommand::ConfigureSurface),
    );

    if let Some(focus) = transaction.focus {
        commands.push(WmCommand::FocusSurface(focus));
    }

    commands.extend(
        transaction
            .render_positions
            .iter()
            .copied()
            .map(WmCommand::RenderSurface),
    );

    WmResponsePacket {
        transaction: transaction.transaction,
        commands,
        timeout_msec: transaction.timeout_msec,
    }
}
fn clamp_size(size: Size, min_size: Option<Size>, max_size: Option<Size>) -> Size {
    let mut width = size.width;
    let mut height = size.height;

    if let Some(min_size) = min_size {
        width = width.max(min_size.width);
        height = height.max(min_size.height);
    }

    if let Some(max_size) = max_size {
        width = width.min(max_size.width);
        height = height.min(max_size.height);
    }

    Size { width, height }
}

use sophia_protocol::{
    LayoutNodeSnapshot, LayoutTransaction, Rect, Size, SurfacePlacement, SurfaceSizeRequest,
    TransactionId, Transform, WorkspaceId,
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

#[cfg(test)]
mod tests {
    use super::*;
    use sophia_protocol::{
        LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeState, SurfaceConstraints, SurfaceId,
    };

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
}

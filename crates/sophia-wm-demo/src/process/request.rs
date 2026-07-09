use sophia_protocol::{
    OutputId, TransactionId, WmManageSurface, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket,
    WorkspaceId,
};

use super::{
    codec::{
        arg_values, encode_node, encode_rect, parse_node, process_usage, required_node,
        required_rect, required_surface, required_u64,
    },
    error::WmProcessError,
};

pub fn request_to_process_args(request: &WmRequestPacket) -> Vec<String> {
    match &request.kind {
        WmRequestKind::ManageSurface(manage) => {
            let mut args = vec![
                "manage".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--output={}", manage.output.raw()),
                format!("--workspace={}", manage.workspace.raw()),
                format!("--bounds={}", encode_rect(manage.bounds)),
                format!("--node={}", encode_node(&manage.node)),
            ];
            args.shrink_to_fit();
            args
        }
        WmRequestKind::RelayoutWorkspace(relayout) => {
            let mut args = vec![
                "relayout".to_owned(),
                format!("--transaction={}", request.transaction.raw()),
                format!("--output={}", relayout.output.raw()),
                format!("--workspace={}", relayout.workspace.raw()),
                format!("--bounds={}", encode_rect(relayout.bounds)),
            ];
            args.extend(
                relayout
                    .nodes
                    .iter()
                    .map(|node| format!("--node={}", encode_node(node))),
            );
            args
        }
        WmRequestKind::SurfaceRemoved { surface, workspace } => vec![
            "remove".to_owned(),
            format!("--transaction={}", request.transaction.raw()),
            format!("--workspace={}", workspace.raw()),
            format!("--surface={}:{}", surface.index(), surface.generation()),
        ],
    }
}

pub fn parse_process_request(args: &[String]) -> Result<WmRequestPacket, WmProcessError> {
    let Some(kind) = args.first().map(String::as_str) else {
        return Err(WmProcessError::new(process_usage()));
    };
    let transaction = TransactionId::from_raw(required_u64(args, "--transaction")?);
    let workspace = WorkspaceId::from_raw(required_u64(args, "--workspace")?);

    match kind {
        "manage" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let bounds = required_rect(args, "--bounds")?;
            let node = required_node(args, "--node", workspace)?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::ManageSurface(WmManageSurface {
                    node,
                    output,
                    workspace,
                    bounds,
                }),
            })
        }
        "relayout" => {
            let output = OutputId::from_raw(required_u64(args, "--output")?);
            let bounds = required_rect(args, "--bounds")?;
            let nodes = arg_values(args, "--node")
                .into_iter()
                .map(|value| parse_node(value, workspace))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                    output,
                    workspace,
                    bounds,
                    nodes,
                }),
            })
        }
        "remove" => {
            let surface = required_surface(args, "--surface")?;
            Ok(WmRequestPacket {
                transaction,
                kind: WmRequestKind::SurfaceRemoved { surface, workspace },
            })
        }
        _ => Err(WmProcessError::new(process_usage())),
    }
}

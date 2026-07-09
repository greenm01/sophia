use crate::prelude::*;
use crate::{HeadlessEngine, LastCommittedLayout, WmTransactionUpdate, request_wm_over_stream};

impl HeadlessEngine {
    pub fn preserve_layout_on_wm_absent(
        &self,
        transaction: TransactionId,
        _layers: &[LayerSnapshot],
    ) -> TransactionCommit {
        warn!(
            transaction = transaction.raw(),
            outcome = ?TransactionOutcome::TimedOut,
            "preserving layout because WM transaction is absent"
        );
        TransactionCommit {
            transaction,
            outcome: TransactionOutcome::TimedOut,
            applied_surfaces: Vec::new(),
        }
    }

    pub fn request_and_commit_wm_transaction<S>(
        &self,
        stream: &mut S,
        request: &WmRequestPacket,
        layers: &mut Vec<LayerSnapshot>,
    ) -> WmTransactionUpdate
    where
        S: Read + Write,
    {
        debug!(
            transaction = request.transaction.raw(),
            request_kind = wm_request_kind_name(&request.kind),
            node_count = wm_request_node_count(&request.kind),
            layer_count = layers.len(),
            "requesting WM transaction"
        );
        match request_wm_over_stream(stream, request) {
            Ok(response) => {
                debug!(
                    transaction = request.transaction.raw(),
                    response_commands = response.commands.len(),
                    response_timeout_msec = response.timeout_msec,
                    "received WM transaction response"
                );
                let transaction = response.into_layout_transaction();
                WmTransactionUpdate {
                    commit: self.commit_layout_transaction(&transaction, layers),
                    ipc_error: None,
                }
            }
            Err(error) => {
                warn!(
                    transaction = request.transaction.raw(),
                    error = %error,
                    "WM transaction IPC failed; preserving layout"
                );
                WmTransactionUpdate {
                    commit: self.preserve_layout_on_wm_absent(request.transaction, layers),
                    ipc_error: Some(error),
                }
            }
        }
    }

    pub fn request_and_cache_wm_transaction<S>(
        &self,
        stream: &mut S,
        request: &WmRequestPacket,
        layers: &mut Vec<LayerSnapshot>,
        last_committed: &mut LastCommittedLayout,
    ) -> WmTransactionUpdate
    where
        S: Read + Write,
    {
        let update = self.request_and_commit_wm_transaction(stream, request, layers);
        match update.commit.outcome {
            TransactionOutcome::Committed => {
                last_committed.replace(layers);
                debug!(
                    transaction = request.transaction.raw(),
                    cached_layers = last_committed.layers().len(),
                    "updated last committed layout cache"
                );
            }
            TransactionOutcome::TimedOut if !last_committed.is_empty() => {
                last_committed.restore_into(layers);
                warn!(
                    transaction = request.transaction.raw(),
                    restored_layers = layers.len(),
                    "restored last committed layout after WM timeout"
                );
            }
            _ => {
                debug!(
                    transaction = request.transaction.raw(),
                    outcome = ?update.commit.outcome,
                    cached_layers = last_committed.layers().len(),
                    "left last committed layout cache unchanged"
                );
            }
        }
        update
    }
}

fn wm_request_kind_name(kind: &WmRequestKind) -> &'static str {
    match kind {
        WmRequestKind::ManageSurface(_) => "manage_surface",
        WmRequestKind::RelayoutWorkspace(_) => "relayout_workspace",
        WmRequestKind::SurfaceRemoved { .. } => "surface_removed",
    }
}

fn wm_request_node_count(kind: &WmRequestKind) -> usize {
    match kind {
        WmRequestKind::ManageSurface(_) => 1,
        WmRequestKind::RelayoutWorkspace(relayout) => relayout.nodes.len(),
        WmRequestKind::SurfaceRemoved { .. } => 0,
    }
}

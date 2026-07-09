use crate::prelude::*;
use crate::{ChromeActionDecision, ReplayReport, validate_chrome_action};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    ChromeAction(ChromeActionRequest),
    SurfaceRemoved {
        transaction: TransactionId,
        surface: SurfaceId,
        workspace: WorkspaceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionUpdate {
    pub chrome_decision: Option<ChromeActionDecision>,
    pub commands: Vec<SessionCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionCommand {
    RequestPoliteClose { surface: SurfaceId },
    SendWmRequest(WmRequestPacket),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionLayerSource {
    Fresh(Vec<LayerSnapshot>),
    RestoreLastCommitted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionTickRequest {
    pub output: OutputId,
    pub frame_serial: u64,
    pub layers: SessionLayerSource,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionTickReport {
    pub frame: FrameSnapshot,
    pub replay: ReplayReport,
    pub restored_last_committed: bool,
}

pub fn handle_session_event(event: SessionEvent, nodes: &[LayoutNodeSnapshot]) -> SessionUpdate {
    match event {
        SessionEvent::ChromeAction(request) => {
            let decision = validate_chrome_action(&request, nodes);
            let commands = match &decision {
                ChromeActionDecision::RequestPoliteClose { surface } => {
                    vec![SessionCommand::RequestPoliteClose { surface: *surface }]
                }
                ChromeActionDecision::Rejected(_) => Vec::new(),
            };
            debug!(
                surface_index = request.surface.index(),
                surface_generation = request.surface.generation(),
                action = ?request.kind,
                decision = ?decision,
                command_count = commands.len(),
                "handled chrome session event"
            );

            SessionUpdate {
                chrome_decision: Some(decision),
                commands,
            }
        }
        SessionEvent::SurfaceRemoved {
            transaction,
            surface,
            workspace,
        } => {
            debug!(
                transaction = transaction.raw(),
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                workspace = workspace.raw(),
                "handled surface removed session event"
            );
            SessionUpdate {
                chrome_decision: None,
                commands: vec![SessionCommand::SendWmRequest(WmRequestPacket {
                    transaction,
                    kind: WmRequestKind::SurfaceRemoved { surface, workspace },
                })],
            }
        }
    }
}

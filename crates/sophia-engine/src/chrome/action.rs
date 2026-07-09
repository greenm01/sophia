use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromeActionDecision {
    RequestPoliteClose { surface: SurfaceId },
    Rejected(ChromeActionRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeActionRejectReason {
    UnknownSurface,
    StaleGeneration,
    NotClosable,
    UnsupportedAction,
}

pub fn validate_chrome_action(
    request: &ChromeActionRequest,
    nodes: &[LayoutNodeSnapshot],
) -> ChromeActionDecision {
    let Some(node) = nodes.iter().find(|node| node.surface == request.surface) else {
        warn!(
            surface_index = request.surface.index(),
            surface_generation = request.surface.generation(),
            request_generation = request.generation,
            action = ?request.kind,
            "rejected chrome action for unknown surface"
        );
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::UnknownSurface);
    };

    if node.generation != request.generation {
        warn!(
            surface_index = request.surface.index(),
            surface_generation = request.surface.generation(),
            request_generation = request.generation,
            current_generation = node.generation,
            action = ?request.kind,
            "rejected stale chrome action"
        );
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::StaleGeneration);
    }

    match request.kind {
        ChromeActionKind::CloseSurfaceRequested => {
            if node.capabilities.closable {
                debug!(
                    surface_index = request.surface.index(),
                    surface_generation = request.surface.generation(),
                    request_generation = request.generation,
                    action = ?request.kind,
                    "accepted chrome action"
                );
                ChromeActionDecision::RequestPoliteClose {
                    surface: request.surface,
                }
            } else {
                warn!(
                    surface_index = request.surface.index(),
                    surface_generation = request.surface.generation(),
                    request_generation = request.generation,
                    action = ?request.kind,
                    "rejected chrome action for non-closable surface"
                );
                ChromeActionDecision::Rejected(ChromeActionRejectReason::NotClosable)
            }
        }
    }
}

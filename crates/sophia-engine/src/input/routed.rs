use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct QueuedRoutedInput {
    pub event: InputEventPacket,
    pub route: InputRoute,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutedInputFlush {
    pub reason: RoutedInputFlushReason,
    pub inputs: Vec<QueuedRoutedInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputRequestError {
    SerialMismatch,
    RouteNotAccepted,
    MissingTargetSurface,
    MissingLocalPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputFlushReason {
    FrameBoundary,
    StateChangingInput,
    TargetCrossing,
    DragStateChanged,
    GrabChanged,
    FocusChanged,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoutedInputQueueAction {
    BufferedMotion,
    Flushed(RoutedInputFlush),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoutedInputCoalescer {
    pending_motion: Option<QueuedRoutedInput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RoutedInputRouteKey {
    target_surface: SurfaceId,
}

impl RoutedInputCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: InputEventPacket, route: InputRoute) -> RoutedInputQueueAction {
        let input = QueuedRoutedInput { event, route };

        if let Some(key) = coalescible_motion_key(&input) {
            return self.push_motion(input, key);
        }

        self.flush_with(RoutedInputFlushReason::StateChangingInput, Some(input))
    }

    pub fn flush_frame(&mut self) -> Option<RoutedInputFlush> {
        self.take_pending(RoutedInputFlushReason::FrameBoundary)
    }

    pub fn flush_barrier(&mut self, reason: RoutedInputFlushReason) -> Option<RoutedInputFlush> {
        self.take_pending(reason)
    }

    pub fn has_pending_motion(&self) -> bool {
        self.pending_motion.is_some()
    }

    fn push_motion(
        &mut self,
        input: QueuedRoutedInput,
        key: RoutedInputRouteKey,
    ) -> RoutedInputQueueAction {
        match self
            .pending_motion
            .as_ref()
            .and_then(coalescible_motion_key)
        {
            Some(pending_key) if pending_key == key => {
                self.pending_motion = Some(input);
                RoutedInputQueueAction::BufferedMotion
            }
            Some(_) => self.flush_with(RoutedInputFlushReason::TargetCrossing, Some(input)),
            None => {
                self.pending_motion = Some(input);
                RoutedInputQueueAction::BufferedMotion
            }
        }
    }

    fn flush_with(
        &mut self,
        reason: RoutedInputFlushReason,
        current: Option<QueuedRoutedInput>,
    ) -> RoutedInputQueueAction {
        let mut inputs = Vec::new();
        if let Some(pending) = self.pending_motion.take() {
            inputs.push(pending);
        }
        if let Some(current) = current {
            inputs.push(current);
        }

        RoutedInputQueueAction::Flushed(RoutedInputFlush { reason, inputs })
    }

    fn take_pending(&mut self, reason: RoutedInputFlushReason) -> Option<RoutedInputFlush> {
        self.pending_motion.take().map(|pending| RoutedInputFlush {
            reason,
            inputs: vec![pending],
        })
    }
}

pub fn routed_input_request_from_physical_event(
    event: &InputEventPacket,
    route: &InputRoute,
) -> Result<RoutedInputRequest, RoutedInputRequestError> {
    if event.serial != route.input_serial {
        return Err(RoutedInputRequestError::SerialMismatch);
    }
    if route.outcome != InputRouteOutcome::Routed {
        return Err(RoutedInputRequestError::RouteNotAccepted);
    }

    let target_surface = route
        .target_surface
        .filter(|surface| surface.is_valid())
        .ok_or(RoutedInputRequestError::MissingTargetSurface)?;
    let local_position = route
        .local_position
        .ok_or(RoutedInputRequestError::MissingLocalPosition)?;

    Ok(RoutedInputRequest {
        serial: event.serial,
        seat: event.seat,
        device: event.device,
        time_msec: event.time_msec,
        target_surface,
        global_position: route.global_position,
        local_position,
        kind: event.kind,
    })
}

pub fn routed_input_requests_from_flush(
    flush: &RoutedInputFlush,
) -> Result<Vec<RoutedInputRequest>, RoutedInputRequestError> {
    flush
        .inputs
        .iter()
        .map(|input| routed_input_request_from_physical_event(&input.event, &input.route))
        .collect()
}

fn coalescible_motion_key(input: &QueuedRoutedInput) -> Option<RoutedInputRouteKey> {
    if input.event.kind != InputEventKind::PointerMotion {
        return None;
    }
    if input.route.outcome != InputRouteOutcome::Routed {
        return None;
    }

    let target_surface = input
        .route
        .target_surface
        .filter(|surface| surface.is_valid())?;

    Some(RoutedInputRouteKey { target_surface })
}

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoutedInputAdapterError {
    SerialMismatch,
    NoTarget,
    StaleTarget,
    Denied,
    MissingTargetWindow,
    MissingLocalPosition,
    InvalidLocalPosition,
    UnsupportedTransform,
}

pub fn build_routed_input_request(
    event: &InputEventPacket,
    route: &InputRoute,
    target_window: XWindowId,
) -> Result<XLibreRoutedInputRequest, RoutedInputAdapterError> {
    build_routed_input_request_inner(event, route, target_window)
}

pub fn build_flat_routed_input_request(
    event: &InputEventPacket,
    route: &InputRoute,
    target_window: XWindowId,
) -> Result<XLibreRoutedInputRequest, RoutedInputAdapterError> {
    if route.transform != Transform::IDENTITY {
        return Err(RoutedInputAdapterError::UnsupportedTransform);
    }

    build_routed_input_request_inner(event, route, target_window)
}

fn build_routed_input_request_inner(
    event: &InputEventPacket,
    route: &InputRoute,
    target_window: XWindowId,
) -> Result<XLibreRoutedInputRequest, RoutedInputAdapterError> {
    if event.serial != route.input_serial {
        return Err(RoutedInputAdapterError::SerialMismatch);
    }

    match route.outcome {
        InputRouteOutcome::Routed => {}
        InputRouteOutcome::NoTarget => return Err(RoutedInputAdapterError::NoTarget),
        InputRouteOutcome::StaleTarget => return Err(RoutedInputAdapterError::StaleTarget),
        InputRouteOutcome::Denied => return Err(RoutedInputAdapterError::Denied),
    }

    if !target_window.is_valid() {
        return Err(RoutedInputAdapterError::MissingTargetWindow);
    }
    let local_position = route
        .local_position
        .ok_or(RoutedInputAdapterError::MissingLocalPosition)?;

    if !local_position.x.is_finite() || !local_position.y.is_finite() {
        return Err(RoutedInputAdapterError::InvalidLocalPosition);
    }

    Ok(XLibreRoutedInputRequest {
        serial: event.serial,
        seat: event.seat,
        device: event.device,
        time_msec: event.time_msec,
        target_window,
        local_position,
        kind: event.kind,
    })
}

pub fn routed_input_decision_allows_delivery(decision: &XLibreRoutedInputDecision) -> bool {
    decision.outcome == XLibreRoutedInputOutcome::Accepted
}

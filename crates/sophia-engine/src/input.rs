use crate::prelude::*;
use crate::render::should_render;

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
    MissingTargetWindow,
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
    target_surface: Option<SurfaceId>,
    target_window: XWindowId,
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
) -> Result<XLibreRoutedInputRequest, RoutedInputRequestError> {
    if event.serial != route.input_serial {
        return Err(RoutedInputRequestError::SerialMismatch);
    }
    if route.outcome != InputRouteOutcome::Routed {
        return Err(RoutedInputRequestError::RouteNotAccepted);
    }

    let target_window = route
        .target_window
        .filter(|window| window.is_valid())
        .ok_or(RoutedInputRequestError::MissingTargetWindow)?;
    let local_position = route
        .local_position
        .ok_or(RoutedInputRequestError::MissingLocalPosition)?;

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

pub fn routed_input_requests_from_flush(
    flush: &RoutedInputFlush,
) -> Result<Vec<XLibreRoutedInputRequest>, RoutedInputRequestError> {
    flush
        .inputs
        .iter()
        .map(|input| routed_input_request_from_physical_event(&input.event, &input.route))
        .collect()
}

pub fn hit_test_scene_for_input(event: &InputEventPacket, layers: &[LayerSnapshot]) -> InputRoute {
    let Some(global_position) = event.global_position else {
        return missed_input_route(event, Point::default());
    };

    let mut ordered_layers = layers.iter().collect::<Vec<_>>();
    ordered_layers.sort_by_key(|layer| layer.stack_rank);

    for layer in ordered_layers.into_iter().rev() {
        if !layer.surface.is_valid() || !should_render(layer) {
            continue;
        }

        let Some(untransformed_position) =
            inverse_transform_point(layer.transform, global_position)
        else {
            continue;
        };
        if !rect_contains_point(layer.geometry, untransformed_position) {
            continue;
        }

        let Some(target_window) = layer.window.filter(|window| window.is_valid()) else {
            continue;
        };

        return InputRoute {
            input_serial: event.serial,
            target_surface: Some(layer.surface),
            target_window: Some(target_window),
            global_position,
            local_position: Some(Point {
                x: untransformed_position.x - f64::from(layer.geometry.x),
                y: untransformed_position.y - f64::from(layer.geometry.y),
            }),
            transform: layer.transform,
            outcome: InputRouteOutcome::Routed,
        };
    }

    missed_input_route(event, global_position)
}

fn missed_input_route(event: &InputEventPacket, global_position: Point) -> InputRoute {
    InputRoute {
        input_serial: event.serial,
        target_surface: None,
        target_window: None,
        global_position,
        local_position: None,
        transform: sophia_protocol::Transform::IDENTITY,
        outcome: InputRouteOutcome::NoTarget,
    }
}

fn rect_contains_point(rect: Rect, point: Point) -> bool {
    let left = f64::from(rect.x);
    let top = f64::from(rect.y);
    let right = left + f64::from(rect.width);
    let bottom = top + f64::from(rect.height);

    point.x >= left && point.x < right && point.y >= top && point.y < bottom
}

fn inverse_transform_point(transform: sophia_protocol::Transform, point: Point) -> Option<Point> {
    let m = transform.matrix.map(f64::from);
    let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
        + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if !determinant.is_finite() || determinant.abs() < f64::EPSILON {
        return None;
    }

    let inv_det = 1.0 / determinant;
    let inverse = [
        (m[4] * m[8] - m[5] * m[7]) * inv_det,
        (m[2] * m[7] - m[1] * m[8]) * inv_det,
        (m[1] * m[5] - m[2] * m[4]) * inv_det,
        (m[5] * m[6] - m[3] * m[8]) * inv_det,
        (m[0] * m[8] - m[2] * m[6]) * inv_det,
        (m[2] * m[3] - m[0] * m[5]) * inv_det,
        (m[3] * m[7] - m[4] * m[6]) * inv_det,
        (m[1] * m[6] - m[0] * m[7]) * inv_det,
        (m[0] * m[4] - m[1] * m[3]) * inv_det,
    ];

    let x = inverse[0] * point.x + inverse[1] * point.y + inverse[2];
    let y = inverse[3] * point.x + inverse[4] * point.y + inverse[5];
    let w = inverse[6] * point.x + inverse[7] * point.y + inverse[8];
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || w.abs() < f64::EPSILON {
        return None;
    }

    Some(Point { x: x / w, y: y / w })
}

fn coalescible_motion_key(input: &QueuedRoutedInput) -> Option<RoutedInputRouteKey> {
    if input.event.kind != InputEventKind::PointerMotion {
        return None;
    }
    if input.route.outcome != InputRouteOutcome::Routed {
        return None;
    }

    let target_window = input
        .route
        .target_window
        .filter(|window| window.is_valid())?;

    Some(RoutedInputRouteKey {
        target_surface: input.route.target_surface,
        target_window,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputDeviceKind {
    Pointer,
    Keyboard,
    Touch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputDeviceDescriptor {
    pub seat: SeatId,
    pub device: DeviceId,
    pub kind: LibinputDeviceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputEventIngest {
    Accepted,
    UnknownDevice,
    SeatMismatch,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibinputEventSource {
    devices: BTreeMap<DeviceId, LibinputDeviceDescriptor>,
    pending: Vec<InputEventPacket>,
}

impl LibinputEventSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_device(&mut self, device: LibinputDeviceDescriptor) {
        self.devices.insert(device.device, device);
    }

    pub fn remove_device(&mut self, device: DeviceId) -> Option<LibinputDeviceDescriptor> {
        self.devices.remove(&device)
    }

    pub fn device(&self, device: DeviceId) -> Option<&LibinputDeviceDescriptor> {
        self.devices.get(&device)
    }

    pub fn devices(&self) -> impl Iterator<Item = &LibinputDeviceDescriptor> {
        self.devices.values()
    }

    pub fn push_event(&mut self, event: InputEventPacket) -> LibinputEventIngest {
        let Some(device) = self.devices.get(&event.device) else {
            return LibinputEventIngest::UnknownDevice;
        };
        if device.seat != event.seat {
            return LibinputEventIngest::SeatMismatch;
        }

        self.pending.push(event);
        LibinputEventIngest::Accepted
    }

    pub fn drain_events(&mut self) -> Vec<InputEventPacket> {
        self.pending.drain(..).collect()
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibinputPollReport {
    pub polled: usize,
    pub accepted: usize,
    pub rejected: Vec<LibinputEventIngest>,
}

pub trait NonBlockingInputPoller {
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibinputPhysicalInputAdapter<P> {
    poller: P,
    source: LibinputEventSource,
}

impl<P> LibinputPhysicalInputAdapter<P> {
    pub fn new(poller: P, source: LibinputEventSource) -> Self {
        Self { poller, source }
    }

    pub fn source(&self) -> &LibinputEventSource {
        &self.source
    }

    pub fn source_mut(&mut self) -> &mut LibinputEventSource {
        &mut self.source
    }

    pub fn into_source(self) -> LibinputEventSource {
        self.source
    }
}

impl<P> LibinputPhysicalInputAdapter<P>
where
    P: NonBlockingInputPoller,
{
    pub fn poll_once(&mut self) -> io::Result<LibinputPollReport> {
        let events = self.poller.poll_ready()?;
        let polled = events.len();
        let mut accepted = 0;
        let mut rejected = Vec::new();

        for event in events {
            match self.source.push_event(event) {
                LibinputEventIngest::Accepted => accepted += 1,
                rejected_outcome => rejected.push(rejected_outcome),
            }
        }

        Ok(LibinputPollReport {
            polled,
            accepted,
            rejected,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueuedInputPoller {
    queued: Vec<InputEventPacket>,
}

impl QueuedInputPoller {
    pub fn new(queued: Vec<InputEventPacket>) -> Self {
        Self { queued }
    }

    pub fn push(&mut self, event: InputEventPacket) {
        self.queued.push(event);
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}

impl NonBlockingInputPoller for QueuedInputPoller {
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>> {
        Ok(self.queued.drain(..).collect())
    }
}

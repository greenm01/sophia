use crate::prelude::*;
use crate::state::*;

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
) -> Result<XLibreRoutedInputRequest, RoutedInputAdapterError> {
    build_routed_input_request_inner(event, route)
}

pub fn build_flat_routed_input_request(
    event: &InputEventPacket,
    route: &InputRoute,
) -> Result<XLibreRoutedInputRequest, RoutedInputAdapterError> {
    if route.transform != Transform::IDENTITY {
        return Err(RoutedInputAdapterError::UnsupportedTransform);
    }

    build_routed_input_request_inner(event, route)
}

fn build_routed_input_request_inner(
    event: &InputEventPacket,
    route: &InputRoute,
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

    let target_window = route
        .target_window
        .filter(|window| window.is_valid())
        .ok_or(RoutedInputAdapterError::MissingTargetWindow)?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputEdgeKind {
    ActiveGrab,
    FocusPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputEdgeSmokeReport {
    pub edge: RoutedInputEdgeKind,
    pub decision: XLibreRoutedInputDecision,
    pub delivery_allowed: bool,
}

pub fn smoke_routed_input_edge(
    edge: RoutedInputEdgeKind,
    serial: u64,
    target_window: XWindowId,
) -> RoutedInputEdgeSmokeReport {
    let outcome = match edge {
        RoutedInputEdgeKind::ActiveGrab => XLibreRoutedInputOutcome::RejectedActiveGrab,
        RoutedInputEdgeKind::FocusPolicy => XLibreRoutedInputOutcome::RejectedFocusPolicy,
    };
    let decision = XLibreRoutedInputDecision {
        serial,
        target_window,
        outcome,
    };

    RoutedInputEdgeSmokeReport {
        edge,
        delivery_allowed: routed_input_decision_allows_delivery(&decision),
        decision,
    }
}

pub fn smoke_routed_input_edges(target_window: XWindowId) -> [RoutedInputEdgeSmokeReport; 2] {
    [
        smoke_routed_input_edge(RoutedInputEdgeKind::ActiveGrab, 1, target_window),
        smoke_routed_input_edge(RoutedInputEdgeKind::FocusPolicy, 2, target_window),
    ]
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputSmokeReport {
    pub display_name: Option<String>,
    pub extension_opcode: u8,
    pub target_window: XWindowId,
    pub device: DeviceId,
    pub decision: XLibreRoutedInputDecision,
    pub dispatch_elapsed: Duration,
    pub request_bytes: usize,
    pub event_x: i16,
    pub event_y: i16,
    pub button: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputStressReport {
    pub display_name: Option<String>,
    pub extension_opcode: u8,
    pub target_window: XWindowId,
    pub device: DeviceId,
    pub iterations: usize,
    pub accepted: usize,
    pub request_bytes: usize,
    pub threshold: Duration,
    pub stats: RoutedInputDispatchStats,
    pub recommendation: RoutedInputOptimizationRecommendation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputOptimizationRecommendation {
    KeepX11RequestPath,
    ConsiderSharedMemoryRing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutedInputTransport {
    X11Request,
    SharedMemoryRing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMemoryRouteRingState {
    Unavailable,
    Available,
    Failed,
}

pub fn select_routed_input_transport(
    recommendation: RoutedInputOptimizationRecommendation,
    shm_state: SharedMemoryRouteRingState,
) -> RoutedInputTransport {
    match (recommendation, shm_state) {
        (
            RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing,
            SharedMemoryRouteRingState::Available,
        ) => RoutedInputTransport::SharedMemoryRing,
        _ => RoutedInputTransport::X11Request,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RoutedInputDispatchStats {
    samples: Vec<Duration>,
}

impl RoutedInputDispatchStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_samples(samples: impl IntoIterator<Item = Duration>) -> Self {
        Self {
            samples: samples.into_iter().collect(),
        }
    }

    pub fn record(&mut self, elapsed: Duration) {
        self.samples.push(elapsed);
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn min(&self) -> Option<Duration> {
        self.samples.iter().copied().min()
    }

    pub fn max(&self) -> Option<Duration> {
        self.samples.iter().copied().max()
    }

    pub fn average(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }

        let total_nanos: u128 = self.samples.iter().map(|sample| sample.as_nanos()).sum();
        let average_nanos = total_nanos / self.samples.len() as u128;
        let average_nanos = average_nanos.min(u128::from(u64::MAX)) as u64;

        Some(Duration::from_nanos(average_nanos))
    }

    pub fn percentile_nearest(&self, percentile: u8) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }

        let percentile = percentile.min(100);
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let last = sorted.len() - 1;
        let index = (last * usize::from(percentile) + 50) / 100;

        sorted.get(index).copied()
    }

    pub fn recommendation(
        &self,
        max_dispatch_threshold: Duration,
    ) -> RoutedInputOptimizationRecommendation {
        match self.max() {
            Some(max) if max > max_dispatch_threshold => {
                RoutedInputOptimizationRecommendation::ConsiderSharedMemoryRing
            }
            _ => RoutedInputOptimizationRecommendation::KeepX11RequestPath,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SophiaRoutedInputDispatch {
    reply: SophiaRoutedInputRouteReply,
    elapsed: Duration,
    request_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SophiaRoutedInputRouteReply {
    serial: u64,
    target_window: XWindowId,
    outcome: XLibreRoutedInputOutcome,
}

impl TryParse for SophiaRoutedInputRouteReply {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let initial_value = value;
        let (response_type, value) = u8::try_parse(value)?;
        let value = value.get(1..).ok_or(ParseError::InsufficientData)?;
        let (_sequence, value) = u16::try_parse(value)?;
        let (length, value) = u32::try_parse(value)?;
        let (serial_hi, value) = u32::try_parse(value)?;
        let (serial_lo, value) = u32::try_parse(value)?;
        let (target_xid, value) = u32::try_parse(value)?;
        let (outcome, _value) = u16::try_parse(value)?;

        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }

        let target_window = XWindowId::new(target_xid, 1);
        if !target_window.is_valid() {
            return Err(ParseError::InvalidValue);
        }

        let outcome = match outcome {
            0 => XLibreRoutedInputOutcome::Accepted,
            1 => XLibreRoutedInputOutcome::RejectedStaleTarget,
            2 => XLibreRoutedInputOutcome::RejectedDeniedNamespace,
            3 => XLibreRoutedInputOutcome::RejectedActiveGrab,
            4 => XLibreRoutedInputOutcome::RejectedFocusPolicy,
            5 => XLibreRoutedInputOutcome::RejectedUnsupportedEvent,
            _ => return Err(ParseError::InvalidValue),
        };
        let reply = Self {
            serial: (u64::from(serial_hi) << 32) | u64::from(serial_lo),
            target_window,
            outcome,
        };
        let remaining = initial_value
            .get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;

        Ok((reply, remaining))
    }
}

pub fn smoke_routed_input(
    display_name: Option<&str>,
) -> Result<RoutedInputSmokeReport, XBridgeError> {
    let harness = RoutedInputHarness::new(display_name)?;
    let local_x = 42;
    let local_y = 37;
    let button = 1;
    let serial = 0x534f_5048_4941_0001;

    let request = XLibreRoutedInputRequest {
        serial,
        seat: SeatId::from_raw(1),
        device: harness.device,
        time_msec: 1,
        target_window: harness.target_window(),
        local_position: Point {
            x: f64::from(local_x),
            y: f64::from(local_y),
        },
        kind: InputEventKind::PointerButton {
            button: u32::from(button),
            pressed: true,
        },
    };
    let dispatch = harness.send(&request)?;
    let decision = XLibreRoutedInputDecision {
        serial: dispatch.reply.serial,
        target_window: dispatch.reply.target_window,
        outcome: dispatch.reply.outcome,
    };
    if !routed_input_decision_allows_delivery(&decision) {
        return Err(XBridgeError::RoutedInput {
            message: format!("routed input rejected with {:?}", decision.outcome),
        });
    }

    let (event_x, event_y, observed_button) =
        wait_for_routed_button_press(&harness.connection, harness.target, Duration::from_secs(2))?;

    if event_x != local_x || event_y != local_y || observed_button != button {
        return Err(XBridgeError::RoutedInput {
            message: format!(
                "unexpected routed button event local=({}, {}) button={}, expected=({}, {}) button={}",
                event_x, event_y, observed_button, local_x, local_y, button
            ),
        });
    }

    Ok(RoutedInputSmokeReport {
        display_name: display_name.map(str::to_owned),
        extension_opcode: harness.routed_major_opcode,
        target_window: harness.target_window(),
        device: harness.device,
        decision,
        dispatch_elapsed: dispatch.elapsed,
        request_bytes: dispatch.request_bytes,
        event_x,
        event_y,
        button: observed_button,
    })
}

pub fn stress_routed_input(
    display_name: Option<&str>,
    iterations: usize,
    threshold: Duration,
) -> Result<RoutedInputStressReport, XBridgeError> {
    if iterations == 0 {
        return Err(XBridgeError::RoutedInput {
            message: "routed-input stress iterations must be greater than zero".to_owned(),
        });
    }

    let harness = RoutedInputHarness::new(display_name)?;
    let mut stats = RoutedInputDispatchStats::new();
    let mut accepted = 0;
    let mut request_bytes = 0;
    let base_serial = 0x534f_5048_5354_0000;

    for index in 0..iterations {
        let local_x = 8 + i32::try_from(index % 120).unwrap_or(0);
        let local_y = 8 + i32::try_from((index / 120) % 80).unwrap_or(0);
        let request = XLibreRoutedInputRequest {
            serial: base_serial + u64::try_from(index).unwrap_or(u64::MAX),
            seat: SeatId::from_raw(1),
            device: harness.device,
            time_msec: u64::try_from(index).unwrap_or(u64::MAX),
            target_window: harness.target_window(),
            local_position: Point {
                x: f64::from(local_x),
                y: f64::from(local_y),
            },
            kind: InputEventKind::PointerMotion,
        };
        let dispatch = harness.send(&request)?;
        request_bytes = dispatch.request_bytes;
        stats.record(dispatch.elapsed);

        if dispatch.reply.outcome == XLibreRoutedInputOutcome::Accepted {
            accepted += 1;
        } else {
            return Err(XBridgeError::RoutedInput {
                message: format!(
                    "routed-input stress rejected sample {index} with {:?}",
                    dispatch.reply.outcome
                ),
            });
        }

        if index % 128 == 0 {
            drain_pending_events(&harness.connection)?;
        }
    }

    drain_pending_events(&harness.connection)?;
    let recommendation = stats.recommendation(threshold);

    Ok(RoutedInputStressReport {
        display_name: display_name.map(str::to_owned),
        extension_opcode: harness.routed_major_opcode,
        target_window: harness.target_window(),
        device: harness.device,
        iterations,
        accepted,
        request_bytes,
        threshold,
        stats,
        recommendation,
    })
}

struct RoutedInputHarness {
    connection: x11rb::rust_connection::RustConnection,
    routed_major_opcode: u8,
    target: Window,
    device: DeviceId,
}

impl RoutedInputHarness {
    fn new(display_name: Option<&str>) -> Result<Self, XBridgeError> {
        let (connection, screen_num) =
            x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
                message: error.to_string(),
            })?;
        let routed_info = connection
            .extension_information(XLIBRE_ROUTED_INPUT_EXTENSION_NAME)
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .ok_or_else(|| XBridgeError::RoutedInput {
                message: format!("missing {XLIBRE_ROUTED_INPUT_EXTENSION_NAME} extension"),
            })?;
        let screen = connection
            .setup()
            .roots
            .get(screen_num)
            .ok_or(XBridgeError::InvalidScreen { screen_num })?;
        let device = master_pointer_device(&connection)?;
        let target = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;
        let gc = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;
        let target_width = 160;
        let target_height = 120;

        connection
            .create_window(
                screen.root_depth,
                target,
                screen.root,
                12,
                14,
                target_width,
                target_height,
                0,
                WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &CreateWindowAux::new()
                    .background_pixel(screen.white_pixel)
                    .event_mask(
                        EventMask::EXPOSURE
                            | EventMask::STRUCTURE_NOTIFY
                            | EventMask::BUTTON_PRESS
                            | EventMask::BUTTON_RELEASE
                            | EventMask::POINTER_MOTION,
                    ),
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .create_gc(
                gc,
                target,
                &CreateGCAux::new()
                    .foreground(screen.black_pixel)
                    .background(screen.white_pixel),
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .map_window(target)
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .poly_fill_rectangle(
                target,
                gc,
                &[Rectangle {
                    x: 8,
                    y: 8,
                    width: target_width.saturating_sub(16),
                    height: target_height.saturating_sub(16),
                }],
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .flush()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;

        wait_for_mapped_window(&connection, target, Duration::from_secs(2))?;

        Ok(Self {
            connection,
            routed_major_opcode: routed_info.major_opcode,
            target,
            device,
        })
    }

    fn target_window(&self) -> XWindowId {
        XWindowId::new(self.target, 1)
    }

    fn send(
        &self,
        request: &XLibreRoutedInputRequest,
    ) -> Result<SophiaRoutedInputDispatch, XBridgeError> {
        send_sophia_routed_input_route(&self.connection, self.routed_major_opcode, request)
    }
}

fn master_pointer_device<C>(connection: &C) -> Result<DeviceId, XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let reply = connection
        .xinput_xi_query_device(Device::ALL_MASTER)
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?;

    reply
        .infos
        .iter()
        .find(|info: &&XIDeviceInfo| info.enabled && info.type_ == DeviceType::MASTER_POINTER)
        .map(|info| DeviceId::from_raw(u64::from(info.deviceid)))
        .ok_or_else(|| XBridgeError::RoutedInput {
            message: "no enabled XInput master pointer found".to_owned(),
        })
}

fn send_sophia_routed_input_route<C>(
    connection: &C,
    major_opcode: u8,
    request: &XLibreRoutedInputRequest,
) -> Result<SophiaRoutedInputDispatch, XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let wire = request.to_wire_request();
    let mut bytes = Vec::with_capacity(usize::from(XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH) * 4);
    major_opcode.serialize_into(&mut bytes);
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE.serialize_into(&mut bytes);
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH.serialize_into(&mut bytes);
    serialize_routed_input_wire(&wire, &mut bytes);

    let request_bytes = bytes.len();
    let start = Instant::now();
    let cookie = connection
        .send_request_with_reply::<SophiaRoutedInputRouteReply>(&[IoSlice::new(&bytes)], Vec::new())
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?;
    let reply = cookie.reply().map_err(|error| XBridgeError::RoutedInput {
        message: error.to_string(),
    })?;

    Ok(SophiaRoutedInputDispatch {
        reply,
        elapsed: start.elapsed(),
        request_bytes,
    })
}

pub const fn routed_input_request_wire_len() -> usize {
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH as usize * 4
}

fn serialize_routed_input_wire(wire: &XLibreRoutedInputWireRequest, bytes: &mut Vec<u8>) {
    wire.serial_hi.serialize_into(bytes);
    wire.serial_lo.serialize_into(bytes);
    wire.target_xid.serialize_into(bytes);
    wire.seat.serialize_into(bytes);
    wire.device.serialize_into(bytes);
    wire.time_msec.serialize_into(bytes);
    wire.local_x_24_8.serialize_into(bytes);
    wire.local_y_24_8.serialize_into(bytes);
    wire.event_code.serialize_into(bytes);
    wire.detail.serialize_into(bytes);
    wire.flags.serialize_into(bytes);
}

pub(crate) fn drain_pending_events<C>(connection: &C) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    while connection
        .poll_for_event()
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?
        .is_some()
    {}

    Ok(())
}

fn wait_for_mapped_window<C>(
    connection: &C,
    window: Window,
    timeout: Duration,
) -> Result<(), XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        let attrs = connection
            .get_window_attributes(window)
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        if attrs.map_state == MapState::VIEWABLE {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::RoutedInput {
        message: format!("timed out waiting for routed-input target {window:#x} to map"),
    })
}

fn wait_for_routed_button_press<C>(
    connection: &C,
    window: Window,
    timeout: Duration,
) -> Result<(i16, i16, u8), XBridgeError>
where
    C: Connection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(event) =
            connection
                .poll_for_event()
                .map_err(|error| XBridgeError::RoutedInput {
                    message: error.to_string(),
                })?
        {
            if let Event::ButtonPress(event) = event {
                if event.event == window {
                    return Ok((event.event_x, event.event_y, event.detail));
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::RoutedInput {
        message: format!("timed out waiting for routed button event on {window:#x}"),
    })
}

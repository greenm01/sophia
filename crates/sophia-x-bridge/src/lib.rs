use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::thread;
use std::time::Duration;
use std::{io::IoSlice, time::Instant};

use sophia_portal::{
    ClipboardPortal, ClipboardTarget, ClipboardTransferRequest, PortalCommand, PortalError,
};
use sophia_protocol::{
    BufferSource, DamageFrame, DeviceId, InputEventKind, InputEventPacket, InputRoute,
    InputRouteOutcome, LayerSnapshot, NamespaceId, OutputId, Point, PortalTransferId, Rect, Region,
    SeatId, Size, SurfaceId, SurfaceSnapshot, Transform, XLIBRE_ROUTED_INPUT_EXTENSION_NAME,
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH, XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE,
    XLibreRoutedInputDecision, XLibreRoutedInputOutcome, XLibreRoutedInputRequest,
    XLibreRoutedInputWireRequest, XWindowId, XWindowMirror,
};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::errors::ParseError;
use x11rb::protocol::Event;
use x11rb::protocol::composite::{ConnectionExt as CompositeConnectionExt, Redirect};
use x11rb::protocol::damage::{ConnectionExt as DamageConnectionExt, ReportLevel};
use x11rb::protocol::xfixes::{
    ConnectionExt as XFixesConnectionExt, SelectionEvent, SelectionEventMask,
};
use x11rb::protocol::xinput::{
    ConnectionExt as XInputConnectionExt, Device, DeviceType, XIDeviceInfo,
};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageData, ClientMessageEvent, ConnectionExt as _, CreateGCAux,
    CreateWindowAux, EventMask, ImageFormat, MapState, Place, PropMode, Rectangle,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Timestamp, Window,
    WindowClass,
};
use x11rb::wrapper::ConnectionExt as X11WrapperConnectionExt;
use x11rb::x11_utils::{Serialize, TryParse};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XMirrorState {
    windows: Vec<XWindowMirror>,
}

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

fn drain_pending_events<C>(connection: &C) -> Result<(), XBridgeError>
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

fn create_clipboard_smoke_window<C>(
    connection: &C,
    root: Window,
    depth: u8,
    visual: u32,
    window: Window,
) -> Result<(), XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    connection
        .create_window(
            depth,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })
}

fn wait_for_selection_request<C>(
    connection: &C,
    requestor: Window,
    selection: Atom,
    property: Atom,
    timeout: Duration,
) -> Result<SelectionRequestEvent, XBridgeError>
where
    C: Connection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(event) =
            connection
                .poll_for_event()
                .map_err(|error| XBridgeError::SelectionMonitor {
                    message: error.to_string(),
                })?
        {
            if let Event::SelectionRequest(event) = event {
                if event.requestor == requestor
                    && event.selection == selection
                    && event.property == property
                {
                    return Ok(event);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::SelectionMonitor {
        message: format!(
            "timed out waiting for SelectionRequest requestor={requestor:#x} selection={selection:#x} property={property:#x}"
        ),
    })
}

fn wait_for_selection_notify<C>(
    connection: &C,
    requestor: Window,
    selection: Atom,
    target: Atom,
    timeout: Duration,
) -> Result<SelectionNotifyEvent, XBridgeError>
where
    C: Connection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(event) =
            connection
                .poll_for_event()
                .map_err(|error| XBridgeError::SelectionMonitor {
                    message: error.to_string(),
                })?
        {
            if let Event::SelectionNotify(event) = event {
                if event.requestor == requestor
                    && event.selection == selection
                    && event.target == target
                {
                    return Ok(event);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::SelectionMonitor {
        message: format!(
            "timed out waiting for SelectionNotify requestor={requestor:#x} selection={selection:#x} target={target:#x}"
        ),
    })
}

fn clipboard_smoke_mirror(window: XWindowId, namespace: NamespaceId) -> XWindowMirror {
    XWindowMirror {
        window,
        parent: None,
        children: Vec::new(),
        toplevel: Some(window),
        client: Some(window),
        mapped: true,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        namespace: Some(namespace),
        stale_metadata: 0,
    }
}

impl XMirrorState {
    pub fn ingest_window(&mut self, mirror: XWindowMirror) {
        self.windows.push(mirror);
    }

    pub fn windows(&self) -> &[XWindowMirror] {
        &self.windows
    }

    pub fn emit_mirrors(&self) -> Vec<XWindowMirror> {
        self.windows.clone()
    }

    pub fn namespace_for_window(&self, window: XWindowId) -> Option<NamespaceId> {
        self.windows
            .iter()
            .find(|mirror| {
                mirror.window == window
                    || mirror.client == Some(window)
                    || mirror.toplevel == Some(window)
            })
            .and_then(|mirror| mirror.namespace)
    }

    pub fn apply_namespace_ownership(&mut self, ownership: &[NamespaceOwnership]) {
        for ownership in ownership {
            if !ownership.window.is_valid() || !ownership.namespace.is_valid() {
                continue;
            }

            for mirror in &mut self.windows {
                if mirror.window == ownership.window
                    || mirror.client == Some(ownership.window)
                    || mirror.toplevel == Some(ownership.window)
                {
                    mirror.namespace = Some(ownership.namespace);
                    mirror.stale_metadata = mirror.stale_metadata.saturating_add(1);
                }
            }
        }
    }

    pub fn apply_event(&mut self, event: XMirrorEvent) {
        match event {
            XMirrorEvent::Map { window } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.mapped = true;
                }
            }
            XMirrorEvent::Unmap { window } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.mapped = false;
                }
            }
            XMirrorEvent::Destroy { window } => {
                self.remove_window(window);
            }
            XMirrorEvent::Configure {
                window,
                geometry,
                above_sibling,
            } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.geometry = geometry;
                }
                self.apply_restack(window, above_sibling);
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Reparent { window, parent } => {
                self.reparent_window(window, parent);
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Property { window, .. } => {
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Restack { window, place } => {
                self.apply_circulate(window, place);
                self.mark_metadata_stale(window);
            }
        }
    }

    pub fn apply_client_hints(&mut self, hints: &XClientHints) {
        let client_windows = hints
            .ewmh_clients
            .iter()
            .chain(hints.icccm_clients.iter())
            .copied()
            .collect::<BTreeSet<_>>();

        for client in client_windows {
            let toplevel = self.toplevel_for_client(client).unwrap_or(client);

            if let Some(client_mirror) = self.window_mut(client) {
                client_mirror.client = Some(client);
                client_mirror.toplevel = Some(toplevel);
            }

            if let Some(toplevel_mirror) = self.window_mut(toplevel) {
                toplevel_mirror.client = Some(client);
                toplevel_mirror.toplevel = Some(toplevel);
            }
        }
    }

    pub fn apply_unmanaged_client_fallback(&mut self) {
        let root_windows = self
            .windows
            .iter()
            .filter(|mirror| mirror.parent.is_none())
            .map(|mirror| mirror.window)
            .collect::<BTreeSet<_>>();
        let fallback_clients = self
            .windows
            .iter()
            .filter(|mirror| mirror.client.is_none() && mirror.mapped)
            .filter(|mirror| {
                mirror
                    .parent
                    .is_some_and(|parent| root_windows.contains(&parent))
            })
            .map(|mirror| mirror.window)
            .collect::<Vec<_>>();

        for client in fallback_clients {
            if let Some(mirror) = self.window_mut(client) {
                mirror.client = Some(client);
                mirror.toplevel = Some(client);
            }
        }
    }

    pub fn emit_surfaces(
        &self,
        surfaces: &mut SurfaceIdMap,
        pixmaps: &CompositePixmapMap,
    ) -> Vec<SurfaceSnapshot> {
        self.windows
            .iter()
            .filter(|mirror| mirror.client.is_some())
            .map(|mirror| SurfaceSnapshot {
                surface: surfaces.surface_for_window(mirror.window),
                window: mirror.window,
                toplevel: mirror.toplevel,
                client: mirror.client,
                namespace: mirror.namespace,
                mapped: mirror.mapped,
                stack_rank: mirror.stack_rank,
                geometry: mirror.geometry,
                source: mirror.client.map_or(BufferSource::None, |client| {
                    pixmaps.source_for_window(client)
                }),
                damage: Region::single(mirror.geometry),
                generation: mirror.stale_metadata,
            })
            .collect()
    }

    pub fn emit_layers(
        &self,
        surfaces: &mut SurfaceIdMap,
        pixmaps: &CompositePixmapMap,
    ) -> Vec<LayerSnapshot> {
        self.emit_surfaces(surfaces, pixmaps)
            .into_iter()
            .filter(|surface| surface.mapped && !surface.geometry.is_empty())
            .map(|surface| LayerSnapshot {
                surface: surface.surface,
                window: Some(surface.window),
                namespace: surface.namespace,
                stack_rank: surface.stack_rank,
                geometry: surface.geometry,
                source: surface.source,
                damage: surface.damage,
                opacity: 1.0,
                crop: None,
                transform: Transform::IDENTITY,
                generation: surface.generation,
            })
            .collect()
    }

    pub fn composite_redirect_targets(&self) -> Vec<CompositeRedirectTarget> {
        self.windows
            .iter()
            .filter(|mirror| mirror.mapped)
            .filter_map(|mirror| mirror.client)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|window| CompositeRedirectTarget {
                window,
                update: CompositeUpdateMode::Manual,
            })
            .collect()
    }

    fn window_mut(&mut self, window: XWindowId) -> Option<&mut XWindowMirror> {
        self.windows
            .iter_mut()
            .find(|mirror| mirror.window == window)
    }

    fn remove_window(&mut self, window: XWindowId) {
        self.windows.retain(|mirror| mirror.window != window);
        for mirror in &mut self.windows {
            mirror.children.retain(|child| *child != window);
        }
    }

    fn reparent_window(&mut self, window: XWindowId, parent: Option<XWindowId>) {
        for mirror in &mut self.windows {
            mirror.children.retain(|child| *child != window);
        }

        if let Some(mirror) = self.window_mut(window) {
            mirror.parent = parent;
        }

        if let Some(parent) = parent {
            if let Some(parent) = self.window_mut(parent) {
                if !parent.children.contains(&window) {
                    parent.children.push(window);
                }
            }
        }
    }

    fn apply_restack(&mut self, window: XWindowId, above_sibling: Option<XWindowId>) {
        let stack_rank = above_sibling
            .and_then(|sibling| self.windows.iter().find(|mirror| mirror.window == sibling))
            .map_or(0, |sibling| sibling.stack_rank.saturating_add(1));

        if let Some(mirror) = self.window_mut(window) {
            mirror.stack_rank = stack_rank;
        }
    }

    fn apply_circulate(&mut self, window: XWindowId, place: RestackPlace) {
        let rank = match place {
            RestackPlace::OnTop => self
                .windows
                .iter()
                .map(|mirror| mirror.stack_rank)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            RestackPlace::OnBottom => 0,
        };

        if let Some(mirror) = self.window_mut(window) {
            mirror.stack_rank = rank;
        }
    }

    fn mark_metadata_stale(&mut self, window: XWindowId) {
        if let Some(mirror) = self.window_mut(window) {
            mirror.stale_metadata = mirror.stale_metadata.saturating_add(1);
        }
    }

    fn toplevel_for_client(&self, client: XWindowId) -> Option<XWindowId> {
        let mut current = client;

        loop {
            let mirror = self
                .windows
                .iter()
                .find(|mirror| mirror.window == current)?;
            let Some(parent) = mirror.parent else {
                return Some(current);
            };
            let Some(parent_mirror) = self.windows.iter().find(|mirror| mirror.window == parent)
            else {
                return Some(current);
            };

            if parent_mirror.parent.is_none() {
                return Some(current);
            }

            current = parent;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeRedirectTarget {
    pub window: XWindowId,
    pub update: CompositeUpdateMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeUpdateMode {
    Automatic,
    Manual,
}

impl CompositeUpdateMode {
    fn to_x11(self) -> Redirect {
        match self {
            Self::Automatic => Redirect::AUTOMATIC,
            Self::Manual => Redirect::MANUAL,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceIdMap {
    next_index: u32,
    surfaces: BTreeMap<XWindowId, SurfaceId>,
}

impl SurfaceIdMap {
    pub fn surface_for_window(&mut self, window: XWindowId) -> SurfaceId {
        if let Some(surface) = self.surfaces.get(&window) {
            return *surface;
        }

        let index = self.next_index;
        self.next_index = self
            .next_index
            .checked_add(1)
            .filter(|next| *next != u32::MAX)
            .expect("Sophia surface ID map overflow");
        let surface = SurfaceId::new(index, window.generation());
        self.surfaces.insert(window, surface);
        surface
    }

    pub fn window_for_surface(&self, surface: SurfaceId) -> Option<XWindowId> {
        self.surfaces
            .iter()
            .find_map(|(window, candidate)| (*candidate == surface).then_some(*window))
    }
}

pub fn close_target_for_surface(
    mirror: &XMirrorState,
    surfaces: &SurfaceIdMap,
    surface: SurfaceId,
) -> Option<XWindowId> {
    let window = surfaces.window_for_surface(surface)?;
    let mirrored = mirror
        .windows()
        .iter()
        .find(|mirror| mirror.window == window)?;

    mirrored
        .client
        .or(mirrored.toplevel)
        .or(Some(mirrored.window))
        .filter(|window| window.is_valid())
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompositePixmapMap {
    pixmaps: BTreeMap<XWindowId, CompositePixmapRecord>,
    next_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositePixmapRecord {
    pub window: XWindowId,
    pub pixmap: u32,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositePixmapLifetimeUpdate {
    pub window: XWindowId,
    pub current: Option<CompositePixmapRecord>,
    pub retired: Option<CompositePixmapRecord>,
}

impl CompositePixmapLifetimeUpdate {
    pub fn replaced_pixmap(&self) -> Option<u32> {
        self.retired.map(|record| record.pixmap)
    }
}

impl CompositePixmapMap {
    pub fn record_for_window(&self, window: XWindowId) -> Option<CompositePixmapRecord> {
        self.pixmaps.get(&window).copied()
    }

    pub fn pixmap_for_window(&self, window: XWindowId) -> Option<u32> {
        self.record_for_window(window).map(|record| record.pixmap)
    }

    pub fn upsert_named_pixmap(
        &mut self,
        window: XWindowId,
        pixmap: u32,
    ) -> CompositePixmapLifetimeUpdate {
        if let Some(current) = self.record_for_window(window) {
            if current.pixmap == pixmap {
                return CompositePixmapLifetimeUpdate {
                    window,
                    current: Some(current),
                    retired: None,
                };
            }
        }

        let generation = self.next_generation.max(1);
        self.next_generation = generation
            .checked_add(1)
            .filter(|next| *next != 0)
            .expect("Sophia composite pixmap generation overflow");
        let current = CompositePixmapRecord {
            window,
            pixmap,
            generation,
        };
        let retired = self.pixmaps.insert(window, current);

        CompositePixmapLifetimeUpdate {
            window,
            current: Some(current),
            retired,
        }
    }

    pub fn insert_named_pixmap(&mut self, window: XWindowId, pixmap: u32) -> Option<u32> {
        self.upsert_named_pixmap(window, pixmap).replaced_pixmap()
    }

    pub fn remove_window_record(
        &mut self,
        window: XWindowId,
    ) -> Option<CompositePixmapLifetimeUpdate> {
        let retired = self.pixmaps.remove(&window)?;
        Some(CompositePixmapLifetimeUpdate {
            window,
            current: None,
            retired: Some(retired),
        })
    }

    pub fn remove_window(&mut self, window: XWindowId) -> Option<u32> {
        self.remove_window_record(window)
            .and_then(|update| update.replaced_pixmap())
    }

    pub fn source_for_window(&self, window: XWindowId) -> BufferSource {
        self.pixmap_for_window(window)
            .map_or(BufferSource::None, |pixmap| BufferSource::XPixmap {
                pixmap,
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferSnapshot {
    pub handle: u64,
    pub pixmap: u32,
    pub size: Size,
    pub depth: u8,
    pub visual: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferStore {
    next_handle: u64,
    buffers: BTreeMap<u64, CpuBufferSnapshot>,
    handle_by_pixmap: BTreeMap<u32, u64>,
}

impl Default for CpuBufferStore {
    fn default() -> Self {
        Self {
            next_handle: 1,
            buffers: BTreeMap::new(),
            handle_by_pixmap: BTreeMap::new(),
        }
    }
}

impl CpuBufferStore {
    pub fn upsert_pixmap(
        &mut self,
        pixmap: u32,
        size: Size,
        depth: u8,
        visual: u32,
        bytes: Vec<u8>,
    ) -> CpuBufferSnapshot {
        let handle = self
            .handle_by_pixmap
            .get(&pixmap)
            .copied()
            .unwrap_or_else(|| {
                let handle = self.next_handle;
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|next| *next != 0)
                    .expect("Sophia CPU buffer handle overflow");
                self.handle_by_pixmap.insert(pixmap, handle);
                handle
            });
        let snapshot = CpuBufferSnapshot {
            handle,
            pixmap,
            size,
            depth,
            visual,
            bytes,
        };
        self.buffers.insert(handle, snapshot.clone());
        snapshot
    }

    pub fn get(&self, handle: u64) -> Option<&CpuBufferSnapshot> {
        self.buffers.get(&handle)
    }

    pub fn handle_for_pixmap(&self, pixmap: u32) -> Option<u64> {
        self.handle_by_pixmap.get(&pixmap).copied()
    }

    pub fn remove_pixmap(&mut self, pixmap: u32) -> Option<CpuBufferSnapshot> {
        let handle = self.handle_by_pixmap.remove(&pixmap)?;
        self.buffers.remove(&handle)
    }

    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DamageRecord {
    pub window: XWindowId,
    pub damage: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DamageTracker {
    damage_by_window: BTreeMap<XWindowId, u32>,
    window_by_damage: BTreeMap<u32, XWindowId>,
    pending_by_window: BTreeMap<XWindowId, Region>,
}

impl DamageTracker {
    pub fn insert_damage(&mut self, window: XWindowId, damage: u32) -> Option<u32> {
        let old_damage = self.damage_by_window.insert(window, damage);
        if let Some(old_damage) = old_damage {
            self.window_by_damage.remove(&old_damage);
        }
        self.window_by_damage.insert(damage, window);
        old_damage
    }

    pub fn damage_for_window(&self, window: XWindowId) -> Option<u32> {
        self.damage_by_window.get(&window).copied()
    }

    pub fn window_for_damage(&self, damage: u32) -> Option<XWindowId> {
        self.window_by_damage.get(&damage).copied()
    }

    pub fn record_for_window(&self, window: XWindowId) -> Option<DamageRecord> {
        self.damage_for_window(window)
            .map(|damage| DamageRecord { window, damage })
    }

    pub fn pending_damage(&self, window: XWindowId) -> Option<&Region> {
        self.pending_by_window.get(&window)
    }

    pub fn drain_damage(&mut self, window: XWindowId) -> Region {
        self.pending_by_window
            .remove(&window)
            .unwrap_or_else(Region::empty)
    }

    pub fn remove_window(&mut self, window: XWindowId) -> Option<u32> {
        self.pending_by_window.remove(&window);
        let damage = self.damage_by_window.remove(&window)?;
        self.window_by_damage.remove(&damage);
        Some(damage)
    }

    pub fn apply_event(&mut self, event: XDamageEvent) -> bool {
        if self.window_for_damage(event.damage) != Some(event.window) {
            return false;
        }

        self.pending_by_window
            .entry(event.window)
            .or_insert_with(Region::empty)
            .push(event.area);
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDamageEvent {
    pub window: XWindowId,
    pub damage: u32,
    pub drawable: XWindowId,
    pub timestamp: u32,
    pub area: Rect,
    pub drawable_geometry: Rect,
}

impl XDamageEvent {
    pub fn from_x11_event(event: &Event, tracker: &DamageTracker) -> Option<Self> {
        let Event::DamageNotify(event) = event else {
            return None;
        };
        let window = tracker.window_for_damage(event.damage)?;

        Some(Self {
            window,
            damage: event.damage,
            drawable: wrap_xid(event.drawable),
            timestamp: event.timestamp,
            area: Rect {
                x: i32::from(event.area.x),
                y: i32::from(event.area.y),
                width: i32::from(event.area.width),
                height: i32::from(event.area.height),
            },
            drawable_geometry: Rect {
                x: i32::from(event.geometry.x),
                y: i32::from(event.geometry.y),
                width: i32::from(event.geometry.width),
                height: i32::from(event.geometry.height),
            },
        })
    }
}

impl XSelectionEvent {
    pub fn from_x11_event(event: &Event) -> Option<Self> {
        let Event::XfixesSelectionNotify(event) = event else {
            return None;
        };

        Some(Self {
            selection: event.selection,
            owner: nonzero_window(event.owner).map(wrap_xid),
            timestamp: event.timestamp,
            selection_timestamp: event.selection_timestamp,
            kind: selection_change_kind(event.subtype),
        })
    }
}

fn selection_change_kind(kind: SelectionEvent) -> XSelectionChangeKind {
    if kind == SelectionEvent::SET_SELECTION_OWNER {
        XSelectionChangeKind::SetOwner
    } else if kind == SelectionEvent::SELECTION_WINDOW_DESTROY {
        XSelectionChangeKind::OwnerWindowDestroyed
    } else if kind == SelectionEvent::SELECTION_CLIENT_CLOSE {
        XSelectionChangeKind::OwnerClientClosed
    } else {
        XSelectionChangeKind::Unknown
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XMirrorEvent {
    Map {
        window: XWindowId,
    },
    Unmap {
        window: XWindowId,
    },
    Destroy {
        window: XWindowId,
    },
    Configure {
        window: XWindowId,
        geometry: Rect,
        above_sibling: Option<XWindowId>,
    },
    Reparent {
        window: XWindowId,
        parent: Option<XWindowId>,
    },
    Property {
        window: XWindowId,
        atom: u32,
        deleted: bool,
    },
    Restack {
        window: XWindowId,
        place: RestackPlace,
    },
}

impl XMirrorEvent {
    pub fn from_x11_event(event: &Event) -> Option<Self> {
        match event {
            Event::MapNotify(event) => Some(Self::Map {
                window: wrap_xid(event.window),
            }),
            Event::UnmapNotify(event) => Some(Self::Unmap {
                window: wrap_xid(event.window),
            }),
            Event::DestroyNotify(event) => Some(Self::Destroy {
                window: wrap_xid(event.window),
            }),
            Event::ConfigureNotify(event) => Some(Self::Configure {
                window: wrap_xid(event.window),
                geometry: Rect {
                    x: i32::from(event.x),
                    y: i32::from(event.y),
                    width: i32::from(event.width),
                    height: i32::from(event.height),
                },
                above_sibling: nonzero_window(event.above_sibling).map(wrap_xid),
            }),
            Event::ReparentNotify(event) => Some(Self::Reparent {
                window: wrap_xid(event.window),
                parent: nonzero_window(event.parent).map(wrap_xid),
            }),
            Event::PropertyNotify(event) => Some(Self::Property {
                window: wrap_xid(event.window),
                atom: event.atom,
                deleted: u8::from(event.state) == 1,
            }),
            Event::CirculateNotify(event) => Some(Self::Restack {
                window: wrap_xid(event.window),
                place: RestackPlace::from_x11(event.place),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestackPlace {
    OnTop,
    OnBottom,
}

impl RestackPlace {
    fn from_x11(place: Place) -> Self {
        if u8::from(place) == u8::from(Place::ON_BOTTOM) {
            Self::OnBottom
        } else {
            Self::OnTop
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XBridgeError {
    Connect {
        message: String,
    },
    InvalidScreen {
        screen_num: usize,
    },
    QueryExtension {
        extension: RequiredExtension,
        message: String,
    },
    QueryTree {
        window: u32,
        message: String,
    },
    WindowAttributes {
        window: u32,
        message: String,
    },
    WindowGeometry {
        window: u32,
        message: String,
    },
    InternAtom {
        atom: String,
        message: String,
    },
    GetProperty {
        window: u32,
        property: u32,
        message: String,
    },
    PoliteClose {
        window: u32,
        message: String,
    },
    CompositeVersion {
        message: String,
    },
    CompositeRedirect {
        window: u32,
        message: String,
    },
    CompositeNamePixmap {
        window: u32,
        pixmap: u32,
        message: String,
    },
    GenerateId {
        message: String,
    },
    DamageVersion {
        message: String,
    },
    DamageCreate {
        window: u32,
        damage: u32,
        message: String,
    },
    PixmapGeometry {
        pixmap: u32,
        message: String,
    },
    PixmapReadback {
        pixmap: u32,
        message: String,
    },
    TestClient {
        message: String,
    },
    RoutedInput {
        message: String,
    },
    SelectionMonitor {
        message: String,
    },
}

impl fmt::Display for XBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { message } => write!(f, "failed to connect to X display: {message}"),
            Self::InvalidScreen { screen_num } => write!(f, "invalid X screen {screen_num}"),
            Self::QueryExtension { extension, message } => {
                write!(
                    f,
                    "failed to query {} extension: {message}",
                    extension.name()
                )
            }
            Self::QueryTree { window, message } => {
                write!(
                    f,
                    "failed to query X window tree for {window:#x}: {message}"
                )
            }
            Self::WindowAttributes { window, message } => {
                write!(
                    f,
                    "failed to query X window attributes for {window:#x}: {message}"
                )
            }
            Self::WindowGeometry { window, message } => {
                write!(
                    f,
                    "failed to query X window geometry for {window:#x}: {message}"
                )
            }
            Self::InternAtom { atom, message } => {
                write!(f, "failed to intern X atom {atom}: {message}")
            }
            Self::GetProperty {
                window,
                property,
                message,
            } => {
                write!(
                    f,
                    "failed to get X property {property:#x} from {window:#x}: {message}"
                )
            }
            Self::PoliteClose { window, message } => {
                write!(
                    f,
                    "failed to request polite close for {window:#x}: {message}"
                )
            }
            Self::CompositeVersion { message } => {
                write!(f, "failed to negotiate XComposite version: {message}")
            }
            Self::CompositeRedirect { window, message } => {
                write!(
                    f,
                    "failed to redirect X window {window:#x} with XComposite: {message}"
                )
            }
            Self::CompositeNamePixmap {
                window,
                pixmap,
                message,
            } => {
                write!(
                    f,
                    "failed to name XComposite pixmap {pixmap:#x} for X window {window:#x}: {message}"
                )
            }
            Self::GenerateId { message } => {
                write!(f, "failed to allocate an X resource ID: {message}")
            }
            Self::DamageVersion { message } => {
                write!(f, "failed to negotiate X Damage version: {message}")
            }
            Self::DamageCreate {
                window,
                damage,
                message,
            } => {
                write!(
                    f,
                    "failed to create X Damage object {damage:#x} for X window {window:#x}: {message}"
                )
            }
            Self::PixmapGeometry { pixmap, message } => {
                write!(
                    f,
                    "failed to query X pixmap geometry for {pixmap:#x}: {message}"
                )
            }
            Self::PixmapReadback { pixmap, message } => {
                write!(f, "failed to read X pixmap {pixmap:#x}: {message}")
            }
            Self::TestClient { message } => {
                write!(f, "failed to run Sophia X test client: {message}")
            }
            Self::RoutedInput { message } => {
                write!(f, "failed to run Sophia routed-input smoke: {message}")
            }
            Self::SelectionMonitor { message } => {
                write!(f, "failed to monitor X selections: {message}")
            }
        }
    }
}

impl std::error::Error for XBridgeError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequiredExtension {
    Composite,
    Damage,
    XFixes,
    Shape,
    Render,
}

impl RequiredExtension {
    pub const ALL: [Self; 5] = [
        Self::Composite,
        Self::Damage,
        Self::XFixes,
        Self::Shape,
        Self::Render,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Composite => "Composite",
            Self::Damage => "DAMAGE",
            Self::XFixes => "XFIXES",
            Self::Shape => "SHAPE",
            Self::Render => "RENDER",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStatus {
    pub extension: RequiredExtension,
    pub present: bool,
    pub major_opcode: Option<u8>,
    pub first_event: Option<u8>,
    pub first_error: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceRecord {
    pub namespace: NamespaceId,
    pub label: String,
    pub source: NamespaceSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespaceSource {
    StaticConfig,
    XServer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticNamespaceConfig {
    namespaces: Vec<NamespaceRecord>,
}

impl StaticNamespaceConfig {
    pub fn new(namespaces: Vec<NamespaceRecord>) -> Self {
        Self { namespaces }
    }

    pub fn namespaces(&self) -> &[NamespaceRecord] {
        &self.namespaces
    }

    pub fn record_namespace(&mut self, record: NamespaceRecord) {
        if let Some(existing) = self
            .namespaces
            .iter_mut()
            .find(|existing| existing.namespace == record.namespace)
        {
            *existing = record;
            return;
        }

        self.namespaces.push(record);
    }

    pub fn with_discovered(mut self, records: impl IntoIterator<Item = NamespaceRecord>) -> Self {
        for record in records {
            self.record_namespace(record);
        }

        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceOwnership {
    pub window: XWindowId,
    pub namespace: NamespaceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XConnectionProbe {
    pub display_name: Option<String>,
    pub screen_num: usize,
    pub required_extensions: Vec<ExtensionStatus>,
    pub namespaces: StaticNamespaceConfig,
}

impl XConnectionProbe {
    pub fn missing_extensions(&self) -> Vec<RequiredExtension> {
        self.required_extensions
            .iter()
            .filter(|status| !status.present)
            .map(|status| status.extension)
            .collect()
    }

    pub fn has_required_extensions(&self) -> bool {
        self.missing_extensions().is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRootImport {
    pub probe: XConnectionProbe,
    pub mirror: XMirrorState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestClientConfig {
    pub display_name: Option<String>,
    pub size: Size,
    pub hold_millis: u64,
}

impl Default for TestClientConfig {
    fn default() -> Self {
        Self {
            display_name: None,
            size: Size {
                width: 320,
                height: 200,
            },
            hold_millis: 5_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestClientWindow {
    pub window: XWindowId,
    pub size: Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeReadbackReport {
    pub display_name: Option<String>,
    pub mirrored_windows: usize,
    pub surfaces: usize,
    pub renderable_layers: usize,
    pub redirect_targets: usize,
    pub readbacks: usize,
    pub total_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeReadbackCapture {
    pub report: SmokeReadbackReport,
    pub surfaces: Vec<SurfaceSnapshot>,
    pub layers: Vec<LayerSnapshot>,
    pub readbacks: Vec<CpuBufferSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAtoms {
    pub wm_state: Atom,
    pub net_client_list: Atom,
    pub wm_protocols: Atom,
    pub wm_delete_window: Atom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionAtoms {
    pub primary: Atom,
    pub secondary: Atom,
    pub clipboard: Atom,
}

impl XSelectionAtoms {
    pub const fn all(self) -> [Atom; 3] {
        [self.primary, self.secondary, self.clipboard]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XSelectionChangeKind {
    SetOwner,
    OwnerWindowDestroyed,
    OwnerClientClosed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionEvent {
    pub selection: Atom,
    pub owner: Option<XWindowId>,
    pub timestamp: u32,
    pub selection_timestamp: u32,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerRecord {
    pub selection: Atom,
    pub namespace: Option<NamespaceId>,
    pub owner: Option<XWindowId>,
    pub generation: u64,
    pub timestamp: u32,
    pub selection_timestamp: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerUpdate {
    pub previous: Option<XSelectionOwnerRecord>,
    pub current: XSelectionOwnerRecord,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardPortalOwnerChange {
    pub source_namespace: NamespaceId,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionPortalRequest {
    pub request: ClipboardTransferRequest,
    pub failure: ClipboardSelectionFailureRequest,
    pub property: Atom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionRequestError {
    UnknownRequestorNamespace,
    UnknownSourceOwner,
    MissingSourceNamespace,
    SameNamespace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionDispatch {
    pub portal_request: ClipboardSelectionPortalRequest,
    pub command: PortalCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionDispatchError {
    NotSelectionRequest,
    Request(ClipboardSelectionRequestError),
    Portal(PortalError),
}

pub fn dispatch_clipboard_selection_request_event(
    event: &Event,
    target_name: impl Into<String>,
    monitor: &XSelectionMonitor,
    mirror: &XMirrorState,
    transfer: PortalTransferId,
    portal: &mut ClipboardPortal,
) -> Result<ClipboardSelectionDispatch, ClipboardSelectionDispatchError> {
    let Event::SelectionRequest(event) = event else {
        return Err(ClipboardSelectionDispatchError::NotSelectionRequest);
    };
    let portal_request = clipboard_portal_request_from_selection_request(
        event,
        target_name,
        monitor,
        mirror,
        transfer,
    )
    .map_err(ClipboardSelectionDispatchError::Request)?;
    let command = portal
        .request_import(portal_request.request.clone())
        .map_err(ClipboardSelectionDispatchError::Portal)?;

    Ok(ClipboardSelectionDispatch {
        portal_request,
        command,
    })
}

pub fn clipboard_portal_request_from_selection_request(
    event: &SelectionRequestEvent,
    target_name: impl Into<String>,
    monitor: &XSelectionMonitor,
    mirror: &XMirrorState,
    transfer: PortalTransferId,
) -> Result<ClipboardSelectionPortalRequest, ClipboardSelectionRequestError> {
    let target_namespace = mirror
        .namespace_for_window(wrap_xid(event.requestor))
        .ok_or(ClipboardSelectionRequestError::UnknownRequestorNamespace)?;
    let source_owner = monitor
        .current_owner_for_selection(event.selection)
        .ok_or(ClipboardSelectionRequestError::UnknownSourceOwner)?;
    let source_namespace = source_owner
        .namespace
        .ok_or(ClipboardSelectionRequestError::MissingSourceNamespace)?;

    if source_namespace == target_namespace {
        return Err(ClipboardSelectionRequestError::SameNamespace);
    }

    Ok(ClipboardSelectionPortalRequest {
        request: ClipboardTransferRequest {
            transfer,
            source_namespace,
            target_namespace,
            target: ClipboardTarget::Atom(target_name.into()),
            byte_size: 0,
            generation: source_owner.generation,
        },
        failure: ClipboardSelectionFailureRequest {
            transfer,
            requestor: event.requestor,
            selection: event.selection,
            target: event.target,
            time: event.time,
        },
        property: event.property,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionFailureRequest {
    pub transfer: PortalTransferId,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub time: Timestamp,
}

pub const MAX_CLIPBOARD_TEXT_HANDOFF_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardTextProperty {
    pub requestor: Window,
    pub property: Atom,
    pub target: Atom,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ClipboardSelectionHandoff {
    pub transfer: PortalTransferId,
    pub property: ClipboardTextProperty,
    pub event: SelectionNotifyEvent,
}

impl ClipboardSelectionHandoff {
    pub fn succeeded_normally(&self) -> bool {
        self.event.property == self.property.property
            && self.event.property != u32::from(AtomEnum::NONE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionHandoffError {
    NotHandoffCommand,
    TransferMismatch,
    MissingProperty,
    UnsupportedTarget,
    TextTooLarge { len: usize, max: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveClipboardPortalSmokeReport {
    pub display_name: Option<String>,
    pub owner: XWindowId,
    pub requestor: XWindowId,
    pub selection: Atom,
    pub target: Atom,
    pub denied_property: Atom,
    pub approved_property: Atom,
    pub failure_property: Atom,
    pub success_property: Atom,
    pub handoff_bytes: usize,
    pub observed_handoff_bytes: usize,
}

pub fn clipboard_selection_text_handoff_notify(
    command: &PortalCommand,
    request: &ClipboardSelectionPortalRequest,
    text: impl AsRef<str>,
) -> Result<ClipboardSelectionHandoff, ClipboardSelectionHandoffError> {
    let PortalCommand::HandoffClipboard { transfer } = command else {
        return Err(ClipboardSelectionHandoffError::NotHandoffCommand);
    };

    if *transfer != request.request.transfer {
        return Err(ClipboardSelectionHandoffError::TransferMismatch);
    }

    if request.property == u32::from(AtomEnum::NONE) {
        return Err(ClipboardSelectionHandoffError::MissingProperty);
    }

    if !request.request.target.is_text() {
        return Err(ClipboardSelectionHandoffError::UnsupportedTarget);
    }

    let bytes = text.as_ref().as_bytes();
    if bytes.len() > MAX_CLIPBOARD_TEXT_HANDOFF_BYTES {
        return Err(ClipboardSelectionHandoffError::TextTooLarge {
            len: bytes.len(),
            max: MAX_CLIPBOARD_TEXT_HANDOFF_BYTES,
        });
    }

    let bytes = bytes.to_vec();
    let failure = request.failure;
    Ok(ClipboardSelectionHandoff {
        transfer: *transfer,
        property: ClipboardTextProperty {
            requestor: failure.requestor,
            property: request.property,
            target: failure.target,
            bytes,
        },
        event: SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: failure.time,
            requestor: failure.requestor,
            selection: failure.selection,
            target: failure.target,
            property: request.property,
        },
    })
}

pub fn smoke_live_clipboard_portal(
    display_name: Option<&str>,
) -> Result<LiveClipboardPortalSmokeReport, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let screen = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?;
    let owner = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let requestor = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let selection = intern_atom(&connection, "CLIPBOARD")?;
    let target = intern_atom(&connection, "UTF8_STRING")?;
    let denied_property = intern_atom(&connection, "SOPHIA_CLIPBOARD_DENIED")?;
    let approved_property = intern_atom(&connection, "SOPHIA_CLIPBOARD_APPROVED")?;

    create_clipboard_smoke_window(
        &connection,
        screen.root,
        screen.root_depth,
        screen.root_visual,
        owner,
    )?;
    create_clipboard_smoke_window(
        &connection,
        screen.root,
        screen.root_depth,
        screen.root_visual,
        requestor,
    )?;
    connection
        .set_selection_owner(owner, selection, 0u32)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    let current_owner = connection
        .get_selection_owner(selection)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .owner;
    if current_owner != owner {
        return Err(XBridgeError::SelectionMonitor {
            message: format!("selection owner is {current_owner:#x}, expected {owner:#x}"),
        });
    }
    drain_pending_events(&connection)?;

    let source_namespace = NamespaceId::from_raw(10);
    let target_namespace = NamespaceId::from_raw(20);
    let owner_window = XWindowId::new(owner, 1);
    let requestor_window = XWindowId::new(requestor, 1);
    let mut mirror = XMirrorState::default();
    mirror.ingest_window(clipboard_smoke_mirror(owner_window, source_namespace));
    mirror.ingest_window(clipboard_smoke_mirror(requestor_window, target_namespace));
    let mut monitor = XSelectionMonitor::new();
    let update = monitor.apply_event(
        XSelectionEvent {
            selection,
            owner: Some(owner_window),
            timestamp: 1,
            selection_timestamp: 1,
            kind: XSelectionChangeKind::SetOwner,
        },
        &mirror,
    );

    connection
        .convert_selection(requestor, selection, target, denied_property, 0u32)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    let deny_request = wait_for_selection_request(
        &connection,
        requestor,
        selection,
        denied_property,
        Duration::from_secs(2),
    )?;
    let mut portal = ClipboardPortal::new();
    let deny_dispatch = dispatch_clipboard_selection_request_event(
        &Event::SelectionRequest(deny_request),
        "UTF8_STRING",
        &monitor,
        &mirror,
        PortalTransferId::from_raw(1),
        &mut portal,
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to dispatch denied selection request: {error:?}"),
    })?;
    let PortalCommand::FailSelection { transfer } = portal
        .deny(PortalTransferId::from_raw(1))
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: format!("failed to deny clipboard request: {error:?}"),
        })?
    else {
        return Err(XBridgeError::SelectionMonitor {
            message: "clipboard denial did not return FailSelection".to_owned(),
        });
    };
    let failure = clipboard_selection_failure_notify(deny_dispatch.portal_request.failure);
    if failure.transfer != transfer {
        return Err(XBridgeError::SelectionMonitor {
            message: "clipboard denial returned mismatched transfer".to_owned(),
        });
    }
    apply_clipboard_selection_failure(&connection, &failure)?;
    let failure_notify = wait_for_selection_notify(
        &connection,
        requestor,
        selection,
        target,
        Duration::from_secs(2),
    )?;
    if failure_notify.property != u32::from(AtomEnum::NONE) {
        return Err(XBridgeError::SelectionMonitor {
            message: format!(
                "denied clipboard request returned property {:#x}, expected None",
                failure_notify.property
            ),
        });
    }

    connection
        .convert_selection(requestor, selection, target, approved_property, 0u32)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    let approve_request = wait_for_selection_request(
        &connection,
        requestor,
        selection,
        approved_property,
        Duration::from_secs(2),
    )?;
    let approve_dispatch = dispatch_clipboard_selection_request_event(
        &Event::SelectionRequest(approve_request),
        "UTF8_STRING",
        &monitor,
        &mirror,
        PortalTransferId::from_raw(2),
        &mut portal,
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to dispatch approved selection request: {error:?}"),
    })?;
    let command = portal
        .approve_generation(PortalTransferId::from_raw(2), update.current.generation)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: format!("failed to approve clipboard request: {error:?}"),
        })?;
    let handoff = clipboard_selection_text_handoff_notify(
        &command,
        &approve_dispatch.portal_request,
        "hello from Sophia",
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to build clipboard handoff: {error:?}"),
    })?;
    apply_clipboard_selection_handoff(&connection, &handoff)?;
    let success_notify = wait_for_selection_notify(
        &connection,
        requestor,
        selection,
        target,
        Duration::from_secs(2),
    )?;
    if success_notify.property != approved_property {
        return Err(XBridgeError::SelectionMonitor {
            message: format!(
                "approved clipboard request returned property {:#x}, expected {approved_property:#x}",
                success_notify.property
            ),
        });
    }
    let observed = connection
        .get_property(
            false,
            requestor,
            approved_property,
            target,
            0,
            (MAX_CLIPBOARD_TEXT_HANDOFF_BYTES / 4) as u32,
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    if observed.format != 8 || observed.value != handoff.property.bytes {
        return Err(XBridgeError::SelectionMonitor {
            message: "approved clipboard property did not contain expected text bytes".to_owned(),
        });
    }

    Ok(LiveClipboardPortalSmokeReport {
        display_name: display_name.map(str::to_owned),
        owner: owner_window,
        requestor: requestor_window,
        selection,
        target,
        denied_property,
        approved_property,
        failure_property: failure_notify.property,
        success_property: success_notify.property,
        handoff_bytes: handoff.property.bytes.len(),
        observed_handoff_bytes: observed.value.len(),
    })
}

pub fn apply_clipboard_selection_handoff<C>(
    connection: &C,
    handoff: &ClipboardSelectionHandoff,
) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    connection
        .change_property8(
            PropMode::REPLACE,
            handoff.property.requestor,
            handoff.property.property,
            handoff.property.target,
            &handoff.property.bytes,
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    send_clipboard_selection_notify(connection, handoff.event)
}

#[derive(Clone, Copy, Debug)]
pub struct ClipboardSelectionFailure {
    pub transfer: PortalTransferId,
    pub event: SelectionNotifyEvent,
}

impl ClipboardSelectionFailure {
    pub fn failed_normally(&self) -> bool {
        self.event.property == u32::from(AtomEnum::NONE)
    }
}

pub fn clipboard_selection_failure_notify(
    request: ClipboardSelectionFailureRequest,
) -> ClipboardSelectionFailure {
    ClipboardSelectionFailure {
        transfer: request.transfer,
        event: SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: u32::from(AtomEnum::NONE),
        },
    }
}

pub fn apply_clipboard_selection_failure<C>(
    connection: &C,
    failure: &ClipboardSelectionFailure,
) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    send_clipboard_selection_notify(connection, failure.event)
}

fn send_clipboard_selection_notify<C>(
    connection: &C,
    event: SelectionNotifyEvent,
) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    connection
        .send_event(false, event.requestor, EventMask::NO_EVENT, event)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })
}

pub fn clipboard_portal_owner_change_from_selection_update(
    update: &XSelectionOwnerUpdate,
) -> Option<ClipboardPortalOwnerChange> {
    if update.kind == XSelectionChangeKind::Unknown {
        return None;
    }

    let source_namespace = update
        .current
        .namespace
        .or_else(|| update.previous.and_then(|record| record.namespace))?;

    Some(ClipboardPortalOwnerChange {
        source_namespace,
        generation: update.current.generation,
    })
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XSelectionMonitor {
    owners: BTreeMap<(Atom, Option<NamespaceId>), XSelectionOwnerRecord>,
}

impl XSelectionMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn owner(
        &self,
        selection: Atom,
        namespace: Option<NamespaceId>,
    ) -> Option<XSelectionOwnerRecord> {
        self.owners.get(&(selection, namespace)).copied()
    }

    pub fn current_owner_for_selection(&self, selection: Atom) -> Option<XSelectionOwnerRecord> {
        self.owners
            .values()
            .filter(|record| record.selection == selection && record.owner.is_some())
            .max_by_key(|record| record.generation)
            .copied()
    }

    pub fn apply_event(
        &mut self,
        event: XSelectionEvent,
        mirror: &XMirrorState,
    ) -> XSelectionOwnerUpdate {
        let namespace_from_owner = event
            .owner
            .and_then(|owner| mirror.namespace_for_window(owner));
        let namespace =
            namespace_from_owner.or_else(|| self.namespace_for_existing_selection(event.selection));
        let key = (event.selection, namespace);
        let previous = self.owners.get(&key).copied();
        let generation = previous
            .map(|record| record.generation.saturating_add(1))
            .unwrap_or(1);
        let current = XSelectionOwnerRecord {
            selection: event.selection,
            namespace,
            owner: event.owner,
            generation,
            timestamp: event.timestamp,
            selection_timestamp: event.selection_timestamp,
        };

        self.owners.insert(key, current);

        XSelectionOwnerUpdate {
            previous,
            current,
            kind: event.kind,
        }
    }

    fn namespace_for_existing_selection(&self, selection: Atom) -> Option<NamespaceId> {
        self.owners
            .iter()
            .find_map(|((record_selection, namespace), record)| {
                if *record_selection == selection && record.owner.is_some() {
                    *namespace
                } else {
                    None
                }
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoliteCloseOutcome {
    SentDeleteWindow { window: XWindowId },
    UnsupportedProtocol { window: XWindowId },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XClientHints {
    pub ewmh_clients: Vec<XWindowId>,
    pub icccm_clients: Vec<XWindowId>,
}

pub fn probe_display(
    display_name: Option<&str>,
    namespaces: StaticNamespaceConfig,
) -> Result<XConnectionProbe, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let required_extensions = query_required_extensions(&connection)?;

    Ok(XConnectionProbe {
        display_name: display_name.map(str::to_owned),
        screen_num,
        required_extensions,
        namespaces,
    })
}

pub fn import_root_window_tree(
    display_name: Option<&str>,
    namespaces: StaticNamespaceConfig,
) -> Result<XRootImport, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let required_extensions = query_required_extensions(&connection)?;
    let mut mirror = import_root_window_tree_from_connection(&connection, screen_num)?;
    let atoms = intern_client_hint_atoms(&connection)?;
    let hints = detect_client_hints(&connection, screen_num, &mirror, atoms)?;
    mirror.apply_client_hints(&hints);

    Ok(XRootImport {
        probe: XConnectionProbe {
            display_name: display_name.map(str::to_owned),
            screen_num,
            required_extensions,
            namespaces,
        },
        mirror,
    })
}

pub fn run_test_client_window(config: TestClientConfig) -> Result<TestClientWindow, XBridgeError> {
    let width = u16::try_from(config.size.width.max(1)).unwrap_or(u16::MAX);
    let height = u16::try_from(config.size.height.max(1)).unwrap_or(u16::MAX);
    let (connection, screen_num) =
        x11rb::connect(config.display_name.as_deref()).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let screen = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?;
    let window = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let gc = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let window_aux = CreateWindowAux::new()
        .background_pixel(screen.white_pixel)
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY);

    connection
        .create_window(
            screen.root_depth,
            window,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &window_aux,
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .create_gc(
            gc,
            window,
            &CreateGCAux::new()
                .foreground(screen.black_pixel)
                .background(screen.white_pixel),
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .map_window(window)
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .poly_fill_rectangle(
            window,
            gc,
            &[Rectangle {
                x: 24,
                y: 24,
                width: width.saturating_sub(48),
                height: height.saturating_sub(48),
            }],
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;

    thread::sleep(Duration::from_millis(config.hold_millis));

    Ok(TestClientWindow {
        window: wrap_xid(window),
        size: Size {
            width: i32::from(width),
            height: i32::from(height),
        },
    })
}

pub fn smoke_readback_display(
    display_name: Option<&str>,
) -> Result<SmokeReadbackReport, XBridgeError> {
    capture_readback_display(display_name).map(|capture| capture.report)
}

pub fn capture_readback_display(
    display_name: Option<&str>,
) -> Result<SmokeReadbackCapture, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let mut mirror = import_root_window_tree_from_connection(&connection, screen_num)?;
    let atoms = intern_client_hint_atoms(&connection)?;
    let hints = detect_client_hints(&connection, screen_num, &mirror, atoms)?;
    mirror.apply_client_hints(&hints);
    mirror.apply_unmanaged_client_fallback();

    let targets = mirror.composite_redirect_targets();
    redirect_composite_targets(&connection, &targets)?;

    let mut pixmaps = CompositePixmapMap::default();
    name_composite_pixmaps(&connection, &targets, &mut pixmaps)?;

    let mut surface_ids = SurfaceIdMap::default();
    let mut surfaces = mirror.emit_surfaces(&mut surface_ids, &pixmaps);
    let mut buffers = CpuBufferStore::default();
    let readbacks = readback_surface_pixmaps(&connection, &mut surfaces, &mut buffers)?;
    let layers = layers_from_surfaces(&surfaces);
    let total_bytes = readbacks
        .iter()
        .map(|readback| readback.bytes.len())
        .sum::<usize>();

    Ok(SmokeReadbackCapture {
        report: SmokeReadbackReport {
            display_name: display_name.map(str::to_owned),
            mirrored_windows: mirror.windows().len(),
            surfaces: surfaces.len(),
            renderable_layers: layers.len(),
            redirect_targets: targets.len(),
            readbacks: readbacks.len(),
            total_bytes,
        },
        surfaces,
        layers,
        readbacks,
    })
}

pub fn redirect_composite_targets<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    connection
        .composite_query_version(0, 4)
        .map_err(|error| XBridgeError::CompositeVersion {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::CompositeVersion {
            message: error.to_string(),
        })?;

    for target in targets {
        connection
            .composite_redirect_window(target.window.xid(), target.update.to_x11())
            .map_err(|error| XBridgeError::CompositeRedirect {
                window: target.window.xid(),
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::CompositeRedirect {
                window: target.window.xid(),
                message: error.to_string(),
            })?;
    }

    Ok(())
}

pub fn name_composite_pixmaps<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
    pixmaps: &mut CompositePixmapMap,
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    for target in targets {
        if pixmaps.pixmap_for_window(target.window).is_some() {
            continue;
        }

        let pixmap = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;

        connection
            .composite_name_window_pixmap(target.window.xid(), pixmap)
            .map_err(|error| XBridgeError::CompositeNamePixmap {
                window: target.window.xid(),
                pixmap,
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::CompositeNamePixmap {
                window: target.window.xid(),
                pixmap,
                message: error.to_string(),
            })?;

        pixmaps.insert_named_pixmap(target.window, pixmap);
    }

    Ok(())
}

pub fn create_damage_trackers<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
    tracker: &mut DamageTracker,
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    connection
        .damage_query_version(1, 1)
        .map_err(|error| XBridgeError::DamageVersion {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::DamageVersion {
            message: error.to_string(),
        })?;

    for target in targets {
        if tracker.damage_for_window(target.window).is_some() {
            continue;
        }

        let damage = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;

        connection
            .damage_create(damage, target.window.xid(), ReportLevel::BOUNDING_BOX)
            .map_err(|error| XBridgeError::DamageCreate {
                window: target.window.xid(),
                damage,
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::DamageCreate {
                window: target.window.xid(),
                damage,
                message: error.to_string(),
            })?;

        tracker.insert_damage(target.window, damage);
    }

    Ok(())
}

pub fn emit_damage_frame(
    tracker: &mut DamageTracker,
    output: OutputId,
    frame_serial: u64,
    buffer_age: u32,
    root_generation: u64,
    surfaces: &[SurfaceSnapshot],
) -> DamageFrame {
    let mut affected_surfaces = Vec::new();
    let mut seen_surfaces = BTreeSet::new();
    let mut damage = Region::empty();

    for surface in surfaces {
        let Some(client) = surface.client else {
            continue;
        };

        let local_damage = tracker.drain_damage(client);
        if local_damage.is_empty() || !surface.mapped {
            continue;
        }

        let translated = translate_region(&local_damage, surface.geometry.x, surface.geometry.y);
        if translated.is_empty() {
            continue;
        }

        if seen_surfaces.insert(surface.surface) {
            affected_surfaces.push(surface.surface);
        }
        damage.extend(&translated);
    }

    DamageFrame {
        output,
        frame_serial,
        buffer_age,
        root_generation,
        affected_surfaces,
        damage,
    }
}

pub fn readback_composite_pixmap<C>(
    connection: &C,
    pixmap: u32,
    buffers: &mut CpuBufferStore,
) -> Result<CpuBufferSnapshot, XBridgeError>
where
    C: Connection,
{
    let geometry = connection
        .get_geometry(pixmap)
        .map_err(|error| XBridgeError::PixmapGeometry {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapGeometry {
            pixmap,
            message: error.to_string(),
        })?;
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            pixmap,
            0,
            0,
            geometry.width,
            geometry.height,
            u32::MAX,
        )
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?;

    Ok(buffers.upsert_pixmap(
        pixmap,
        Size {
            width: i32::from(geometry.width),
            height: i32::from(geometry.height),
        },
        image.depth,
        image.visual,
        image.data,
    ))
}

pub fn readback_surface_pixmaps<C>(
    connection: &C,
    surfaces: &mut [SurfaceSnapshot],
    buffers: &mut CpuBufferStore,
) -> Result<Vec<CpuBufferSnapshot>, XBridgeError>
where
    C: Connection,
{
    let mut readbacks = Vec::new();

    for surface in surfaces {
        let BufferSource::XPixmap { pixmap } = surface.source else {
            continue;
        };
        let readback = readback_composite_pixmap(connection, pixmap, buffers)?;
        surface.source = BufferSource::CpuBuffer {
            handle: readback.handle,
        };
        readbacks.push(readback);
    }

    Ok(readbacks)
}

pub fn layers_from_surfaces(surfaces: &[SurfaceSnapshot]) -> Vec<LayerSnapshot> {
    surfaces
        .iter()
        .filter(|surface| surface.mapped && !surface.geometry.is_empty())
        .map(|surface| LayerSnapshot {
            surface: surface.surface,
            window: Some(surface.window),
            namespace: surface.namespace,
            stack_rank: surface.stack_rank,
            geometry: surface.geometry,
            source: surface.source,
            damage: surface.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: surface.generation,
        })
        .collect()
}

fn translate_region(region: &Region, dx: i32, dy: i32) -> Region {
    let mut translated = Region::empty();
    for rect in &region.rects {
        translated.push(Rect {
            x: rect.x.saturating_add(dx),
            y: rect.y.saturating_add(dy),
            width: rect.width,
            height: rect.height,
        });
    }
    translated
}

fn query_required_extensions<C>(connection: &C) -> Result<Vec<ExtensionStatus>, XBridgeError>
where
    C: Connection,
{
    let mut required_extensions = Vec::with_capacity(RequiredExtension::ALL.len());

    for extension in RequiredExtension::ALL {
        let reply = connection
            .query_extension(extension.name().as_bytes())
            .map_err(|error| XBridgeError::QueryExtension {
                extension,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::QueryExtension {
                extension,
                message: error.to_string(),
            })?;

        required_extensions.push(ExtensionStatus {
            extension,
            present: reply.present,
            major_opcode: reply.present.then_some(reply.major_opcode),
            first_event: reply.present.then_some(reply.first_event),
            first_error: reply.present.then_some(reply.first_error),
        });
    }

    Ok(required_extensions)
}

fn intern_client_hint_atoms<C>(connection: &C) -> Result<XAtoms, XBridgeError>
where
    C: Connection,
{
    Ok(XAtoms {
        wm_state: intern_atom(connection, "WM_STATE")?,
        net_client_list: intern_atom(connection, "_NET_CLIENT_LIST")?,
        wm_protocols: intern_atom(connection, "WM_PROTOCOLS")?,
        wm_delete_window: intern_atom(connection, "WM_DELETE_WINDOW")?,
    })
}

pub fn intern_selection_atoms<C>(connection: &C) -> Result<XSelectionAtoms, XBridgeError>
where
    C: Connection,
{
    Ok(XSelectionAtoms {
        primary: intern_atom(connection, "PRIMARY")?,
        secondary: intern_atom(connection, "SECONDARY")?,
        clipboard: intern_atom(connection, "CLIPBOARD")?,
    })
}

pub fn select_selection_owner_events<C>(
    connection: &C,
    window: Window,
    selections: &[Atom],
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    let mask = SelectionEventMask::SET_SELECTION_OWNER
        | SelectionEventMask::SELECTION_WINDOW_DESTROY
        | SelectionEventMask::SELECTION_CLIENT_CLOSE;

    for selection in selections {
        connection
            .xfixes_select_selection_input(window, *selection, mask)
            .map_err(|error| XBridgeError::SelectionMonitor {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::SelectionMonitor {
                message: error.to_string(),
            })?;
    }

    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;

    Ok(())
}

fn intern_atom<C>(connection: &C, name: &str) -> Result<Atom, XBridgeError>
where
    C: Connection,
{
    connection
        .intern_atom(false, name.as_bytes())
        .map_err(|error| XBridgeError::InternAtom {
            atom: name.to_owned(),
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| XBridgeError::InternAtom {
            atom: name.to_owned(),
            message: error.to_string(),
        })
}

fn detect_client_hints<C>(
    connection: &C,
    screen_num: usize,
    mirror: &XMirrorState,
    atoms: XAtoms,
) -> Result<XClientHints, XBridgeError>
where
    C: Connection,
{
    let root = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?
        .root;
    let ewmh_clients = read_window_list_property(connection, root, atoms.net_client_list)?
        .into_iter()
        .map(wrap_xid)
        .collect();
    let mut icccm_clients = Vec::new();

    for mirror in mirror.windows() {
        if has_property(connection, mirror.window.xid(), atoms.wm_state)? {
            icccm_clients.push(mirror.window);
        }
    }

    Ok(XClientHints {
        ewmh_clients,
        icccm_clients,
    })
}

fn read_window_list_property<C>(
    connection: &C,
    window: Window,
    property: Atom,
) -> Result<Vec<Window>, XBridgeError>
where
    C: Connection,
{
    let reply = connection
        .get_property(false, window, property, AtomEnum::WINDOW, 0, u32::MAX / 4)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?;

    Ok(reply
        .value32()
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default())
}

fn has_property<C>(connection: &C, window: Window, property: Atom) -> Result<bool, XBridgeError>
where
    C: Connection,
{
    connection
        .get_property(false, window, property, AtomEnum::ANY, 0, 0)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.type_ != 0)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })
}

pub fn polite_close_surface<C>(
    connection: &C,
    mirror: &XMirrorState,
    surfaces: &SurfaceIdMap,
    atoms: XAtoms,
    surface: SurfaceId,
    timestamp: u32,
) -> Result<PoliteCloseOutcome, XBridgeError>
where
    C: Connection,
{
    let target = close_target_for_surface(mirror, surfaces, surface).ok_or_else(|| {
        XBridgeError::PoliteClose {
            window: 0,
            message: format!("surface {:?} has no X close target", surface),
        }
    })?;

    polite_close_window(connection, target, atoms, timestamp)
}

pub fn polite_close_window<C>(
    connection: &C,
    window: XWindowId,
    atoms: XAtoms,
    timestamp: u32,
) -> Result<PoliteCloseOutcome, XBridgeError>
where
    C: Connection,
{
    if !window_supports_wm_delete(connection, window, atoms)? {
        return Ok(PoliteCloseOutcome::UnsupportedProtocol { window });
    }

    let event = build_wm_delete_client_message(window, atoms, timestamp);
    connection
        .send_event(false, window.xid(), EventMask::NO_EVENT, event)
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?;

    Ok(PoliteCloseOutcome::SentDeleteWindow { window })
}

pub fn build_wm_delete_client_message(
    window: XWindowId,
    atoms: XAtoms,
    timestamp: u32,
) -> ClientMessageEvent {
    ClientMessageEvent::new(
        32,
        window.xid(),
        atoms.wm_protocols,
        ClientMessageData::from([atoms.wm_delete_window, timestamp, 0, 0, 0]),
    )
}

fn window_supports_wm_delete<C>(
    connection: &C,
    window: XWindowId,
    atoms: XAtoms,
) -> Result<bool, XBridgeError>
where
    C: Connection,
{
    Ok(
        read_atom_list_property(connection, window.xid(), atoms.wm_protocols)?
            .into_iter()
            .any(|atom| atom == atoms.wm_delete_window),
    )
}

fn read_atom_list_property<C>(
    connection: &C,
    window: Window,
    property: Atom,
) -> Result<Vec<Atom>, XBridgeError>
where
    C: Connection,
{
    let reply = connection
        .get_property(false, window, property, AtomEnum::ATOM, 0, u32::MAX / 4)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?;

    Ok(reply
        .value32()
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default())
}

fn import_root_window_tree_from_connection<C>(
    connection: &C,
    screen_num: usize,
) -> Result<XMirrorState, XBridgeError>
where
    C: Connection,
{
    let root = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?
        .root;
    let mut queue = VecDeque::from([(root, None, 0)]);
    let mut visited = BTreeSet::new();
    let mut mirror = XMirrorState::default();

    while let Some((window, parent, stack_rank)) = queue.pop_front() {
        if !visited.insert(window) {
            continue;
        }

        let tree = connection
            .query_tree(window)
            .map_err(|error| XBridgeError::QueryTree {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::QueryTree {
                window,
                message: error.to_string(),
            })?;
        let attributes = connection
            .get_window_attributes(window)
            .map_err(|error| XBridgeError::WindowAttributes {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::WindowAttributes {
                window,
                message: error.to_string(),
            })?;
        let geometry = connection
            .get_geometry(window)
            .map_err(|error| XBridgeError::WindowGeometry {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::WindowGeometry {
                window,
                message: error.to_string(),
            })?;

        for (rank, child) in tree.children.iter().copied().enumerate() {
            let rank = u32::try_from(rank).expect("X child stack rank overflow");
            queue.push_back((child, Some(window), rank));
        }

        mirror.ingest_window(XWindowMirror {
            window: wrap_xid(window),
            parent: parent.map(wrap_xid),
            children: tree.children.iter().copied().map(wrap_xid).collect(),
            toplevel: None,
            client: None,
            mapped: u8::from(attributes.map_state) == u8::from(MapState::VIEWABLE),
            stack_rank,
            geometry: Rect {
                x: i32::from(geometry.x),
                y: i32::from(geometry.y),
                width: i32::from(geometry.width),
                height: i32::from(geometry.height),
            },
            namespace: None,
            stale_metadata: 0,
        });
    }

    Ok(mirror)
}

fn wrap_xid(window: Window) -> XWindowId {
    XWindowId::new(window, 1)
}

fn nonzero_window(window: Window) -> Option<Window> {
    (window != 0).then_some(window)
}

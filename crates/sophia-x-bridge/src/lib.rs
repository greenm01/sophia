use core::fmt;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::thread;
use std::time::Duration;
use std::{io::IoSlice, time::Instant};

use sophia_protocol::{
    BufferSource, DamageFrame, DeviceId, InputEventKind, InputEventPacket, InputRoute,
    InputRouteOutcome, LayerSnapshot, NamespaceId, OutputId, Point, Rect, Region, SeatId, Size,
    SurfaceId, SurfaceSnapshot, Transform, XLIBRE_ROUTED_INPUT_EXTENSION_NAME,
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH, XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE,
    XLibreRoutedInputDecision, XLibreRoutedInputOutcome, XLibreRoutedInputRequest,
    XLibreRoutedInputWireRequest, XWindowId, XWindowMirror,
};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::errors::ParseError;
use x11rb::protocol::Event;
use x11rb::protocol::composite::{ConnectionExt as CompositeConnectionExt, Redirect};
use x11rb::protocol::damage::{ConnectionExt as DamageConnectionExt, ReportLevel};
use x11rb::protocol::xinput::{
    ConnectionExt as XInputConnectionExt, Device, DeviceType, XIDeviceInfo,
};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, ImageFormat,
    MapState, Place, Rectangle, Window, WindowClass,
};
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
    UnsupportedTransform,
}

pub fn build_flat_routed_input_request(
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

    if route.transform != Transform::IDENTITY {
        return Err(RoutedInputAdapterError::UnsupportedTransform);
    }

    let target_window = route
        .target_window
        .filter(|window| window.is_valid())
        .ok_or(RoutedInputAdapterError::MissingTargetWindow)?;
    let local_position = route
        .local_position
        .ok_or(RoutedInputAdapterError::MissingLocalPosition)?;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInputSmokeReport {
    pub display_name: Option<String>,
    pub extension_opcode: u8,
    pub target_window: XWindowId,
    pub device: DeviceId,
    pub decision: XLibreRoutedInputDecision,
    pub event_x: i16,
    pub event_y: i16,
    pub button: u8,
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
    let local_x = 42;
    let local_y = 37;
    let button = 1;
    let serial = 0x534f_5048_4941_0001;

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

    let request = XLibreRoutedInputRequest {
        serial,
        seat: SeatId::from_raw(1),
        device,
        time_msec: 1,
        target_window: XWindowId::new(target, 1),
        local_position: Point {
            x: f64::from(local_x),
            y: f64::from(local_y),
        },
        kind: InputEventKind::PointerButton {
            button: u32::from(button),
            pressed: true,
        },
    };
    let reply = send_sophia_routed_input_route(&connection, routed_info.major_opcode, &request)?;
    let decision = XLibreRoutedInputDecision {
        serial: reply.serial,
        target_window: reply.target_window,
        outcome: reply.outcome,
    };
    if !routed_input_decision_allows_delivery(&decision) {
        return Err(XBridgeError::RoutedInput {
            message: format!("routed input rejected with {:?}", decision.outcome),
        });
    }

    let (event_x, event_y, observed_button) =
        wait_for_routed_button_press(&connection, target, Duration::from_secs(2))?;

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
        extension_opcode: routed_info.major_opcode,
        target_window: XWindowId::new(target, 1),
        device,
        decision,
        event_x,
        event_y,
        button: observed_button,
    })
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
) -> Result<SophiaRoutedInputRouteReply, XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let wire = request.to_wire_request();
    let mut bytes = Vec::with_capacity(usize::from(XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH) * 4);
    major_opcode.serialize_into(&mut bytes);
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE.serialize_into(&mut bytes);
    XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH.serialize_into(&mut bytes);
    serialize_routed_input_wire(&wire, &mut bytes);

    let cookie = connection
        .send_request_with_reply::<SophiaRoutedInputRouteReply>(&[IoSlice::new(&bytes)], Vec::new())
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?;
    cookie.reply().map_err(|error| XBridgeError::RoutedInput {
        message: error.to_string(),
    })
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
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompositePixmapMap {
    pixmaps: BTreeMap<XWindowId, u32>,
}

impl CompositePixmapMap {
    pub fn pixmap_for_window(&self, window: XWindowId) -> Option<u32> {
        self.pixmaps.get(&window).copied()
    }

    pub fn insert_named_pixmap(&mut self, window: XWindowId, pixmap: u32) -> Option<u32> {
        self.pixmaps.insert(window, pixmap)
    }

    pub fn remove_window(&mut self, window: XWindowId) -> Option<u32> {
        self.pixmaps.remove(&window)
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
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use sophia_protocol::{DeviceId, InputEventKind, Point, SeatId};

    fn status(extension: RequiredExtension, present: bool) -> ExtensionStatus {
        ExtensionStatus {
            extension,
            present,
            major_opcode: present.then_some(128),
            first_event: present.then_some(64),
            first_error: present.then_some(32),
        }
    }

    #[test]
    fn probe_reports_missing_required_extensions() {
        let probe = XConnectionProbe {
            display_name: Some(":99".to_owned()),
            screen_num: 0,
            required_extensions: vec![
                status(RequiredExtension::Composite, true),
                status(RequiredExtension::Damage, false),
            ],
            namespaces: StaticNamespaceConfig::default(),
        };

        assert_eq!(probe.missing_extensions(), vec![RequiredExtension::Damage]);
        assert!(!probe.has_required_extensions());
    }

    #[test]
    fn static_namespace_config_records_known_namespaces() {
        let config = StaticNamespaceConfig::new(vec![NamespaceRecord {
            namespace: NamespaceId::from_raw(1),
            label: "trusted".to_owned(),
            source: NamespaceSource::StaticConfig,
        }]);

        assert_eq!(config.namespaces().len(), 1);
        assert_eq!(config.namespaces()[0].label, "trusted");
        assert_eq!(config.namespaces()[0].source, NamespaceSource::StaticConfig);
    }

    #[test]
    fn test_client_config_has_bounded_defaults() {
        let config = TestClientConfig::default();

        assert!(config.size.width > 0);
        assert!(config.size.height > 0);
        assert!(config.hold_millis > 0);
    }

    #[test]
    fn wraps_imported_xids_with_initial_generation() {
        assert_eq!(wrap_xid(0x1200042), XWindowId::new(0x1200042, 1));
    }

    fn mirror(window: u32, parent: Option<u32>, stack_rank: u32) -> XWindowMirror {
        XWindowMirror {
            window: wrap_xid(window),
            parent: parent.map(wrap_xid),
            children: Vec::new(),
            toplevel: None,
            client: None,
            mapped: false,
            stack_rank,
            geometry: Rect {
                x: i32::try_from(window).unwrap_or(0),
                y: 0,
                width: 100,
                height: 50,
            },
            namespace: None,
            stale_metadata: 0,
        }
    }

    #[test]
    fn mirror_events_update_map_state() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x10, None, 0));

        state.apply_event(XMirrorEvent::Map {
            window: wrap_xid(0x10),
        });
        assert!(state.windows()[0].mapped);

        state.apply_event(XMirrorEvent::Unmap {
            window: wrap_xid(0x10),
        });
        assert!(!state.windows()[0].mapped);
    }

    #[test]
    fn mirror_events_remove_destroyed_windows_from_parent_children() {
        let mut state = XMirrorState::default();
        let mut parent = mirror(0x10, None, 0);
        parent.children.push(wrap_xid(0x20));
        state.ingest_window(parent);
        state.ingest_window(mirror(0x20, Some(0x10), 0));

        state.apply_event(XMirrorEvent::Destroy {
            window: wrap_xid(0x20),
        });

        assert_eq!(state.windows().len(), 1);
        assert!(state.windows()[0].children.is_empty());
    }

    #[test]
    fn mirror_events_reparent_windows() {
        let mut state = XMirrorState::default();
        let mut old_parent = mirror(0x10, None, 0);
        old_parent.children.push(wrap_xid(0x30));
        state.ingest_window(old_parent);
        state.ingest_window(mirror(0x20, None, 1));
        state.ingest_window(mirror(0x30, Some(0x10), 0));

        state.apply_event(XMirrorEvent::Reparent {
            window: wrap_xid(0x30),
            parent: Some(wrap_xid(0x20)),
        });

        let old_parent = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x10))
            .unwrap();
        let new_parent = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();
        let child = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x30))
            .unwrap();

        assert!(old_parent.children.is_empty());
        assert_eq!(new_parent.children, vec![wrap_xid(0x30)]);
        assert_eq!(child.parent, Some(wrap_xid(0x20)));
        assert_eq!(child.stale_metadata, 1);
    }

    #[test]
    fn mirror_events_track_restack_and_property_staleness() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x10, None, 3));
        state.ingest_window(mirror(0x20, None, 5));

        state.apply_event(XMirrorEvent::Configure {
            window: wrap_xid(0x10),
            geometry: Rect {
                x: 1,
                y: 2,
                width: 300,
                height: 200,
            },
            above_sibling: Some(wrap_xid(0x20)),
        });
        state.apply_event(XMirrorEvent::Property {
            window: wrap_xid(0x10),
            atom: 42,
            deleted: false,
        });

        let window = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x10))
            .unwrap();

        assert_eq!(window.stack_rank, 6);
        assert_eq!(window.stale_metadata, 2);
    }

    #[test]
    fn client_hints_mark_root_child_as_toplevel() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x01, None, 0));
        state.ingest_window(mirror(0x20, Some(0x01), 0));

        state.apply_client_hints(&XClientHints {
            ewmh_clients: vec![wrap_xid(0x20)],
            icccm_clients: Vec::new(),
        });

        let client = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();

        assert_eq!(client.client, Some(wrap_xid(0x20)));
        assert_eq!(client.toplevel, Some(wrap_xid(0x20)));
    }

    #[test]
    fn unmanaged_client_fallback_marks_mapped_root_children() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x01, None, 0));
        let mut client = mirror(0x20, Some(0x01), 0);
        client.mapped = true;
        state.ingest_window(client);
        let mut nested = mirror(0x30, Some(0x20), 0);
        nested.mapped = true;
        state.ingest_window(nested);

        state.apply_unmanaged_client_fallback();

        let client = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();
        let nested = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x30))
            .unwrap();

        assert_eq!(client.client, Some(wrap_xid(0x20)));
        assert_eq!(client.toplevel, Some(wrap_xid(0x20)));
        assert_eq!(nested.client, None);
    }

    #[test]
    fn client_hints_promote_reparented_frame_as_toplevel() {
        let mut state = XMirrorState::default();
        let mut root = mirror(0x01, None, 0);
        root.children.push(wrap_xid(0x20));
        let mut frame = mirror(0x20, Some(0x01), 0);
        frame.children.push(wrap_xid(0x30));
        state.ingest_window(root);
        state.ingest_window(frame);
        state.ingest_window(mirror(0x30, Some(0x20), 0));

        state.apply_client_hints(&XClientHints {
            ewmh_clients: Vec::new(),
            icccm_clients: vec![wrap_xid(0x30)],
        });

        let frame = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();
        let client = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x30))
            .unwrap();

        assert_eq!(frame.client, Some(wrap_xid(0x30)));
        assert_eq!(frame.toplevel, Some(wrap_xid(0x20)));
        assert_eq!(client.client, Some(wrap_xid(0x30)));
        assert_eq!(client.toplevel, Some(wrap_xid(0x20)));
    }

    #[test]
    fn surface_id_map_returns_stable_surface_ids() {
        let mut surfaces = SurfaceIdMap::default();
        let window = wrap_xid(0x20);
        let first = surfaces.surface_for_window(window);
        let second = surfaces.surface_for_window(window);

        assert_eq!(first, second);
        assert!(first.is_valid());
    }

    #[test]
    fn composite_pixmap_map_returns_buffer_sources() {
        let mut pixmaps = CompositePixmapMap::default();
        let window = wrap_xid(0x20);

        assert_eq!(pixmaps.source_for_window(window), BufferSource::None);

        pixmaps.insert_named_pixmap(window, 0x9000);

        assert_eq!(
            pixmaps.source_for_window(window),
            BufferSource::XPixmap { pixmap: 0x9000 }
        );
        assert_eq!(pixmaps.remove_window(window), Some(0x9000));
        assert_eq!(pixmaps.pixmap_for_window(window), None);
    }

    #[test]
    fn cpu_buffer_store_reuses_handles_for_pixmap_updates() {
        let mut store = CpuBufferStore::default();
        let first = store.upsert_pixmap(
            0x9000,
            Size {
                width: 2,
                height: 2,
            },
            24,
            0x21,
            vec![1, 2, 3, 4],
        );
        let second = store.upsert_pixmap(
            0x9000,
            Size {
                width: 2,
                height: 2,
            },
            24,
            0x21,
            vec![5, 6, 7, 8],
        );

        assert_eq!(first.handle, second.handle);
        assert_eq!(store.handle_for_pixmap(0x9000), Some(first.handle));
        assert_eq!(store.get(first.handle).unwrap().bytes, vec![5, 6, 7, 8]);
        assert_eq!(store.remove_pixmap(0x9000).unwrap().handle, first.handle);
        assert!(store.is_empty());
    }

    #[test]
    fn layers_from_surfaces_keeps_cpu_buffer_sources_renderable() {
        let surface = SurfaceSnapshot {
            surface: SurfaceId::new(1, 1),
            window: wrap_xid(0x20),
            toplevel: Some(wrap_xid(0x20)),
            client: Some(wrap_xid(0x20)),
            namespace: None,
            mapped: true,
            stack_rank: 7,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 320,
                height: 200,
            },
            source: BufferSource::CpuBuffer { handle: 9 },
            damage: Region::single(Rect {
                x: 10,
                y: 20,
                width: 320,
                height: 200,
            }),
            generation: 3,
        };

        let layers = layers_from_surfaces(&[surface]);

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].source, BufferSource::CpuBuffer { handle: 9 });
        assert_eq!(layers[0].stack_rank, 7);
        assert_eq!(layers[0].damage.rects.len(), 1);
    }

    #[test]
    fn damage_tracker_maps_damage_handles_to_windows() {
        let mut tracker = DamageTracker::default();
        let window = wrap_xid(0x20);

        tracker.insert_damage(window, 0x5000);

        assert_eq!(tracker.damage_for_window(window), Some(0x5000));
        assert_eq!(tracker.window_for_damage(0x5000), Some(window));
        assert_eq!(
            tracker.record_for_window(window),
            Some(DamageRecord {
                window,
                damage: 0x5000
            })
        );
    }

    #[test]
    fn damage_tracker_accumulates_and_drains_regions() {
        let mut tracker = DamageTracker::default();
        let window = wrap_xid(0x20);
        tracker.insert_damage(window, 0x5000);

        let applied = tracker.apply_event(XDamageEvent {
            window,
            damage: 0x5000,
            drawable: window,
            timestamp: 42,
            area: Rect {
                x: 5,
                y: 6,
                width: 70,
                height: 80,
            },
            drawable_geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
        });

        assert!(applied);
        assert_eq!(
            tracker.pending_damage(window).unwrap().rects,
            vec![Rect {
                x: 5,
                y: 6,
                width: 70,
                height: 80,
            }]
        );
        assert_eq!(tracker.drain_damage(window).rects.len(), 1);
        assert_eq!(tracker.pending_damage(window), None);
    }

    #[test]
    fn x_damage_event_converts_known_x11_damage_notify() {
        let mut tracker = DamageTracker::default();
        let window = wrap_xid(0x20);
        tracker.insert_damage(window, 0x5000);

        let event = Event::DamageNotify(x11rb::protocol::damage::NotifyEvent {
            response_type: 0,
            level: ReportLevel::BOUNDING_BOX,
            sequence: 1,
            drawable: 0x20,
            damage: 0x5000,
            timestamp: 42,
            area: x11rb::protocol::xproto::Rectangle {
                x: 5,
                y: 6,
                width: 70,
                height: 80,
            },
            geometry: x11rb::protocol::xproto::Rectangle {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
        });

        let converted = XDamageEvent::from_x11_event(&event, &tracker).unwrap();

        assert_eq!(converted.window, window);
        assert_eq!(converted.damage, 0x5000);
        assert_eq!(
            converted.area,
            Rect {
                x: 5,
                y: 6,
                width: 70,
                height: 80,
            }
        );
    }

    #[test]
    fn emits_damage_frame_from_tracked_client_damage() {
        let mut state = XMirrorState::default();
        let mut frame = mirror(0x20, None, 4);
        frame.mapped = true;
        frame.client = Some(wrap_xid(0x30));
        frame.toplevel = Some(wrap_xid(0x20));
        frame.geometry = Rect {
            x: 100,
            y: 200,
            width: 640,
            height: 480,
        };
        state.ingest_window(frame);

        let mut surfaces = SurfaceIdMap::default();
        let pixmaps = CompositePixmapMap::default();
        let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);

        let mut tracker = DamageTracker::default();
        tracker.insert_damage(wrap_xid(0x30), 0x5000);
        assert!(tracker.apply_event(XDamageEvent {
            window: wrap_xid(0x30),
            damage: 0x5000,
            drawable: wrap_xid(0x30),
            timestamp: 42,
            area: Rect {
                x: 5,
                y: 6,
                width: 70,
                height: 80,
            },
            drawable_geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
        }));

        let frame = emit_damage_frame(&mut tracker, OutputId::from_raw(1), 9, 2, 3, &snapshots);

        assert_eq!(frame.output, OutputId::from_raw(1));
        assert_eq!(frame.frame_serial, 9);
        assert_eq!(frame.buffer_age, 2);
        assert_eq!(frame.root_generation, 3);
        assert_eq!(frame.affected_surfaces, vec![snapshots[0].surface]);
        assert_eq!(
            frame.damage.rects,
            vec![Rect {
                x: 105,
                y: 206,
                width: 70,
                height: 80,
            }]
        );
        assert!(tracker.pending_damage(wrap_xid(0x30)).is_none());
    }

    #[test]
    fn damage_frame_drops_unmapped_surface_damage() {
        let mut state = XMirrorState::default();
        let mut window = mirror(0x20, None, 4);
        window.client = Some(wrap_xid(0x20));
        window.toplevel = Some(wrap_xid(0x20));
        state.ingest_window(window);

        let mut surfaces = SurfaceIdMap::default();
        let pixmaps = CompositePixmapMap::default();
        let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);

        let mut tracker = DamageTracker::default();
        tracker.insert_damage(wrap_xid(0x20), 0x5000);
        assert!(tracker.apply_event(XDamageEvent {
            window: wrap_xid(0x20),
            damage: 0x5000,
            drawable: wrap_xid(0x20),
            timestamp: 42,
            area: Rect {
                x: 5,
                y: 6,
                width: 70,
                height: 80,
            },
            drawable_geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
        }));

        let frame = emit_damage_frame(&mut tracker, OutputId::from_raw(1), 9, 2, 3, &snapshots);

        assert!(frame.affected_surfaces.is_empty());
        assert!(frame.damage.is_empty());
        assert!(tracker.pending_damage(wrap_xid(0x20)).is_none());
    }

    #[test]
    fn emits_surface_and_layer_snapshots_for_detected_clients() {
        let mut state = XMirrorState::default();
        let mut window = mirror(0x20, None, 4);
        window.mapped = true;
        window.client = Some(wrap_xid(0x20));
        window.toplevel = Some(wrap_xid(0x20));
        window.geometry = Rect {
            x: 10,
            y: 20,
            width: 640,
            height: 480,
        };
        state.ingest_window(window);

        let mut surfaces = SurfaceIdMap::default();
        let pixmaps = CompositePixmapMap::default();
        let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);
        let layers = state.emit_layers(&mut surfaces, &pixmaps);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].window, wrap_xid(0x20));
        assert_eq!(snapshots[0].geometry.width, 640);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].surface, snapshots[0].surface);
        assert_eq!(layers[0].source, BufferSource::None);
    }

    #[test]
    fn emits_named_pixmap_sources_for_detected_clients() {
        let mut state = XMirrorState::default();
        let mut frame = mirror(0x20, None, 4);
        frame.mapped = true;
        frame.client = Some(wrap_xid(0x30));
        frame.toplevel = Some(wrap_xid(0x20));
        state.ingest_window(frame);

        let mut surfaces = SurfaceIdMap::default();
        let mut pixmaps = CompositePixmapMap::default();
        pixmaps.insert_named_pixmap(wrap_xid(0x30), 0x9000);

        let snapshots = state.emit_surfaces(&mut surfaces, &pixmaps);
        let layers = state.emit_layers(&mut surfaces, &pixmaps);

        assert_eq!(
            snapshots[0].source,
            BufferSource::XPixmap { pixmap: 0x9000 }
        );
        assert_eq!(layers[0].source, BufferSource::XPixmap { pixmap: 0x9000 });
    }

    #[test]
    fn composite_redirect_targets_use_unique_mapped_clients() {
        let mut state = XMirrorState::default();
        let mut frame = mirror(0x20, None, 0);
        frame.mapped = true;
        frame.client = Some(wrap_xid(0x30));
        frame.toplevel = Some(wrap_xid(0x20));
        let mut client = mirror(0x30, Some(0x20), 0);
        client.mapped = true;
        client.client = Some(wrap_xid(0x30));
        client.toplevel = Some(wrap_xid(0x20));
        let mut unmapped = mirror(0x40, None, 0);
        unmapped.client = Some(wrap_xid(0x40));
        unmapped.toplevel = Some(wrap_xid(0x40));
        state.ingest_window(frame);
        state.ingest_window(client);
        state.ingest_window(unmapped);

        let targets = state.composite_redirect_targets();

        assert_eq!(
            targets,
            vec![CompositeRedirectTarget {
                window: wrap_xid(0x30),
                update: CompositeUpdateMode::Manual,
            }]
        );
    }

    #[test]
    fn builds_flat_routed_input_request_for_xlibre() {
        let event = input_event(10);
        let route = input_route(
            10,
            InputRouteOutcome::Routed,
            Some(wrap_xid(0x30)),
            Some(Point { x: 12.0, y: 8.0 }),
            Transform::IDENTITY,
        );

        let request = build_flat_routed_input_request(&event, &route).unwrap();

        assert_eq!(request.serial, 10);
        assert_eq!(request.seat, SeatId::from_raw(1));
        assert_eq!(request.device, DeviceId::from_raw(2));
        assert_eq!(request.target_window, wrap_xid(0x30));
        assert_eq!(request.local_position, Point { x: 12.0, y: 8.0 });
        assert_eq!(
            request.kind,
            InputEventKind::PointerButton {
                button: 1,
                pressed: true,
            }
        );
    }

    #[test]
    fn flat_routed_input_rejects_transformed_routes() {
        let event = input_event(11);
        let route = input_route(
            11,
            InputRouteOutcome::Routed,
            Some(wrap_xid(0x30)),
            Some(Point { x: 1.0, y: 2.0 }),
            Transform {
                matrix: [
                    2.0, 0.0, 0.0, //
                    0.0, 2.0, 0.0, //
                    0.0, 0.0, 1.0,
                ],
            },
        );

        assert_eq!(
            build_flat_routed_input_request(&event, &route),
            Err(RoutedInputAdapterError::UnsupportedTransform)
        );
    }

    #[test]
    fn flat_routed_input_rejects_stale_target_before_xlibre_request() {
        let event = input_event(12);
        let route = input_route(
            12,
            InputRouteOutcome::StaleTarget,
            Some(wrap_xid(0x30)),
            Some(Point { x: 1.0, y: 2.0 }),
            Transform::IDENTITY,
        );

        assert_eq!(
            build_flat_routed_input_request(&event, &route),
            Err(RoutedInputAdapterError::StaleTarget)
        );
    }

    #[test]
    fn xlibre_decision_blocks_denied_namespace_grab_and_focus_cases() {
        for outcome in [
            XLibreRoutedInputOutcome::RejectedDeniedNamespace,
            XLibreRoutedInputOutcome::RejectedActiveGrab,
            XLibreRoutedInputOutcome::RejectedFocusPolicy,
            XLibreRoutedInputOutcome::RejectedStaleTarget,
        ] {
            let decision = XLibreRoutedInputDecision {
                serial: 13,
                target_window: wrap_xid(0x30),
                outcome,
            };

            assert!(!routed_input_decision_allows_delivery(&decision));
        }
    }

    #[test]
    fn xlibre_decision_accepts_only_server_accepted_delivery() {
        let decision = XLibreRoutedInputDecision {
            serial: 14,
            target_window: wrap_xid(0x30),
            outcome: XLibreRoutedInputOutcome::Accepted,
        };

        assert!(routed_input_decision_allows_delivery(&decision));
    }

    fn input_event(serial: u64) -> InputEventPacket {
        InputEventPacket {
            serial,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: 1_000,
            kind: InputEventKind::PointerButton {
                button: 1,
                pressed: true,
            },
            global_position: Some(Point { x: 100.0, y: 200.0 }),
            target_surface: Some(SurfaceId::new(3, 1)),
            target_window: Some(wrap_xid(0x30)),
            local_position: Some(Point { x: 12.0, y: 8.0 }),
        }
    }

    fn input_route(
        serial: u64,
        outcome: InputRouteOutcome,
        target_window: Option<XWindowId>,
        local_position: Option<Point>,
        transform: Transform,
    ) -> InputRoute {
        InputRoute {
            input_serial: serial,
            target_surface: Some(SurfaceId::new(3, 1)),
            target_window,
            global_position: Point { x: 100.0, y: 200.0 },
            local_position,
            transform,
            outcome,
        }
    }
}

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::Arc;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer as _, Format, Fourcc, Modifier};
use smithay::backend::input::{ButtonState, KeyState, Keycode};
use smithay::input::keyboard::{FilterResult, KeyboardHandle};
use smithay::input::pointer::{ButtonEvent, MotionEvent, PointerHandle};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::{
    wl_buffer, wl_callback, wl_seat, wl_shm, wl_surface,
};
use smithay::reexports::wayland_server::{Client, Display, ListeningSocket, Resource};
use smithay::utils::{Serial, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, Damage,
    SurfaceAttributes, with_states,
};
use smithay::wayland::dmabuf::{
    DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier, get_dmabuf,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::shell::xdg::{
    Configure, PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{BufferAccessError, ShmHandler, ShmState, with_buffer_contents};
use smithay::{
    delegate_compositor, delegate_dmabuf, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
};
use sophia_protocol::{
    AuthorityFeedback, AuthorityLocalId, BufferSource, CpuBufferFormat, CpuBufferRegistration,
    InputEventKind, NamespaceId, Rect, Region, RoutedInputDecision, RoutedInputOutcome,
    RoutedInputRequest, Size, SurfaceId, TransactionId,
};

use crate::{
    WaylandAuthorityAction, WaylandAuthorityReducer, WaylandSurfaceEvent, WaylandSurfaceRole,
    WaylandXdgEvent,
};

const MAX_SHM_BUFFER_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_TRANSACTION_TIMEOUT_MSEC: u32 = 250;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaylandFrontendEvent {
    ClientConnected { namespace: NamespaceId },
    Authority(WaylandAuthorityAction),
    CpuBufferRegistered(CpuBufferRegistration),
    DmaBufRegistered(DmaBufRegistration),
    ProtocolError(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DmaBufRegistration {
    pub handle: u64,
    pub size: Size,
    pub format: u32,
    pub modifier: u64,
    pub plane_count: u8,
    pub dmabuf: Dmabuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaylandFrontendError {
    message: String,
}

impl WaylandFrontendError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WaylandFrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WaylandFrontendError {}

pub struct WaylandFrontend {
    display: Display<FrontendState>,
    listener: ListeningSocket,
    clients: Vec<Client>,
    state: FrontendState,
    next_namespace: u64,
}

impl WaylandFrontend {
    pub fn bind(display_name: &str, output_size: Size) -> Result<Self, WaylandFrontendError> {
        Self::bind_with_imports(display_name, output_size, false)
    }

    pub fn bind_with_dmabuf(
        display_name: &str,
        output_size: Size,
    ) -> Result<Self, WaylandFrontendError> {
        Self::bind_with_imports(display_name, output_size, true)
    }

    fn bind_with_imports(
        display_name: &str,
        output_size: Size,
        enable_dmabuf: bool,
    ) -> Result<Self, WaylandFrontendError> {
        if display_name.is_empty() || output_size.width <= 0 || output_size.height <= 0 {
            return Err(WaylandFrontendError::new(
                "Wayland display name and output size must be valid",
            ));
        }
        let display = Display::<FrontendState>::new()
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
        let handle = display.handle();
        let compositor_state = CompositorState::new::<FrontendState>(&handle);
        let xdg_shell_state = XdgShellState::new::<FrontendState>(&handle);
        let shm_state = ShmState::new::<FrontendState>(
            &handle,
            vec![wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888],
        );
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&handle, "seat0");
        let keyboard = seat
            .add_keyboard(Default::default(), 600, 25)
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
        let pointer = seat.add_pointer();
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_formats = vec![
            Format {
                code: Fourcc::Xrgb8888,
                modifier: Modifier::Linear,
            },
            Format {
                code: Fourcc::Argb8888,
                modifier: Modifier::Linear,
            },
        ];
        let dmabuf_global = enable_dmabuf
            .then(|| dmabuf_state.create_global::<FrontendState>(&handle, dmabuf_formats));
        let output = Output::new(
            "SOPHIA-1".to_owned(),
            PhysicalProperties {
                size: (300, 190).into(),
                subpixel: Subpixel::Unknown,
                make: "Sophia".to_owned(),
                model: "Virtual Output".to_owned(),
            },
        );
        output.create_global::<FrontendState>(&handle);
        let mode = Mode {
            size: (output_size.width, output_size.height).into(),
            refresh: 60_000,
        };
        output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(Scale::Integer(1)),
            Some((0, 0).into()),
        );
        output.set_preferred(mode);

        let listener = ListeningSocket::bind(display_name)
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
        Ok(Self {
            display,
            listener,
            clients: Vec::new(),
            state: FrontendState::new(
                compositor_state,
                xdg_shell_state,
                shm_state,
                seat_state,
                seat,
                keyboard,
                pointer,
                dmabuf_state,
                dmabuf_global,
                output,
                output_size,
            ),
            next_namespace: 1,
        })
    }

    pub fn display_name(&self) -> Option<&str> {
        self.listener.socket_name().and_then(|name| name.to_str())
    }

    pub fn dispatch(&mut self) -> Result<Vec<WaylandFrontendEvent>, WaylandFrontendError> {
        while let Some(stream) = self
            .listener
            .accept()
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?
        {
            let namespace = NamespaceId::from_raw(self.next_namespace);
            self.next_namespace = self.next_namespace.saturating_add(1);
            let client = self
                .display
                .handle()
                .insert_client(stream, Arc::new(FrontendClientState::new(namespace)))
                .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
            self.clients.push(client);
            self.state
                .events
                .push_back(WaylandFrontendEvent::ClientConnected { namespace });
        }
        self.display
            .dispatch_clients(&mut self.state)
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
        self.display
            .flush_clients()
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
        Ok(self.state.events.drain(..).collect())
    }

    pub fn apply_feedback(
        &mut self,
        feedback: AuthorityFeedback,
    ) -> Result<Vec<WaylandFrontendEvent>, WaylandFrontendError> {
        let actions = self
            .state
            .reducer
            .apply_feedback(feedback)
            .map_err(|error| WaylandFrontendError::new(format!("{error:?}")))?;
        self.state.record_actions(actions);
        self.display
            .flush_clients()
            .map_err(|error| WaylandFrontendError::new(error.to_string()))?;
        Ok(self.state.events.drain(..).collect())
    }

    pub fn route_input(&mut self, request: &RoutedInputRequest) -> RoutedInputDecision {
        self.state.route_input(request)
    }
}

struct FrontendState {
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    shm_state: ShmState,
    dmabuf_state: DmabufState,
    _dmabuf_global: Option<DmabufGlobal>,
    seat_state: SeatState<Self>,
    _seat: Seat<Self>,
    keyboard: KeyboardHandle<Self>,
    pointer: PointerHandle<Self>,
    _output: Output,
    output_size: Size,
    reducer: WaylandAuthorityReducer,
    surface_by_object: BTreeMap<u32, AuthorityLocalId>,
    surface_resources: BTreeMap<SurfaceId, wl_surface::WlSurface>,
    toplevels: BTreeMap<SurfaceId, ToplevelSurface>,
    callbacks: BTreeMap<u64, wl_callback::WlCallback>,
    buffers: BTreeMap<u64, wl_buffer::WlBuffer>,
    imported_dmabufs: Vec<(Dmabuf, u64)>,
    events: VecDeque<WaylandFrontendEvent>,
    next_surface: u32,
    next_buffer: u64,
    next_transaction: u64,
}

impl FrontendState {
    fn new(
        compositor_state: CompositorState,
        xdg_shell_state: XdgShellState,
        shm_state: ShmState,
        seat_state: SeatState<Self>,
        seat: Seat<Self>,
        keyboard: KeyboardHandle<Self>,
        pointer: PointerHandle<Self>,
        dmabuf_state: DmabufState,
        dmabuf_global: Option<DmabufGlobal>,
        output: Output,
        output_size: Size,
    ) -> Self {
        Self {
            compositor_state,
            xdg_shell_state,
            shm_state,
            seat_state,
            _seat: seat,
            keyboard,
            pointer,
            dmabuf_state,
            _dmabuf_global: dmabuf_global,
            _output: output,
            output_size,
            reducer: WaylandAuthorityReducer::new(),
            surface_by_object: BTreeMap::new(),
            surface_resources: BTreeMap::new(),
            toplevels: BTreeMap::new(),
            callbacks: BTreeMap::new(),
            buffers: BTreeMap::new(),
            imported_dmabufs: Vec::new(),
            events: VecDeque::new(),
            next_surface: 1,
            next_buffer: 1,
            next_transaction: 1,
        }
    }

    fn record_actions(&mut self, actions: Vec<WaylandAuthorityAction>) {
        for action in actions {
            match &action {
                WaylandAuthorityAction::FrameDone {
                    callback,
                    presentation_msec,
                } => {
                    if let Some(resource) = self.callbacks.remove(callback) {
                        resource.done(u32::try_from(*presentation_msec).unwrap_or(u32::MAX));
                    }
                }
                WaylandAuthorityAction::BufferReleased(feedback) => {
                    let handle = match feedback.source {
                        BufferSource::CpuBuffer { handle } | BufferSource::DmaBuf { handle } => {
                            Some(handle)
                        }
                        _ => None,
                    };
                    if let Some(buffer) = handle.and_then(|handle| self.buffers.remove(&handle)) {
                        buffer.release();
                    }
                }
                WaylandAuthorityAction::CloseRequested { surface } => {
                    if let Some(toplevel) = self.toplevels.get(surface) {
                        toplevel.send_close();
                    }
                }
                _ => {}
            }
            self.events
                .push_back(WaylandFrontendEvent::Authority(action));
        }
    }

    fn local_id(surface: &wl_surface::WlSurface) -> AuthorityLocalId {
        AuthorityLocalId::new(u64::from(surface.id().protocol_id()), 1)
    }

    fn namespace(surface: &wl_surface::WlSurface) -> Option<NamespaceId> {
        let client = surface.client()?;
        let state = client.get_data::<FrontendClientState>()?;
        Some(state.namespace)
    }

    fn snapshot_shm_buffer(
        &mut self,
        buffer: &wl_buffer::WlBuffer,
    ) -> Result<(BufferSource, CpuBufferRegistration), WaylandFrontendError> {
        let handle = self.next_buffer;
        self.next_buffer = self.next_buffer.saturating_add(1);
        let registration = with_buffer_contents(buffer, |pointer, pool_len, data| {
            copy_shm_contents(handle, pointer, pool_len, data)
        })
        .map_err(|error| match error {
            BufferAccessError::NotManaged => WaylandFrontendError::new("non-SHM buffer attached"),
            other => WaylandFrontendError::new(format!("SHM buffer access failed: {other:?}")),
        })??;
        self.buffers.insert(handle, buffer.clone());
        Ok((BufferSource::CpuBuffer { handle }, registration))
    }

    fn imported_dmabuf(
        &mut self,
        buffer: &wl_buffer::WlBuffer,
    ) -> Result<BufferSource, WaylandFrontendError> {
        let dmabuf = get_dmabuf(buffer)
            .map_err(|_| WaylandFrontendError::new("buffer is neither SHM nor DMA-BUF"))?;
        let handle = self
            .imported_dmabufs
            .iter()
            .find_map(|(candidate, handle)| (candidate == dmabuf).then_some(*handle))
            .ok_or_else(|| WaylandFrontendError::new("DMA-BUF was not admitted"))?;
        self.buffers.insert(handle, buffer.clone());
        Ok(BufferSource::DmaBuf { handle })
    }

    fn commit_surface(&mut self, surface: &wl_surface::WlSurface) {
        let Some(local_id) = self
            .surface_by_object
            .get(&surface.id().protocol_id())
            .copied()
        else {
            return;
        };
        let (buffer, damage, callbacks) = with_states(surface, |states| {
            let mut cached = states.cached_state.get::<SurfaceAttributes>();
            let attributes = cached.current();
            (
                attributes.buffer.take(),
                std::mem::take(&mut attributes.damage),
                std::mem::take(&mut attributes.frame_callbacks),
            )
        });

        match buffer {
            Some(BufferAssignment::NewBuffer(buffer)) => match self.snapshot_shm_buffer(&buffer) {
                Ok((source, registration)) => {
                    self.events
                        .push_back(WaylandFrontendEvent::CpuBufferRegistered(registration));
                    self.apply_reducer_event(WaylandSurfaceEvent::Attach {
                        local_id,
                        buffer: source,
                    });
                }
                Err(_) => match self.imported_dmabuf(&buffer) {
                    Ok(source) => self.apply_reducer_event(WaylandSurfaceEvent::Attach {
                        local_id,
                        buffer: source,
                    }),
                    Err(error) => self
                        .events
                        .push_back(WaylandFrontendEvent::ProtocolError(error.to_string())),
                },
            },
            Some(BufferAssignment::Removed) => {
                self.apply_reducer_event(WaylandSurfaceEvent::Detach { local_id });
            }
            None => {}
        }

        let mut region = Region::empty();
        for item in damage {
            let rectangle = match item {
                Damage::Surface(rectangle) => rectangle,
                Damage::Buffer(rectangle) => {
                    rectangle.to_logical(1, Transform::Normal, &rectangle.size)
                }
            };
            region.push(Rect {
                x: rectangle.loc.x,
                y: rectangle.loc.y,
                width: rectangle.size.w,
                height: rectangle.size.h,
            });
        }
        if !region.is_empty() {
            self.apply_reducer_event(WaylandSurfaceEvent::Damage {
                local_id,
                damage: region,
            });
        }
        for callback in callbacks {
            let id = u64::from(callback.id().protocol_id());
            self.callbacks.insert(id, callback);
            self.apply_reducer_event(WaylandSurfaceEvent::RequestFrame {
                local_id,
                callback: id,
            });
        }

        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self.next_transaction.saturating_add(1);
        self.apply_reducer_event(WaylandSurfaceEvent::Commit {
            local_id,
            transaction,
            timeout_msec: DEFAULT_TRANSACTION_TIMEOUT_MSEC,
        });
    }

    fn apply_reducer_event(&mut self, event: WaylandSurfaceEvent) {
        match self.reducer.apply_surface_event(event) {
            Ok(actions) => self.record_actions(actions),
            Err(error) => self
                .events
                .push_back(WaylandFrontendEvent::ProtocolError(format!("{error:?}"))),
        }
    }

    fn route_input(&mut self, request: &RoutedInputRequest) -> RoutedInputDecision {
        let outcome = self.deliver_input(request);
        RoutedInputDecision {
            serial: request.serial,
            target_surface: request.target_surface,
            outcome,
        }
    }

    fn deliver_input(&mut self, request: &RoutedInputRequest) -> RoutedInputOutcome {
        let Some(surface) = self.surface_resources.get(&request.target_surface).cloned() else {
            return RoutedInputOutcome::RejectedStaleTarget;
        };
        let serial = Serial::from(u32::try_from(request.serial).unwrap_or(u32::MAX));
        let time = u32::try_from(request.time_msec).unwrap_or(u32::MAX);
        match request.kind {
            InputEventKind::Key { keycode, pressed } => {
                let keyboard = self.keyboard.clone();
                keyboard.set_focus(self, Some(surface), serial);
                keyboard.input::<(), _>(
                    self,
                    Keycode::from(keycode),
                    if pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    },
                    serial,
                    time,
                    |_, _, _| FilterResult::Forward,
                );
            }
            InputEventKind::PointerMotion => {
                let pointer = self.pointer.clone();
                pointer.motion(
                    self,
                    Some((
                        surface,
                        (
                            request.global_position.x - request.local_position.x,
                            request.global_position.y - request.local_position.y,
                        )
                            .into(),
                    )),
                    &MotionEvent {
                        location: (request.global_position.x, request.global_position.y).into(),
                        serial,
                        time,
                    },
                );
                pointer.frame(self);
            }
            InputEventKind::PointerButton { button, pressed } => {
                let pointer = self.pointer.clone();
                pointer.motion(
                    self,
                    Some((
                        surface,
                        (
                            request.global_position.x - request.local_position.x,
                            request.global_position.y - request.local_position.y,
                        )
                            .into(),
                    )),
                    &MotionEvent {
                        location: (request.global_position.x, request.global_position.y).into(),
                        serial,
                        time,
                    },
                );
                pointer.button(
                    self,
                    &ButtonEvent {
                        serial,
                        time,
                        button,
                        state: if pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                    },
                );
                pointer.frame(self);
            }
        }
        RoutedInputOutcome::Accepted
    }
}

impl BufferHandler for FrontendState {
    fn buffer_destroyed(&mut self, buffer: &wl_buffer::WlBuffer) {
        self.buffers.retain(|_, candidate| candidate != buffer);
    }
}

impl CompositorHandler for FrontendState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<FrontendClientState>()
            .expect("registered Sophia Wayland client")
            .compositor_state
    }

    fn commit(&mut self, surface: &wl_surface::WlSurface) {
        self.commit_surface(surface);
    }

    fn destroyed(&mut self, surface: &wl_surface::WlSurface) {
        let Some(local_id) = self.surface_by_object.remove(&surface.id().protocol_id()) else {
            return;
        };
        if let Ok(actions) = self
            .reducer
            .apply_surface_event(WaylandSurfaceEvent::Destroyed { local_id })
        {
            for action in &actions {
                if let WaylandAuthorityAction::SurfaceDestroyed { surface } = action {
                    self.surface_resources.remove(surface);
                    self.toplevels.remove(surface);
                }
            }
            self.record_actions(actions);
        }
    }
}

impl XdgShellHandler for FrontendState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, toplevel: ToplevelSurface) {
        let resource = toplevel.wl_surface().clone();
        let Some(namespace) = Self::namespace(&resource) else {
            self.events.push_back(WaylandFrontendEvent::ProtocolError(
                "toplevel has no Sophia namespace".to_owned(),
            ));
            return;
        };
        let local_id = Self::local_id(&resource);
        let surface = SurfaceId::new(self.next_surface, 1);
        self.next_surface = self.next_surface.saturating_add(1);
        let geometry = Rect {
            x: 0,
            y: 0,
            width: self.output_size.width,
            height: self.output_size.height,
        };
        self.apply_reducer_event(WaylandSurfaceEvent::Created {
            namespace,
            local_id,
            surface,
            geometry,
        });
        self.apply_reducer_event(WaylandSurfaceEvent::AssignRole {
            local_id,
            role: WaylandSurfaceRole::Toplevel,
        });
        self.surface_by_object
            .insert(resource.id().protocol_id(), local_id);
        self.surface_resources.insert(surface, resource);
        self.toplevels.insert(surface, toplevel.clone());
        toplevel.with_pending_state(|state| {
            state.size = Some((self.output_size.width, self.output_size.height).into());
        });
        let serial = u32::from(toplevel.send_configure());
        if let Ok(actions) = self.reducer.apply_xdg_event(WaylandXdgEvent::Configure {
            local_id,
            serial,
            size: self.output_size,
        }) {
            self.record_actions(actions);
        }
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn ack_configure(&mut self, surface: wl_surface::WlSurface, configure: Configure) {
        let Some(local_id) = self
            .surface_by_object
            .get(&surface.id().protocol_id())
            .copied()
        else {
            return;
        };
        let serial = match configure {
            Configure::Toplevel(configure) => u32::from(configure.serial),
            Configure::Popup(configure) => u32::from(configure.serial),
        };
        if let Ok(actions) = self
            .reducer
            .apply_xdg_event(WaylandXdgEvent::AckConfigure { local_id, serial })
        {
            self.record_actions(actions);
        }
    }
}

impl ShmHandler for FrontendState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl OutputHandler for FrontendState {}

impl DmabufHandler for FrontendState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        match validate_dmabuf(&dmabuf, self.next_buffer) {
            Ok(registration) => {
                self.next_buffer = self.next_buffer.saturating_add(1);
                self.imported_dmabufs.push((dmabuf, registration.handle));
                self.events
                    .push_back(WaylandFrontendEvent::DmaBufRegistered(registration));
                if let Err(error) = notifier.successful::<Self>() {
                    self.events
                        .push_back(WaylandFrontendEvent::ProtocolError(format!(
                            "DMA-BUF wl_buffer creation failed: {error}"
                        )));
                }
            }
            Err(error) => {
                self.events
                    .push_back(WaylandFrontendEvent::ProtocolError(error.to_string()));
                notifier.failed();
            }
        }
    }
}

struct FrontendClientState {
    compositor_state: CompositorClientState,
    namespace: NamespaceId,
}

impl FrontendClientState {
    fn new(namespace: NamespaceId) -> Self {
        Self {
            compositor_state: CompositorClientState::default(),
            namespace,
        }
    }
}

impl ClientData for FrontendClientState {
    fn initialized(&self, _client_id: ClientId) {}

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl SeatHandler for FrontendState {
    type KeyboardFocus = wl_surface::WlSurface;
    type PointerFocus = wl_surface::WlSurface;
    type TouchFocus = wl_surface::WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&wl_surface::WlSurface>) {}

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
}

fn copy_shm_contents(
    handle: u64,
    pointer: *const u8,
    pool_len: usize,
    data: smithay::wayland::shm::BufferData,
) -> Result<CpuBufferRegistration, WaylandFrontendError> {
    let offset = usize::try_from(data.offset)
        .map_err(|_| WaylandFrontendError::new("negative SHM offset"))?;
    let width =
        usize::try_from(data.width).map_err(|_| WaylandFrontendError::new("invalid SHM width"))?;
    let height = usize::try_from(data.height)
        .map_err(|_| WaylandFrontendError::new("invalid SHM height"))?;
    let stride = usize::try_from(data.stride)
        .map_err(|_| WaylandFrontendError::new("invalid SHM stride"))?;
    if width == 0 || height == 0 || stride < width.saturating_mul(4) {
        return Err(WaylandFrontendError::new("invalid SHM dimensions"));
    }
    let byte_len = stride
        .checked_mul(height)
        .filter(|length| *length <= MAX_SHM_BUFFER_BYTES)
        .ok_or_else(|| WaylandFrontendError::new("SHM buffer exceeds Sophia limit"))?;
    let end = offset
        .checked_add(byte_len)
        .filter(|end| *end <= pool_len)
        .ok_or_else(|| WaylandFrontendError::new("SHM buffer exceeds pool"))?;
    let format = match data.format {
        wl_shm::Format::Argb8888 => CpuBufferFormat::Argb8888,
        wl_shm::Format::Xrgb8888 => CpuBufferFormat::Xrgb8888,
        _ => return Err(WaylandFrontendError::new("unsupported SHM format")),
    };
    // Smithay validates and pins the pool for the duration of this callback.
    // Copying creates the immutable snapshot consumed by Sophia's renderer.
    let bytes = unsafe { std::slice::from_raw_parts(pointer.add(offset), end - offset) }.to_vec();
    Ok(CpuBufferRegistration {
        handle,
        size: Size {
            width: data.width,
            height: data.height,
        },
        stride: u32::try_from(stride)
            .map_err(|_| WaylandFrontendError::new("SHM stride overflow"))?,
        format,
        generation: 1,
        bytes,
    })
}

fn validate_dmabuf(
    dmabuf: &Dmabuf,
    handle: u64,
) -> Result<DmaBufRegistration, WaylandFrontendError> {
    let size = dmabuf.size();
    if size.w <= 0 || size.h <= 0 || size.w > 16_384 || size.h > 16_384 {
        return Err(WaylandFrontendError::new("invalid DMA-BUF dimensions"));
    }
    let format = dmabuf.format();
    if !matches!(format.code, Fourcc::Xrgb8888 | Fourcc::Argb8888)
        || !matches!(format.modifier, Modifier::Linear | Modifier::Invalid)
    {
        return Err(WaylandFrontendError::new(
            "unsupported DMA-BUF format or modifier",
        ));
    }
    let plane_count = dmabuf.num_planes();
    if plane_count != 1 {
        return Err(WaylandFrontendError::new("unsupported DMA-BUF plane count"));
    }
    let stride = dmabuf.strides().next().unwrap_or(0);
    if stride < u32::try_from(size.w).unwrap_or(u32::MAX).saturating_mul(4) {
        return Err(WaylandFrontendError::new("invalid DMA-BUF stride"));
    }
    Ok(DmaBufRegistration {
        handle,
        size: Size {
            width: size.w,
            height: size.h,
        },
        format: format.code as u32,
        modifier: format.modifier.into(),
        plane_count: u8::try_from(plane_count).unwrap_or(u8::MAX),
        dmabuf: dmabuf.clone(),
    })
}

delegate_compositor!(FrontendState);
delegate_xdg_shell!(FrontendState);
delegate_shm!(FrontendState);
delegate_output!(FrontendState);
delegate_seat!(FrontendState);
delegate_dmabuf!(FrontendState);

#[cfg(test)]
mod tests {
    use std::os::fd::OwnedFd;

    use smithay::backend::allocator::dmabuf::{Dmabuf, DmabufFlags};
    use smithay::backend::allocator::{Fourcc, Modifier};

    use super::{WaylandFrontendError, validate_dmabuf};

    fn dmabuf(stride: u32) -> Dmabuf {
        let file = tempfile::tempfile().unwrap();
        file.set_len(64 * 64 * 4).unwrap();
        let mut builder = Dmabuf::builder(
            (64, 64),
            Fourcc::Xrgb8888,
            Modifier::Linear,
            DmabufFlags::empty(),
        );
        assert!(builder.add_plane(OwnedFd::from(file), 0, 0, stride));
        builder.build().unwrap()
    }

    #[test]
    fn admits_bounded_linear_single_plane_dmabuf() {
        let registration = validate_dmabuf(&dmabuf(64 * 4), 7).unwrap();
        assert_eq!(registration.handle, 7);
        assert_eq!(registration.size.width, 64);
        assert_eq!(registration.plane_count, 1);
    }

    #[test]
    fn rejects_short_dmabuf_stride() {
        let error = validate_dmabuf(&dmabuf(64), 7).unwrap_err();
        assert_eq!(error, WaylandFrontendError::new("invalid DMA-BUF stride"));
    }
}

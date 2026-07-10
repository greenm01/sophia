use crate::prelude::*;

#[cfg(feature = "libinput-events")]
use input::event::{
    Event as NativeLibinputEvent,
    keyboard::{KeyState, KeyboardEvent, KeyboardEventTrait},
    pointer::{ButtonState, PointerEvent, PointerEventTrait},
};
#[cfg(feature = "libinput-events")]
use sophia_protocol::{InputEventKind, Point};

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputNativeEventAdapterReport {
    pub status: LibinputNativeEventAdapterStatus,
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputNativeEventAdapterStatus {
    SkeletonReady,
}

#[cfg(feature = "libinput-events")]
pub const fn native_libinput_event_adapter_report() -> LibinputNativeEventAdapterReport {
    LibinputNativeEventAdapterReport {
        status: LibinputNativeEventAdapterStatus::SkeletonReady,
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Debug, PartialEq)]
pub struct LibinputNativeEventReadResult {
    pub report: LibinputNativeEventReadReport,
    pub events: Vec<InputEventPacket>,
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibinputNativeEventReadReport {
    pub status: LibinputNativeEventReadStatus,
    pub events_read: usize,
    pub queued_remaining: usize,
}

#[cfg(feature = "libinput-events")]
impl LibinputNativeEventReadReport {
    pub const fn idle() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::Idle,
            events_read: 0,
            queued_remaining: 0,
        }
    }

    pub const fn would_block() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::WouldBlock,
            events_read: 0,
            queued_remaining: 0,
        }
    }

    pub const fn events_read(events_read: usize, queued_remaining: usize) -> Self {
        Self {
            status: if events_read == 0 {
                LibinputNativeEventReadStatus::Idle
            } else {
                LibinputNativeEventReadStatus::EventsRead
            },
            events_read,
            queued_remaining,
        }
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LibinputNativeEventReadStatus::ReadFailed,
            events_read: 0,
            queued_remaining: 0,
        }
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibinputNativeEventReadStatus {
    Idle,
    WouldBlock,
    EventsRead,
    ReadFailed,
}

#[cfg(feature = "libinput-events")]
pub trait LiveLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult;
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeLibinputDeviceMap {
    pub seat: SeatId,
    pub pointer_device: Option<DeviceId>,
    pub keyboard_device: Option<DeviceId>,
}

#[cfg(feature = "libinput-events")]
impl NativeLibinputDeviceMap {
    pub const fn new(seat: SeatId) -> Self {
        Self {
            seat,
            pointer_device: None,
            keyboard_device: None,
        }
    }

    pub const fn with_pointer_device(mut self, device: DeviceId) -> Self {
        self.pointer_device = Some(device);
        self
    }

    pub const fn with_keyboard_device(mut self, device: DeviceId) -> Self {
        self.keyboard_device = Some(device);
        self
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Debug)]
pub struct NativeLibinputEventReader {
    libinput: input::Libinput,
    devices: NativeLibinputDeviceMap,
    pointer_position: Point,
    next_serial: u64,
}

#[cfg(feature = "libinput-events")]
impl NativeLibinputEventReader {
    pub fn new(libinput: input::Libinput, devices: NativeLibinputDeviceMap) -> Self {
        Self {
            libinput,
            devices,
            pointer_position: Point { x: 0.0, y: 0.0 },
            next_serial: 1,
        }
    }

    pub const fn devices(&self) -> NativeLibinputDeviceMap {
        self.devices
    }

    pub const fn pointer_position(&self) -> Point {
        self.pointer_position
    }

    pub fn libinput_mut(&mut self) -> &mut input::Libinput {
        &mut self.libinput
    }

    fn next_serial(&mut self) -> u64 {
        let serial = self.next_serial;
        self.next_serial = self.next_serial.saturating_add(1);
        serial
    }

    fn event_packet(
        &mut self,
        device: DeviceId,
        time_msec: u64,
        kind: InputEventKind,
        global_position: Option<Point>,
    ) -> InputEventPacket {
        InputEventPacket {
            serial: self.next_serial(),
            seat: self.devices.seat,
            device,
            time_msec,
            kind,
            global_position,
            target_surface: None,
            target_window: None,
            local_position: None,
        }
    }

    fn reduce_event(&mut self, event: NativeLibinputEvent) -> Option<InputEventPacket> {
        match event {
            NativeLibinputEvent::Pointer(PointerEvent::Motion(event)) => {
                let device = self.devices.pointer_device?;
                self.pointer_position.x += event.dx();
                self.pointer_position.y += event.dy();
                Some(self.event_packet(
                    device,
                    u64::from(event.time()),
                    InputEventKind::PointerMotion,
                    Some(self.pointer_position),
                ))
            }
            NativeLibinputEvent::Pointer(PointerEvent::Button(event)) => {
                let device = self.devices.pointer_device?;
                Some(self.event_packet(
                    device,
                    u64::from(event.time()),
                    InputEventKind::PointerButton {
                        button: event.button(),
                        pressed: event.button_state() == ButtonState::Pressed,
                    },
                    Some(self.pointer_position),
                ))
            }
            NativeLibinputEvent::Keyboard(KeyboardEvent::Key(event)) => {
                let device = self.devices.keyboard_device?;
                Some(self.event_packet(
                    device,
                    u64::from(event.time()),
                    InputEventKind::Key {
                        keycode: event.key(),
                        pressed: event.key_state() == KeyState::Pressed,
                    },
                    None,
                ))
            }
            _ => None,
        }
    }
}

#[cfg(feature = "libinput-events")]
impl LiveLibinputEventReader for NativeLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult {
        if max_read == 0 {
            return LibinputNativeEventReadResult {
                report: LibinputNativeEventReadReport::idle(),
                events: Vec::new(),
            };
        }

        if self.libinput.dispatch().is_err() {
            return LibinputNativeEventReadResult {
                report: LibinputNativeEventReadReport::read_failed(),
                events: Vec::new(),
            };
        }

        let mut events = Vec::new();
        while events.len() < max_read {
            let Some(event) = self.libinput.next() else {
                break;
            };
            if let Some(packet) = self.reduce_event(event) {
                events.push(packet);
            }
        }

        LibinputNativeEventReadResult {
            report: LibinputNativeEventReadReport::events_read(events.len(), 0),
            events,
        }
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLibinputEventPoller<R> {
    reader: R,
    max_read_per_poll: usize,
    last_read: LibinputNativeEventReadReport,
}

#[cfg(feature = "libinput-events")]
impl<R> NativeLibinputEventPoller<R> {
    pub fn new(reader: R, max_read_per_poll: usize) -> Self {
        Self {
            reader,
            max_read_per_poll,
            last_read: LibinputNativeEventReadReport::idle(),
        }
    }

    pub const fn last_read_report(&self) -> LibinputNativeEventReadReport {
        self.last_read
    }

    pub const fn max_read_per_poll(&self) -> usize {
        self.max_read_per_poll
    }

    pub fn reader(&self) -> &R {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut R {
        &mut self.reader
    }
}

#[cfg(feature = "libinput-events")]
impl<R> NonBlockingInputPoller for NativeLibinputEventPoller<R>
where
    R: LiveLibinputEventReader,
{
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>> {
        let result = self.reader.read_ready_input_events(self.max_read_per_poll);
        self.last_read = result.report;
        if result.report.status == LibinputNativeEventReadStatus::ReadFailed {
            return Err(io::Error::other("reduced native libinput read failed"));
        }
        Ok(result.events)
    }
}

#[cfg(feature = "libinput-events")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FakeLiveLibinputEventReader {
    queued: VecDeque<InputEventPacket>,
    fail_next_read: bool,
}

#[cfg(feature = "libinput-events")]
impl FakeLiveLibinputEventReader {
    pub fn new(events: impl IntoIterator<Item = InputEventPacket>) -> Self {
        Self {
            queued: events.into_iter().collect(),
            fail_next_read: false,
        }
    }

    pub fn fail_next_read(&mut self) {
        self.fail_next_read = true;
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}

#[cfg(feature = "libinput-events")]
impl LiveLibinputEventReader for FakeLiveLibinputEventReader {
    fn read_ready_input_events(&mut self, max_read: usize) -> LibinputNativeEventReadResult {
        if self.fail_next_read {
            self.fail_next_read = false;
            return LibinputNativeEventReadResult {
                report: LibinputNativeEventReadReport::read_failed(),
                events: Vec::new(),
            };
        }

        let mut events = Vec::new();
        for _ in 0..max_read {
            let Some(event) = self.queued.pop_front() else {
                break;
            };
            events.push(event);
        }

        LibinputNativeEventReadResult {
            report: LibinputNativeEventReadReport::events_read(events.len(), self.queued.len()),
            events,
        }
    }
}

use crate::prelude::*;

use super::{LibinputNativeEventReadReport, LibinputNativeEventReadResult};

use input::event::{
    Event as NativeLibinputEvent,
    keyboard::{KeyState, KeyboardEvent, KeyboardEventTrait},
    pointer::{ButtonState, PointerEvent, PointerEventTrait},
};
use sophia_protocol::{InputEventKind, Point};

#[derive(Debug)]
pub struct NativeLibinputEventReader {
    libinput: input::Libinput,
    devices: NativeLibinputDeviceMap,
    pointer_position: Point,
    next_serial: u64,
}

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

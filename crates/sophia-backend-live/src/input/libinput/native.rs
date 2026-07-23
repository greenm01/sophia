use crate::prelude::*;

use super::{
    LibinputNativeEventReadReport, LibinputNativeEventReadResult, NativeLibinputEventPoller,
};

use input::DeviceCapability;
use input::event::{
    Event as NativeLibinputEvent, EventTrait,
    device::DeviceEvent,
    keyboard::{KeyState, KeyboardEvent, KeyboardEventTrait},
    pointer::{ButtonState, PointerEvent, PointerEventTrait},
};
use sophia_protocol::{InputEventKind, Point};
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct NativeLibinputEventReader {
    libinput: input::Libinput,
    devices: NativeLibinputDeviceMap,
    policy: Arc<Mutex<NativeLibinputPolicyReport>>,
    pointer_position: Point,
    next_serial: u64,
}

impl NativeLibinputEventReader {
    pub fn new(libinput: input::Libinput, devices: NativeLibinputDeviceMap) -> Self {
        Self::new_with_policy(libinput, devices, NativeLibinputPolicyReport::default())
    }

    pub fn new_with_policy(
        libinput: input::Libinput,
        devices: NativeLibinputDeviceMap,
        policy: NativeLibinputPolicyReport,
    ) -> Self {
        Self {
            libinput,
            devices,
            policy: Arc::new(Mutex::new(policy)),
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

    pub fn policy_report(&self) -> NativeLibinputPolicyReport {
        self.policy
            .lock()
            .map_or_else(|_| NativeLibinputPolicyReport::default(), |policy| *policy)
    }

    pub(crate) fn policy_handle(&self) -> Arc<Mutex<NativeLibinputPolicyReport>> {
        Arc::clone(&self.policy)
    }

    pub fn libinput_mut(&mut self) -> &mut input::Libinput {
        &mut self.libinput
    }

    pub(crate) fn libinput_mut_ref(&self) -> &input::Libinput {
        &self.libinput
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
            local_position: None,
        }
    }

    fn reduce_event(&mut self, event: NativeLibinputEvent) -> Option<InputEventPacket> {
        match event {
            NativeLibinputEvent::Device(DeviceEvent::Added(event)) => {
                let mut device = event.device();
                if let Ok(mut policy) = self.policy.lock() {
                    if !policy.udev_managed {
                        return None;
                    }
                    policy.devices_added = policy.devices_added.saturating_add(1);
                    policy.active_devices = policy.active_devices.saturating_add(1);
                    if device.has_capability(DeviceCapability::Keyboard) {
                        policy.keyboards = policy.keyboards.saturating_add(1);
                    }
                    if device.has_capability(DeviceCapability::Pointer) {
                        policy.pointers = policy.pointers.saturating_add(1);
                    }
                    if device.has_capability(DeviceCapability::Touch) {
                        policy.touch_devices = policy.touch_devices.saturating_add(1);
                    }
                    if device.config_tap_finger_count() > 0 {
                        policy.tap_capable = policy.tap_capable.saturating_add(1);
                        if device.config_tap_set_enabled(true).is_ok()
                            && device.config_tap_enabled()
                        {
                            policy.tap_enabled = policy.tap_enabled.saturating_add(1);
                        }
                    }
                }
                None
            }
            NativeLibinputEvent::Device(DeviceEvent::Removed(event)) => {
                let device = event.device();
                if let Ok(mut policy) = self.policy.lock() {
                    if policy.udev_managed {
                        policy.devices_removed = policy.devices_removed.saturating_add(1);
                        policy.active_devices = policy.active_devices.saturating_sub(1);
                        if device.has_capability(DeviceCapability::Keyboard) {
                            policy.keyboards = policy.keyboards.saturating_sub(1);
                        }
                        if device.has_capability(DeviceCapability::Pointer) {
                            policy.pointers = policy.pointers.saturating_sub(1);
                        }
                        if device.has_capability(DeviceCapability::Touch) {
                            policy.touch_devices = policy.touch_devices.saturating_sub(1);
                        }
                    }
                }
                None
            }
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectLibinputInterface;

impl input::LibinputInterface for DirectLibinputInterface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        const O_ACCMODE: i32 = 3;
        const O_WRONLY: i32 = 1;
        const O_RDWR: i32 = 2;
        let access_mode = flags & O_ACCMODE;
        std::fs::OpenOptions::new()
            .read(access_mode != O_WRONLY)
            .write(access_mode == O_WRONLY || access_mode == O_RDWR)
            .custom_flags(flags & !O_ACCMODE)
            .open(path)
            .map(Into::into)
            .map_err(|error| error.raw_os_error().unwrap_or(1))
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(fd);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLibinputOpenError {
    NoDevices,
    TooManyDevices,
    InvalidDevicePath,
    DeviceUnavailable,
    DeviceConfigurationFailed,
    SeatAssignmentFailed,
    MissingKeyboard,
    MissingPointer,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeLibinputPolicyReport {
    pub devices_added: usize,
    pub devices_removed: usize,
    pub active_devices: usize,
    pub keyboards: usize,
    pub pointers: usize,
    pub touch_devices: usize,
    pub tap_capable: usize,
    pub tap_enabled: usize,
    pub udev_managed: bool,
}

impl core::fmt::Display for NativeLibinputOpenError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "native libinput open failed: {self:?}")
    }
}

impl std::error::Error for NativeLibinputOpenError {}

pub fn open_native_libinput_path_poller(
    paths: &[PathBuf],
    devices: NativeLibinputDeviceMap,
    max_read_per_poll: usize,
) -> Result<NativeLibinputEventPoller<NativeLibinputEventReader>, NativeLibinputOpenError> {
    if paths.is_empty() {
        return Err(NativeLibinputOpenError::NoDevices);
    }
    if paths.len() > 16 {
        return Err(NativeLibinputOpenError::TooManyDevices);
    }
    let mut libinput = input::Libinput::new_from_path(DirectLibinputInterface);
    let mut policy = NativeLibinputPolicyReport::default();
    for path in paths {
        let resolved = resolve_native_libinput_device_path(path)?;
        let path = resolved
            .to_str()
            .ok_or(NativeLibinputOpenError::InvalidDevicePath)?;
        let mut device = libinput
            .path_add_device(path)
            .ok_or(NativeLibinputOpenError::DeviceUnavailable)?;
        policy.devices_added = policy.devices_added.saturating_add(1);
        policy.active_devices = policy.active_devices.saturating_add(1);
        if device.has_capability(DeviceCapability::Keyboard) {
            policy.keyboards = policy.keyboards.saturating_add(1);
        }
        if device.has_capability(DeviceCapability::Pointer) {
            policy.pointers = policy.pointers.saturating_add(1);
        }
        if device.has_capability(DeviceCapability::Touch) {
            policy.touch_devices = policy.touch_devices.saturating_add(1);
        }
        if device.config_tap_finger_count() > 0 {
            policy.tap_capable = policy.tap_capable.saturating_add(1);
            device
                .config_tap_set_enabled(true)
                .map_err(|_| NativeLibinputOpenError::DeviceConfigurationFailed)?;
            if !device.config_tap_enabled() {
                return Err(NativeLibinputOpenError::DeviceConfigurationFailed);
            }
            policy.tap_enabled = policy.tap_enabled.saturating_add(1);
        }
    }
    Ok(NativeLibinputEventPoller::new(
        NativeLibinputEventReader::new_with_policy(libinput, devices, policy),
        max_read_per_poll.clamp(1, 256),
    ))
}

pub fn open_native_libinput_udev_poller(
    seat_name: &str,
    devices: NativeLibinputDeviceMap,
    max_read_per_poll: usize,
) -> Result<NativeLibinputEventPoller<NativeLibinputEventReader>, NativeLibinputOpenError> {
    if seat_name.is_empty() || seat_name.len() > 64 || !seat_name.is_ascii() {
        return Err(NativeLibinputOpenError::SeatAssignmentFailed);
    }
    let mut libinput = input::Libinput::new_with_udev(DirectLibinputInterface);
    libinput
        .udev_assign_seat(seat_name)
        .map_err(|_| NativeLibinputOpenError::SeatAssignmentFailed)?;
    let mut reader = NativeLibinputEventReader::new_with_policy(
        libinput,
        devices,
        NativeLibinputPolicyReport {
            udev_managed: true,
            ..NativeLibinputPolicyReport::default()
        },
    );
    let _ = reader.read_ready_input_events(256);
    let policy = reader.policy_report();
    if policy.keyboards == 0 {
        return Err(NativeLibinputOpenError::MissingKeyboard);
    }
    if policy.pointers == 0 && policy.touch_devices == 0 {
        return Err(NativeLibinputOpenError::MissingPointer);
    }
    Ok(NativeLibinputEventPoller::new(
        reader,
        max_read_per_poll.clamp(1, 256),
    ))
}

pub fn resolve_native_libinput_device_path(
    path: &Path,
) -> Result<PathBuf, NativeLibinputOpenError> {
    if !path.is_absolute() {
        return Err(NativeLibinputOpenError::InvalidDevicePath);
    }
    std::fs::canonicalize(path).map_err(|_| NativeLibinputOpenError::DeviceUnavailable)
}

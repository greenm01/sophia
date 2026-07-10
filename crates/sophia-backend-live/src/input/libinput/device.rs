use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeLibinputDeviceMap {
    pub seat: SeatId,
    pub pointer_device: Option<DeviceId>,
    pub keyboard_device: Option<DeviceId>,
}

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

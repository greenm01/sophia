#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XCorePointerMapper {
    button_state: u16,
}

impl XCorePointerMapper {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn state(self) -> u16 {
        self.button_state
    }

    pub fn map_evdev_button(&mut self, evdev_button: u32, pressed: bool) -> Option<(u8, u16)> {
        let (button, mask) = match evdev_button {
            272 => (1, 1 << 8),
            274 => (2, 1 << 9),
            273 => (3, 1 << 10),
            275 => (8, 0),
            276 => (9, 0),
            _ => return None,
        };
        let state = self.button_state;
        if pressed {
            self.button_state |= mask;
        } else {
            self.button_state &= !mask;
        }
        Some((button, state))
    }
}

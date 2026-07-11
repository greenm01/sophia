#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XCoreKeyboardMapper {
    shift: u8,
    control: u8,
    alt: u8,
    caps_lock: bool,
}

impl XCoreKeyboardMapper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map_evdev_key(&mut self, evdev_keycode: u32, pressed: bool) -> Option<(u8, u16)> {
        let state = self.modifier_mask();
        match evdev_keycode {
            42 | 54 => update_modifier_count(&mut self.shift, pressed),
            29 | 97 => update_modifier_count(&mut self.control, pressed),
            56 | 100 => update_modifier_count(&mut self.alt, pressed),
            58 if pressed => self.caps_lock = !self.caps_lock,
            _ => {}
        }
        let x_keycode = evdev_keycode
            .checked_add(8)
            .and_then(|keycode| u8::try_from(keycode).ok().filter(|keycode| *keycode >= 8))?;
        Some((x_keycode, state))
    }

    pub fn modifier_mask(self) -> u16 {
        u16::from(self.shift > 0)
            | (u16::from(self.caps_lock) << 1)
            | (u16::from(self.control > 0) << 2)
            | (u16::from(self.alt > 0) << 3)
    }
}

fn update_modifier_count(count: &mut u8, pressed: bool) {
    if pressed {
        *count = count.saturating_add(1);
    } else {
        *count = count.saturating_sub(1);
    }
}

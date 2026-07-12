pub const EVDEV_KEY_BACKSPACE: u32 = 14;
pub const EVDEV_KEY_LEFTCTRL: u32 = 29;
pub const EVDEV_KEY_LEFTALT: u32 = 56;
pub const EVDEV_KEY_RIGHTALT: u32 = 100;
pub const EVDEV_KEY_RIGHTCTRL: u32 = 97;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmergencyChordAction {
    None,
    Armed,
    Triggered,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmergencyChordState {
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    backspace: bool,
    armed: bool,
    waiting_for_full_release: bool,
}

impl EmergencyChordState {
    pub const fn awaiting_arm() -> Self {
        Self {
            left_control: false,
            right_control: false,
            left_alt: false,
            right_alt: false,
            backspace: false,
            armed: false,
            waiting_for_full_release: false,
        }
    }

    pub const fn armed() -> Self {
        Self {
            armed: true,
            ..Self::awaiting_arm()
        }
    }

    pub const fn is_armed(self) -> bool {
        self.armed
    }

    pub fn observe(&mut self, keycode: u32, pressed: bool) -> EmergencyChordAction {
        match keycode {
            EVDEV_KEY_LEFTCTRL => self.left_control = pressed,
            EVDEV_KEY_RIGHTCTRL => self.right_control = pressed,
            EVDEV_KEY_LEFTALT => self.left_alt = pressed,
            EVDEV_KEY_RIGHTALT => self.right_alt = pressed,
            EVDEV_KEY_BACKSPACE => self.backspace = pressed,
            _ => return EmergencyChordAction::None,
        }

        if self.waiting_for_full_release {
            if !self.left_control
                && !self.right_control
                && !self.left_alt
                && !self.right_alt
                && !self.backspace
            {
                self.waiting_for_full_release = false;
            }
            return EmergencyChordAction::None;
        }

        let control = self.left_control || self.right_control;
        let alt = self.left_alt || self.right_alt;
        if !(control && alt && self.backspace) {
            return EmergencyChordAction::None;
        }

        self.waiting_for_full_release = true;
        if self.armed {
            EmergencyChordAction::Triggered
        } else {
            self.armed = true;
            EmergencyChordAction::Armed
        }
    }
}

impl Default for EmergencyChordState {
    fn default() -> Self {
        Self::awaiting_arm()
    }
}

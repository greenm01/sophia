use xkbcommon::xkb;

pub const XKB_RMLVO_FIELD_MAX_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XkbRmlvoConfig {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: String,
}

impl Default for XkbRmlvoConfig {
    fn default() -> Self {
        Self {
            rules: "evdev".to_owned(),
            model: "pc105".to_owned(),
            layout: "us".to_owned(),
            variant: String::new(),
            options: String::new(),
        }
    }
}

impl XkbRmlvoConfig {
    pub fn validate(&self) -> Result<(), XkbKeyboardError> {
        for value in [
            &self.rules,
            &self.model,
            &self.layout,
            &self.variant,
            &self.options,
        ] {
            if value.len() > XKB_RMLVO_FIELD_MAX_BYTES || value.as_bytes().contains(&0) {
                return Err(XkbKeyboardError::InvalidConfiguration);
            }
        }
        if self.rules.is_empty() || self.model.is_empty() || self.layout.is_empty() {
            return Err(XkbKeyboardError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XkbKeyboardError {
    InvalidConfiguration,
    KeymapCompilationFailed,
}

impl core::fmt::Display for XkbKeyboardError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid XKB RMLVO configuration",
            Self::KeymapCompilationFailed => "XKB keymap compilation failed",
        })
    }
}

impl std::error::Error for XkbKeyboardError {}

/// Immutable, client-visible description compiled from the session RMLVO.
///
/// Core X11 and XKB replies must describe the same map used by the per-seat
/// state machines. Keeping the reduced wire representation here prevents the
/// two protocol paths from silently drifting apart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XkbKeymapSnapshot {
    config: XkbRmlvoConfig,
    min_keycode: u8,
    max_keycode: u8,
    keysyms: Vec<[u32; 2]>,
    modifier_map: Vec<(u8, u8)>,
}

impl XkbKeymapSnapshot {
    pub fn new(config: &XkbRmlvoConfig) -> Result<Self, XkbKeyboardError> {
        let keymap = compile_keymap(config)?;
        // The X11 setup contract exposes the full 8..=255 core keycode range.
        // xkbcommon may report a narrower range (normally starting at 9), so
        // preserve explicit NoSymbol entries at either edge.
        let min_keycode = 8;
        let max_keycode = u8::MAX;
        let mut keysyms = Vec::with_capacity(usize::from(max_keycode - min_keycode) + 1);
        for raw in min_keycode..=max_keycode {
            let key = xkb::Keycode::new(u32::from(raw));
            let base = keymap
                .key_get_syms_by_level(key, 0, 0)
                .first()
                .map_or(0, |keysym| keysym.raw());
            let shifted = keymap
                .key_get_syms_by_level(key, 0, 1)
                .first()
                .map_or(base, |keysym| keysym.raw());
            keysyms.push([base, shifted]);
        }
        Ok(Self {
            config: config.clone(),
            min_keycode,
            max_keycode,
            keysyms,
            modifier_map: vec![
                (50, 1),
                (62, 1),
                (66, 2),
                (37, 4),
                (105, 4),
                (64, 8),
                (108, 8),
                (77, 16),
                (133, 64),
                (134, 64),
            ],
        })
    }

    pub fn config(&self) -> &XkbRmlvoConfig {
        &self.config
    }

    pub fn core_mapping(&self, first_keycode: u8, count: u8) -> Vec<u32> {
        let mut result = Vec::with_capacity(usize::from(count) * 2);
        for offset in 0..count {
            let keycode = first_keycode.saturating_add(offset);
            let pair = keycode
                .checked_sub(self.min_keycode)
                .and_then(|index| self.keysyms.get(usize::from(index)))
                .copied()
                .unwrap_or([0, 0]);
            result.extend(pair);
        }
        result
    }

    pub fn xkb_keysyms(&self) -> Vec<[u32; 2]> {
        self.keysyms.clone()
    }

    pub fn modifier_map(&self) -> Vec<(u8, u8)> {
        self.modifier_map.clone()
    }

    pub const fn min_keycode(&self) -> u8 {
        self.min_keycode
    }
    pub const fn max_keycode(&self) -> u8 {
        self.max_keycode
    }
}

fn compile_keymap(config: &XkbRmlvoConfig) -> Result<xkb::Keymap, XkbKeyboardError> {
    config.validate()?;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS | xkb::CONTEXT_NO_ENVIRONMENT_NAMES);
    xkb::Keymap::new_from_names(
        &context,
        &config.rules,
        &config.model,
        &config.layout,
        &config.variant,
        Some(config.options.clone()),
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or(XkbKeyboardError::KeymapCompilationFailed)
}

pub struct XkbKeyboardState {
    state: xkb::State,
}

impl core::fmt::Debug for XkbKeyboardState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("XkbKeyboardState")
            .finish_non_exhaustive()
    }
}

impl XkbKeyboardState {
    pub fn new(config: &XkbRmlvoConfig) -> Result<Self, XkbKeyboardError> {
        let keymap = compile_keymap(config)?;
        Ok(Self {
            state: xkb::State::new(&keymap),
        })
    }

    pub fn map_evdev_key(&mut self, evdev_keycode: u32, pressed: bool) -> Option<(u8, u16)> {
        let x_keycode = evdev_keycode
            .checked_add(8)
            .and_then(|keycode| u8::try_from(keycode).ok().filter(|keycode| *keycode >= 8))?;
        let state = self.modifier_mask();
        self.state.update_key(
            xkb::Keycode::new(u32::from(x_keycode)),
            if pressed {
                xkb::KeyDirection::Down
            } else {
                xkb::KeyDirection::Up
            },
        );
        Some((x_keycode, state))
    }

    pub fn modifier_mask(&self) -> u16 {
        u16::try_from(self.state.serialize_mods(xkb::STATE_MODS_EFFECTIVE) & 0xff).unwrap_or(0)
    }
}

impl Default for XkbKeyboardState {
    fn default() -> Self {
        Self::new(&XkbRmlvoConfig::default())
            .expect("the deterministic evdev/pc105/us XKB keymap must compile")
    }
}

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
        if evdev_keycode == 0 {
            return None;
        }
        let state = self.modifier_mask();
        match evdev_keycode {
            42 => update_modifier_bit(&mut self.shift, 1, pressed),
            54 => update_modifier_bit(&mut self.shift, 2, pressed),
            29 => update_modifier_bit(&mut self.control, 1, pressed),
            97 => update_modifier_bit(&mut self.control, 2, pressed),
            56 => update_modifier_bit(&mut self.alt, 1, pressed),
            100 => update_modifier_bit(&mut self.alt, 2, pressed),
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

fn update_modifier_bit(bits: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *bits |= bit;
    } else {
        *bits &= !bit;
    }
}

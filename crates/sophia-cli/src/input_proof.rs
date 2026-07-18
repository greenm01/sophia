#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalTextProofEvent {
    pub keycode: u8,
    pub pressed: bool,
    pub state: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalTextProofProgress {
    Awaiting,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalTextProofMismatch {
    pub event_index: usize,
    pub expected: PhysicalTextProofEvent,
    pub observed: PhysicalTextProofEvent,
}

pub const fn pointer_selection_pending(required: bool, routed_buttons: usize) -> bool {
    required && routed_buttons == 0
}

pub fn pointer_proof_suppresses_return(required: bool, keycode: u32, text_complete: bool) -> bool {
    required && text_complete && keycode == 28
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalTextProofBuildError {
    InvalidText,
    UnsupportedCharacter(u8),
}

impl core::fmt::Display for PhysicalTextProofBuildError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidText => {
                formatter.write_str("physical text proof requires 1-24 lowercase ASCII letters")
            }
            Self::UnsupportedCharacter(byte) => {
                write!(
                    formatter,
                    "physical text proof has no keycode for byte {byte}"
                )
            }
        }
    }
}

impl std::error::Error for PhysicalTextProofBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalTextProof {
    expected: Vec<PhysicalTextProofEvent>,
    matched_events: usize,
}

impl PhysicalTextProof {
    pub fn new(text: &str) -> Result<Self, PhysicalTextProofBuildError> {
        Self::build(text, true)
    }

    pub fn new_without_submit(text: &str) -> Result<Self, PhysicalTextProofBuildError> {
        Self::build(text, false)
    }

    fn build(text: &str, submit: bool) -> Result<Self, PhysicalTextProofBuildError> {
        if text.is_empty() || text.len() > 24 || !text.bytes().all(|byte| byte.is_ascii_lowercase())
        {
            return Err(PhysicalTextProofBuildError::InvalidText);
        }

        let mut expected = Vec::with_capacity((text.len() + 1).saturating_mul(2));
        for byte in text.bytes() {
            let keycode = x11_keycode_for_lowercase_ascii(byte)
                .ok_or(PhysicalTextProofBuildError::UnsupportedCharacter(byte))?;
            push_key_pair(&mut expected, keycode);
        }
        if submit {
            push_key_pair(&mut expected, 36);
        }

        Ok(Self {
            expected,
            matched_events: 0,
        })
    }

    pub fn observe(
        &mut self,
        observed: PhysicalTextProofEvent,
    ) -> Result<PhysicalTextProofProgress, PhysicalTextProofMismatch> {
        if self.is_complete() {
            return Ok(PhysicalTextProofProgress::Complete);
        }
        let expected = self.expected[self.matched_events];
        if observed != expected {
            return Err(PhysicalTextProofMismatch {
                event_index: self.matched_events,
                expected,
                observed,
            });
        }
        self.matched_events = self.matched_events.saturating_add(1);
        Ok(if self.is_complete() {
            PhysicalTextProofProgress::Complete
        } else {
            PhysicalTextProofProgress::Awaiting
        })
    }

    pub fn expected_events(&self) -> usize {
        self.expected.len()
    }

    pub fn matched_events(&self) -> usize {
        self.matched_events
    }

    pub fn is_complete(&self) -> bool {
        self.matched_events == self.expected.len()
    }
}

fn push_key_pair(expected: &mut Vec<PhysicalTextProofEvent>, keycode: u8) {
    for pressed in [true, false] {
        expected.push(PhysicalTextProofEvent {
            keycode,
            pressed,
            state: 0,
        });
    }
}

fn x11_keycode_for_lowercase_ascii(byte: u8) -> Option<u8> {
    b"qwertyuiop"
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|index| 24 + index as u8)
        .or_else(|| {
            b"asdfghjkl"
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|index| 38 + index as u8)
        })
        .or_else(|| {
            b"zxcvbnm"
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|index| 52 + index as u8)
        })
}

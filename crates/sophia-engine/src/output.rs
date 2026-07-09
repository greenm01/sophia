use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadlessOutput {
    pub id: OutputId,
    pub size: Size,
    pub scale: u32,
}

impl HeadlessOutput {
    pub const fn deterministic() -> Self {
        Self {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1280,
                height: 720,
            },
            scale: 1,
        }
    }
}

impl Default for HeadlessOutput {
    fn default() -> Self {
        Self::deterministic()
    }
}

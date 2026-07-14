use crate::{OutputId, Rect, Size};
use std::collections::BTreeSet;

pub const MAX_OUTPUT_TOPOLOGY_ENTRIES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputTopologyEntry {
    pub output: OutputId,
    pub logical: Rect,
    pub pixel_size: Size,
    pub scale: u32,
    pub refresh_millihz: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputTopologySnapshot {
    pub generation: u64,
    pub primary: OutputId,
    pub outputs: Vec<OutputTopologyEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputTopologyError {
    InvalidGeneration,
    InvalidPrimary,
    Empty,
    CapacityExceeded,
    InvalidOutput,
    DuplicateOutput,
    InvalidGeometry,
    InvalidMode,
    RootSizeExceeded,
}

impl OutputTopologySnapshot {
    pub fn validate(&self) -> Result<Size, OutputTopologyError> {
        if self.generation == 0 {
            return Err(OutputTopologyError::InvalidGeneration);
        }
        if self.outputs.is_empty() {
            return Err(OutputTopologyError::Empty);
        }
        if self.outputs.len() > MAX_OUTPUT_TOPOLOGY_ENTRIES {
            return Err(OutputTopologyError::CapacityExceeded);
        }

        let mut ids = BTreeSet::new();
        let mut right = 0i32;
        let mut bottom = 0i32;
        for entry in &self.outputs {
            if !entry.output.is_valid() {
                return Err(OutputTopologyError::InvalidOutput);
            }
            if !ids.insert(entry.output) {
                return Err(OutputTopologyError::DuplicateOutput);
            }
            if entry.logical.is_empty() || entry.logical.x < 0 || entry.logical.y < 0 {
                return Err(OutputTopologyError::InvalidGeometry);
            }
            if entry.pixel_size.width <= 0
                || entry.pixel_size.height <= 0
                || entry.scale == 0
                || entry.refresh_millihz == 0
            {
                return Err(OutputTopologyError::InvalidMode);
            }
            let entry_right = entry
                .logical
                .x
                .checked_add(entry.logical.width)
                .ok_or(OutputTopologyError::RootSizeExceeded)?;
            let entry_bottom = entry
                .logical
                .y
                .checked_add(entry.logical.height)
                .ok_or(OutputTopologyError::RootSizeExceeded)?;
            right = right.max(entry_right);
            bottom = bottom.max(entry_bottom);
        }
        if !ids.contains(&self.primary) {
            return Err(OutputTopologyError::InvalidPrimary);
        }
        if right <= 0 || bottom <= 0 || right > i32::from(u16::MAX) || bottom > i32::from(u16::MAX)
        {
            return Err(OutputTopologyError::RootSizeExceeded);
        }
        Ok(Size {
            width: right,
            height: bottom,
        })
    }

    pub fn root_size(&self) -> Result<Size, OutputTopologyError> {
        self.validate()
    }

    pub fn deterministic() -> Self {
        Self {
            generation: 1,
            primary: OutputId::from_raw(1),
            outputs: vec![OutputTopologyEntry {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 1280,
                    height: 720,
                },
                scale: 1,
                refresh_millihz: 60_000,
            }],
        }
    }
}

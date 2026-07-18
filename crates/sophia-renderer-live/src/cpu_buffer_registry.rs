use std::collections::BTreeMap;

use sophia_protocol::{Rect, Size};

use crate::LiveCpuBufferSource;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCpuBufferPatch {
    pub handle: u64,
    pub size: Size,
    pub stride: u32,
    pub format: u32,
    pub generation: u64,
    pub rect: Rect,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveCpuBufferUpdate {
    Replace(LiveCpuBufferSource),
    Patch(LiveCpuBufferPatch),
}

impl LiveCpuBufferUpdate {
    pub const fn handle(&self) -> u64 {
        match self {
            Self::Replace(buffer) => buffer.handle,
            Self::Patch(patch) => patch.handle,
        }
    }

    pub const fn generation(&self) -> u64 {
        match self {
            Self::Replace(buffer) => buffer.generation,
            Self::Patch(patch) => patch.generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveCpuBufferRegistryError {
    InvalidBufferMetadata,
    MissingPatchBase,
    PatchMetadataMismatch,
    InvalidPatchBounds,
    InvalidPatchBytes,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveCpuBufferRegistry {
    buffers: BTreeMap<u64, LiveCpuBufferSource>,
}

impl LiveCpuBufferRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        update: LiveCpuBufferUpdate,
    ) -> Result<bool, LiveCpuBufferRegistryError> {
        if self
            .buffers
            .get(&update.handle())
            .is_some_and(|current| update.generation() < current.generation)
        {
            return Ok(false);
        }
        match update {
            LiveCpuBufferUpdate::Replace(buffer) => {
                if !valid_buffer(&buffer) {
                    return Err(LiveCpuBufferRegistryError::InvalidBufferMetadata);
                }
                self.buffers.insert(buffer.handle, buffer);
            }
            LiveCpuBufferUpdate::Patch(patch) => self.apply_patch(patch)?,
        }
        Ok(true)
    }

    pub fn get(&self, handle: u64) -> Option<&LiveCpuBufferSource> {
        self.buffers.get(&handle)
    }

    pub fn retain_handles(&mut self, mut retain: impl FnMut(u64) -> bool) {
        self.buffers.retain(|handle, _| retain(*handle));
    }

    pub fn checksum(&self) -> u64 {
        self.buffers
            .values()
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, buffer| {
                buffer.bytes.iter().fold(hash, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                })
            })
    }

    fn apply_patch(&mut self, patch: LiveCpuBufferPatch) -> Result<(), LiveCpuBufferRegistryError> {
        let buffer = self
            .buffers
            .get_mut(&patch.handle)
            .ok_or(LiveCpuBufferRegistryError::MissingPatchBase)?;
        if buffer.size != patch.size
            || buffer.stride != patch.stride
            || buffer.format != patch.format
            || patch.generation < buffer.generation
        {
            return Err(LiveCpuBufferRegistryError::PatchMetadataMismatch);
        }
        let x = usize::try_from(patch.rect.x)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let y = usize::try_from(patch.rect.y)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let width = usize::try_from(patch.rect.width)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let height = usize::try_from(patch.rect.height)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let buffer_width = usize::try_from(buffer.size.width)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let buffer_height = usize::try_from(buffer.size.height)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let stride = usize::try_from(buffer.stride)
            .map_err(|_| LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let row_bytes = width
            .checked_mul(4)
            .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
        let expected = row_bytes
            .checked_mul(height)
            .ok_or(LiveCpuBufferRegistryError::InvalidPatchBytes)?;
        if width == 0
            || height == 0
            || x.checked_add(width)
                .is_none_or(|right| right > buffer_width)
            || y.checked_add(height)
                .is_none_or(|bottom| bottom > buffer_height)
            || patch.bytes.len() != expected
        {
            return Err(if patch.bytes.len() != expected {
                LiveCpuBufferRegistryError::InvalidPatchBytes
            } else {
                LiveCpuBufferRegistryError::InvalidPatchBounds
            });
        }
        for row in 0..height {
            let source = row
                .checked_mul(row_bytes)
                .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
            let target = y
                .checked_add(row)
                .and_then(|row| row.checked_mul(stride))
                .and_then(|offset| offset.checked_add(x.saturating_mul(4)))
                .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
            let target_end = target
                .checked_add(row_bytes)
                .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
            let target = buffer
                .bytes
                .get_mut(target..target_end)
                .ok_or(LiveCpuBufferRegistryError::InvalidPatchBounds)?;
            target.copy_from_slice(&patch.bytes[source..source + row_bytes]);
        }
        buffer.generation = patch.generation;
        Ok(())
    }
}

fn valid_buffer(buffer: &LiveCpuBufferSource) -> bool {
    let Ok(width) = usize::try_from(buffer.size.width) else {
        return false;
    };
    let Ok(height) = usize::try_from(buffer.size.height) else {
        return false;
    };
    let Ok(stride) = usize::try_from(buffer.stride) else {
        return false;
    };
    let Some(row_bytes) = width.checked_mul(4) else {
        return false;
    };
    let Some(byte_len) = stride.checked_mul(height) else {
        return false;
    };
    width > 0
        && height > 0
        && stride >= row_bytes
        && byte_len <= 64 * 1024 * 1024
        && buffer.bytes.len() == byte_len
}

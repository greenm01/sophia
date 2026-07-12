use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferSnapshot {
    pub handle: u64,
    pub pixmap: u32,
    pub size: Size,
    pub depth: u8,
    pub visual: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferPatchSnapshot {
    pub handle: u64,
    pub pixmap: u32,
    pub size: Size,
    pub depth: u8,
    pub visual: u32,
    pub rect: Rect,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveCpuBufferUpdate {
    Replace(CpuBufferSnapshot),
    Patch(CpuBufferPatchSnapshot),
}

impl LiveCpuBufferUpdate {
    pub const fn handle(&self) -> u64 {
        match self {
            Self::Replace(snapshot) => snapshot.handle,
            Self::Patch(patch) => patch.handle,
        }
    }

    pub fn byte_len(&self) -> usize {
        match self {
            Self::Replace(snapshot) => snapshot.bytes.len(),
            Self::Patch(patch) => patch.bytes.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferStore {
    next_handle: u64,
    buffers: BTreeMap<u64, CpuBufferSnapshot>,
    handle_by_pixmap: BTreeMap<u32, u64>,
}

impl Default for CpuBufferStore {
    fn default() -> Self {
        Self {
            next_handle: 1,
            buffers: BTreeMap::new(),
            handle_by_pixmap: BTreeMap::new(),
        }
    }
}

impl CpuBufferStore {
    pub fn upsert_pixmap(
        &mut self,
        pixmap: u32,
        size: Size,
        depth: u8,
        visual: u32,
        bytes: Vec<u8>,
    ) -> CpuBufferSnapshot {
        let handle = self
            .handle_by_pixmap
            .get(&pixmap)
            .copied()
            .unwrap_or_else(|| {
                let handle = self.next_handle;
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|next| *next != 0)
                    .expect("Sophia CPU buffer handle overflow");
                self.handle_by_pixmap.insert(pixmap, handle);
                handle
            });

        let snapshot = CpuBufferSnapshot {
            handle,
            pixmap,
            size,
            depth,
            visual,
            bytes,
        };
        self.buffers.insert(handle, snapshot.clone());
        snapshot
    }

    pub fn get(&self, handle: u64) -> Option<&CpuBufferSnapshot> {
        self.buffers.get(&handle)
    }

    pub fn patch_pixmap(
        &mut self,
        pixmap: u32,
        rect: Rect,
        bytes: Vec<u8>,
    ) -> Option<CpuBufferPatchSnapshot> {
        let handle = self.handle_for_pixmap(pixmap)?;
        let buffer = self.buffers.get_mut(&handle)?;
        let left = usize::try_from(rect.x).ok()?;
        let top = usize::try_from(rect.y).ok()?;
        let width = usize::try_from(rect.width).ok()?;
        let height = usize::try_from(rect.height).ok()?;
        let buffer_width = usize::try_from(buffer.size.width).ok()?;
        let buffer_height = usize::try_from(buffer.size.height).ok()?;
        let right = left.checked_add(width)?;
        let bottom = top.checked_add(height)?;
        if width == 0 || height == 0 || right > buffer_width || bottom > buffer_height {
            return None;
        }
        let row_bytes = width.checked_mul(4)?;
        if bytes.len() != row_bytes.checked_mul(height)? {
            return None;
        }
        let stride = buffer_width.checked_mul(4)?;
        for row in 0..height {
            let source = row.checked_mul(row_bytes)?;
            let target = top
                .checked_add(row)?
                .checked_mul(stride)?
                .checked_add(left.checked_mul(4)?)?;
            buffer
                .bytes
                .get_mut(target..target.checked_add(row_bytes)?)?
                .copy_from_slice(bytes.get(source..source.checked_add(row_bytes)?)?);
        }
        Some(CpuBufferPatchSnapshot {
            handle,
            pixmap,
            size: buffer.size,
            depth: buffer.depth,
            visual: buffer.visual,
            rect,
            bytes,
        })
    }

    pub fn handle_for_pixmap(&self, pixmap: u32) -> Option<u64> {
        self.handle_by_pixmap.get(&pixmap).copied()
    }

    pub fn remove_pixmap(&mut self, pixmap: u32) -> Option<CpuBufferSnapshot> {
        let handle = self.handle_by_pixmap.remove(&pixmap)?;
        self.buffers.remove(&handle)
    }

    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

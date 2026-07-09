use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompositePixmapMap {
    pixmaps: BTreeMap<XWindowId, CompositePixmapRecord>,
    next_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositePixmapRecord {
    pub window: XWindowId,
    pub pixmap: u32,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositePixmapLifetimeUpdate {
    pub window: XWindowId,
    pub current: Option<CompositePixmapRecord>,
    pub retired: Option<CompositePixmapRecord>,
}

impl CompositePixmapLifetimeUpdate {
    pub fn replaced_pixmap(&self) -> Option<u32> {
        self.retired.map(|record| record.pixmap)
    }
}

impl CompositePixmapMap {
    pub fn record_for_window(&self, window: XWindowId) -> Option<CompositePixmapRecord> {
        self.pixmaps.get(&window).copied()
    }

    pub fn pixmap_for_window(&self, window: XWindowId) -> Option<u32> {
        self.record_for_window(window).map(|record| record.pixmap)
    }

    pub fn upsert_named_pixmap(
        &mut self,
        window: XWindowId,
        pixmap: u32,
    ) -> CompositePixmapLifetimeUpdate {
        if let Some(current) = self.record_for_window(window) {
            if current.pixmap == pixmap {
                return CompositePixmapLifetimeUpdate {
                    window,
                    current: Some(current),
                    retired: None,
                };
            }
        }

        let generation = self.next_generation.max(1);
        self.next_generation = generation
            .checked_add(1)
            .filter(|next| *next != 0)
            .expect("Sophia composite pixmap generation overflow");
        let current = CompositePixmapRecord {
            window,
            pixmap,
            generation,
        };
        let retired = self.pixmaps.insert(window, current);

        CompositePixmapLifetimeUpdate {
            window,
            current: Some(current),
            retired,
        }
    }

    pub fn insert_named_pixmap(&mut self, window: XWindowId, pixmap: u32) -> Option<u32> {
        self.upsert_named_pixmap(window, pixmap).replaced_pixmap()
    }

    pub fn remove_window_record(
        &mut self,
        window: XWindowId,
    ) -> Option<CompositePixmapLifetimeUpdate> {
        let retired = self.pixmaps.remove(&window)?;
        Some(CompositePixmapLifetimeUpdate {
            window,
            current: None,
            retired: Some(retired),
        })
    }

    pub fn remove_window(&mut self, window: XWindowId) -> Option<u32> {
        self.remove_window_record(window)
            .and_then(|update| update.replaced_pixmap())
    }

    pub fn source_for_window(&self, window: XWindowId) -> BufferSource {
        self.pixmap_for_window(window)
            .map_or(BufferSource::None, |pixmap| BufferSource::XPixmap {
                pixmap,
            })
    }
}

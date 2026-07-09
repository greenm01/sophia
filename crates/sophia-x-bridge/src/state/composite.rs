use super::*;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeRedirectTarget {
    pub window: XWindowId,
    pub update: CompositeUpdateMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeUpdateMode {
    Automatic,
    Manual,
}

impl CompositeUpdateMode {
    pub(crate) fn to_x11(self) -> Redirect {
        match self {
            Self::Automatic => Redirect::AUTOMATIC,
            Self::Manual => Redirect::MANUAL,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceIdMap {
    next_index: u32,
    surfaces: BTreeMap<XWindowId, SurfaceId>,
}

impl SurfaceIdMap {
    pub fn surface_for_window(&mut self, window: XWindowId) -> SurfaceId {
        if let Some(surface) = self.surfaces.get(&window) {
            return *surface;
        }

        let index = self.next_index;
        self.next_index = self
            .next_index
            .checked_add(1)
            .filter(|next| *next != u32::MAX)
            .expect("Sophia surface ID map overflow");
        let surface = SurfaceId::new(index, window.generation());
        self.surfaces.insert(window, surface);
        surface
    }

    pub fn window_for_surface(&self, surface: SurfaceId) -> Option<XWindowId> {
        self.surfaces
            .iter()
            .find_map(|(window, candidate)| (*candidate == surface).then_some(*window))
    }
}

pub fn close_target_for_surface(
    mirror: &XMirrorState,
    surfaces: &SurfaceIdMap,
    surface: SurfaceId,
) -> Option<XWindowId> {
    let window = surfaces.window_for_surface(surface)?;
    let mirrored = mirror
        .windows()
        .iter()
        .find(|mirror| mirror.window == window)?;

    mirrored
        .client
        .or(mirrored.toplevel)
        .or(Some(mirrored.window))
        .filter(|window| window.is_valid())
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DamageRecord {
    pub window: XWindowId,
    pub damage: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DamageTracker {
    damage_by_window: BTreeMap<XWindowId, u32>,
    window_by_damage: BTreeMap<u32, XWindowId>,
    pending_by_window: BTreeMap<XWindowId, Region>,
}

impl DamageTracker {
    pub fn insert_damage(&mut self, window: XWindowId, damage: u32) -> Option<u32> {
        let old_damage = self.damage_by_window.insert(window, damage);
        if let Some(old_damage) = old_damage {
            self.window_by_damage.remove(&old_damage);
        }
        self.window_by_damage.insert(damage, window);
        old_damage
    }

    pub fn damage_for_window(&self, window: XWindowId) -> Option<u32> {
        self.damage_by_window.get(&window).copied()
    }

    pub fn window_for_damage(&self, damage: u32) -> Option<XWindowId> {
        self.window_by_damage.get(&damage).copied()
    }

    pub fn record_for_window(&self, window: XWindowId) -> Option<DamageRecord> {
        self.damage_for_window(window)
            .map(|damage| DamageRecord { window, damage })
    }

    pub fn pending_damage(&self, window: XWindowId) -> Option<&Region> {
        self.pending_by_window.get(&window)
    }

    pub fn drain_damage(&mut self, window: XWindowId) -> Region {
        self.pending_by_window
            .remove(&window)
            .unwrap_or_else(Region::empty)
    }

    pub fn remove_window(&mut self, window: XWindowId) -> Option<u32> {
        self.pending_by_window.remove(&window);
        let damage = self.damage_by_window.remove(&window)?;
        self.window_by_damage.remove(&damage);
        Some(damage)
    }

    pub fn apply_event(&mut self, event: XDamageEvent) -> bool {
        if self.window_for_damage(event.damage) != Some(event.window) {
            return false;
        }

        self.pending_by_window
            .entry(event.window)
            .or_insert_with(Region::empty)
            .push(event.area);
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDamageEvent {
    pub window: XWindowId,
    pub damage: u32,
    pub drawable: XWindowId,
    pub timestamp: u32,
    pub area: Rect,
    pub drawable_geometry: Rect,
}

impl XDamageEvent {
    pub fn from_x11_event(event: &Event, tracker: &DamageTracker) -> Option<Self> {
        let Event::DamageNotify(event) = event else {
            return None;
        };
        let window = tracker.window_for_damage(event.damage)?;

        Some(Self {
            window,
            damage: event.damage,
            drawable: wrap_xid(event.drawable),
            timestamp: event.timestamp,
            area: Rect {
                x: i32::from(event.area.x),
                y: i32::from(event.area.y),
                width: i32::from(event.area.width),
                height: i32::from(event.area.height),
            },
            drawable_geometry: Rect {
                x: i32::from(event.geometry.x),
                y: i32::from(event.geometry.y),
                width: i32::from(event.geometry.width),
                height: i32::from(event.geometry.height),
            },
        })
    }
}

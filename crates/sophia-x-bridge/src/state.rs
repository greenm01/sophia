use crate::prelude::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XMirrorState {
    windows: Vec<XWindowMirror>,
}

pub const MAX_CLIENT_CLASS_KEY_LEN: usize = 128;
pub const DEFAULT_SYNC_TIMEOUT_STRIKE_LIMIT: u32 = 3;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientClassKey(String);

impl ClientClassKey {
    pub fn new(value: impl AsRef<str>) -> Option<Self> {
        let value = value.as_ref().trim();
        if value.is_empty()
            || value.len() > MAX_CLIENT_CLASS_KEY_LEN
            || value.chars().any(char::is_control)
        {
            return None;
        }

        Some(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSyncProfile {
    pub window: XWindowId,
    pub namespace: Option<NamespaceId>,
    pub class_key: Option<ClientClassKey>,
    pub advertised_sync: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncReputationTracker {
    timeout_strikes: BTreeMap<(Option<NamespaceId>, ClientClassKey), u32>,
    strike_limit: u32,
}

impl SyncReputationTracker {
    pub fn new(strike_limit: u32) -> Self {
        Self {
            timeout_strikes: BTreeMap::new(),
            strike_limit,
        }
    }

    pub fn record_timeout(&mut self, namespace: Option<NamespaceId>, class_key: &ClientClassKey) {
        let strikes = self
            .timeout_strikes
            .entry((namespace, class_key.clone()))
            .or_insert(0);
        *strikes = strikes.saturating_add(1);
    }

    pub fn strikes_for(&self, namespace: Option<NamespaceId>, class_key: &ClientClassKey) -> u32 {
        self.timeout_strikes
            .get(&(namespace, class_key.clone()))
            .copied()
            .unwrap_or(0)
    }

    pub fn is_downgraded(
        &self,
        namespace: Option<NamespaceId>,
        class_key: &ClientClassKey,
    ) -> bool {
        self.strikes_for(namespace, class_key) >= self.strike_limit()
    }

    pub fn strike_limit(&self) -> u32 {
        if self.strike_limit == 0 {
            DEFAULT_SYNC_TIMEOUT_STRIKE_LIMIT
        } else {
            self.strike_limit
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceSyncRegistry {
    profiles: BTreeMap<XWindowId, ClientSyncProfile>,
    reputation: SyncReputationTracker,
}

impl SurfaceSyncRegistry {
    pub fn new(reputation: SyncReputationTracker) -> Self {
        Self {
            profiles: BTreeMap::new(),
            reputation,
        }
    }

    pub fn upsert_profile(&mut self, profile: ClientSyncProfile) {
        self.profiles.insert(profile.window, profile);
    }

    pub fn record_timeout_for_window(&mut self, window: XWindowId) -> bool {
        let Some(profile) = self.profiles.get(&window) else {
            return false;
        };
        let Some(class_key) = &profile.class_key else {
            return false;
        };

        self.reputation.record_timeout(profile.namespace, class_key);
        true
    }

    pub fn capability_for_window(&self, window: XWindowId) -> ResizeSyncCapability {
        let Some(profile) = self.profiles.get(&window) else {
            return ResizeSyncCapability::ImplicitOnly;
        };
        if !profile.advertised_sync {
            return ResizeSyncCapability::ImplicitOnly;
        }
        if let Some(class_key) = &profile.class_key {
            if self.reputation.is_downgraded(profile.namespace, class_key) {
                return ResizeSyncCapability::ImplicitOnly;
            }
        }

        ResizeSyncCapability::ExplicitSync
    }
}

pub fn sync_capability_from_wm_protocols(
    protocols: &[Atom],
    net_wm_sync_request_atom: Atom,
) -> ResizeSyncCapability {
    if protocols.contains(&net_wm_sync_request_atom) {
        ResizeSyncCapability::ExplicitSync
    } else {
        ResizeSyncCapability::ImplicitOnly
    }
}

impl XMirrorState {
    pub fn ingest_window(&mut self, mirror: XWindowMirror) {
        self.windows.push(mirror);
    }

    pub fn windows(&self) -> &[XWindowMirror] {
        &self.windows
    }

    pub fn emit_mirrors(&self) -> Vec<XWindowMirror> {
        self.windows.clone()
    }

    pub fn namespace_for_window(&self, window: XWindowId) -> Option<NamespaceId> {
        self.windows
            .iter()
            .find(|mirror| {
                mirror.window == window
                    || mirror.client == Some(window)
                    || mirror.toplevel == Some(window)
            })
            .and_then(|mirror| mirror.namespace)
    }

    pub fn apply_namespace_ownership(&mut self, ownership: &[NamespaceOwnership]) {
        for ownership in ownership {
            if !ownership.window.is_valid() || !ownership.namespace.is_valid() {
                continue;
            }

            for mirror in &mut self.windows {
                if mirror.window == ownership.window
                    || mirror.client == Some(ownership.window)
                    || mirror.toplevel == Some(ownership.window)
                {
                    mirror.namespace = Some(ownership.namespace);
                    mirror.stale_metadata = mirror.stale_metadata.saturating_add(1);
                }
            }
        }
    }

    pub fn apply_event(&mut self, event: XMirrorEvent) {
        match event {
            XMirrorEvent::Map { window } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.mapped = true;
                }
            }
            XMirrorEvent::Unmap { window } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.mapped = false;
                }
            }
            XMirrorEvent::Destroy { window } => {
                self.remove_window(window);
            }
            XMirrorEvent::Configure {
                window,
                geometry,
                above_sibling,
            } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.geometry = geometry;
                }
                self.apply_restack(window, above_sibling);
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Reparent { window, parent } => {
                self.reparent_window(window, parent);
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Property { window, .. } => {
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Restack { window, place } => {
                self.apply_circulate(window, place);
                self.mark_metadata_stale(window);
            }
        }
    }

    pub fn apply_client_hints(&mut self, hints: &XClientHints) {
        let client_windows = hints
            .ewmh_clients
            .iter()
            .chain(hints.icccm_clients.iter())
            .copied()
            .collect::<BTreeSet<_>>();

        for client in client_windows {
            let toplevel = self.toplevel_for_client(client).unwrap_or(client);

            if let Some(client_mirror) = self.window_mut(client) {
                client_mirror.client = Some(client);
                client_mirror.toplevel = Some(toplevel);
            }

            if let Some(toplevel_mirror) = self.window_mut(toplevel) {
                toplevel_mirror.client = Some(client);
                toplevel_mirror.toplevel = Some(toplevel);
            }
        }
    }

    pub fn apply_unmanaged_client_fallback(&mut self) {
        let root_windows = self
            .windows
            .iter()
            .filter(|mirror| mirror.parent.is_none())
            .map(|mirror| mirror.window)
            .collect::<BTreeSet<_>>();
        let fallback_clients = self
            .windows
            .iter()
            .filter(|mirror| mirror.client.is_none() && mirror.mapped)
            .filter(|mirror| {
                mirror
                    .parent
                    .is_some_and(|parent| root_windows.contains(&parent))
            })
            .map(|mirror| mirror.window)
            .collect::<Vec<_>>();

        for client in fallback_clients {
            if let Some(mirror) = self.window_mut(client) {
                mirror.client = Some(client);
                mirror.toplevel = Some(client);
            }
        }
    }

    pub fn emit_surfaces(
        &self,
        surfaces: &mut SurfaceIdMap,
        pixmaps: &CompositePixmapMap,
    ) -> Vec<SurfaceSnapshot> {
        self.emit_surfaces_with_sync(surfaces, pixmaps, None)
    }

    pub fn emit_surfaces_with_sync(
        &self,
        surfaces: &mut SurfaceIdMap,
        pixmaps: &CompositePixmapMap,
        sync: Option<&SurfaceSyncRegistry>,
    ) -> Vec<SurfaceSnapshot> {
        self.windows
            .iter()
            .filter(|mirror| mirror.client.is_some())
            .map(|mirror| SurfaceSnapshot {
                surface: surfaces.surface_for_window(mirror.window),
                window: mirror.window,
                toplevel: mirror.toplevel,
                client: mirror.client,
                namespace: mirror.namespace,
                mapped: mirror.mapped,
                stack_rank: mirror.stack_rank,
                geometry: mirror.geometry,
                source: mirror.client.map_or(BufferSource::None, |client| {
                    pixmaps.source_for_window(client)
                }),
                damage: Region::single(mirror.geometry),
                generation: mirror.stale_metadata,
                resize_sync: mirror
                    .client
                    .and_then(|client| sync.map(|sync| sync.capability_for_window(client)))
                    .unwrap_or(ResizeSyncCapability::ImplicitOnly),
            })
            .collect()
    }

    pub fn emit_layers(
        &self,
        surfaces: &mut SurfaceIdMap,
        pixmaps: &CompositePixmapMap,
    ) -> Vec<LayerSnapshot> {
        self.emit_surfaces(surfaces, pixmaps)
            .into_iter()
            .filter(|surface| surface.mapped && !surface.geometry.is_empty())
            .map(|surface| LayerSnapshot {
                surface: surface.surface,
                window: Some(surface.window),
                namespace: surface.namespace,
                stack_rank: surface.stack_rank,
                geometry: surface.geometry,
                source: surface.source,
                damage: surface.damage,
                opacity: 1.0,
                crop: None,
                transform: Transform::IDENTITY,
                generation: surface.generation,
                resize_sync: surface.resize_sync,
            })
            .collect()
    }

    pub fn composite_redirect_targets(&self) -> Vec<CompositeRedirectTarget> {
        self.windows
            .iter()
            .filter(|mirror| mirror.mapped)
            .filter_map(|mirror| mirror.client)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|window| CompositeRedirectTarget {
                window,
                update: CompositeUpdateMode::Manual,
            })
            .collect()
    }

    fn window_mut(&mut self, window: XWindowId) -> Option<&mut XWindowMirror> {
        self.windows
            .iter_mut()
            .find(|mirror| mirror.window == window)
    }

    fn remove_window(&mut self, window: XWindowId) {
        self.windows.retain(|mirror| mirror.window != window);
        for mirror in &mut self.windows {
            mirror.children.retain(|child| *child != window);
        }
    }

    fn reparent_window(&mut self, window: XWindowId, parent: Option<XWindowId>) {
        for mirror in &mut self.windows {
            mirror.children.retain(|child| *child != window);
        }

        if let Some(mirror) = self.window_mut(window) {
            mirror.parent = parent;
        }

        if let Some(parent) = parent {
            if let Some(parent) = self.window_mut(parent) {
                if !parent.children.contains(&window) {
                    parent.children.push(window);
                }
            }
        }
    }

    fn apply_restack(&mut self, window: XWindowId, above_sibling: Option<XWindowId>) {
        let stack_rank = above_sibling
            .and_then(|sibling| self.windows.iter().find(|mirror| mirror.window == sibling))
            .map_or(0, |sibling| sibling.stack_rank.saturating_add(1));

        if let Some(mirror) = self.window_mut(window) {
            mirror.stack_rank = stack_rank;
        }
    }

    fn apply_circulate(&mut self, window: XWindowId, place: RestackPlace) {
        let rank = match place {
            RestackPlace::OnTop => self
                .windows
                .iter()
                .map(|mirror| mirror.stack_rank)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            RestackPlace::OnBottom => 0,
        };

        if let Some(mirror) = self.window_mut(window) {
            mirror.stack_rank = rank;
        }
    }

    fn mark_metadata_stale(&mut self, window: XWindowId) {
        if let Some(mirror) = self.window_mut(window) {
            mirror.stale_metadata = mirror.stale_metadata.saturating_add(1);
        }
    }

    fn toplevel_for_client(&self, client: XWindowId) -> Option<XWindowId> {
        let mut current = client;

        loop {
            let mirror = self
                .windows
                .iter()
                .find(|mirror| mirror.window == current)?;
            let Some(parent) = mirror.parent else {
                return Some(current);
            };
            let Some(parent_mirror) = self.windows.iter().find(|mirror| mirror.window == parent)
            else {
                return Some(current);
            };

            if parent_mirror.parent.is_none() {
                return Some(current);
            }

            current = parent;
        }
    }
}

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

impl XSelectionEvent {
    pub fn from_x11_event(event: &Event) -> Option<Self> {
        let Event::XfixesSelectionNotify(event) = event else {
            return None;
        };

        Some(Self {
            selection: event.selection,
            owner: nonzero_window(event.owner).map(wrap_xid),
            timestamp: event.timestamp,
            selection_timestamp: event.selection_timestamp,
            kind: selection_change_kind(event.subtype),
        })
    }
}

fn selection_change_kind(kind: SelectionEvent) -> XSelectionChangeKind {
    if kind == SelectionEvent::SET_SELECTION_OWNER {
        XSelectionChangeKind::SetOwner
    } else if kind == SelectionEvent::SELECTION_WINDOW_DESTROY {
        XSelectionChangeKind::OwnerWindowDestroyed
    } else if kind == SelectionEvent::SELECTION_CLIENT_CLOSE {
        XSelectionChangeKind::OwnerClientClosed
    } else {
        XSelectionChangeKind::Unknown
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XMirrorEvent {
    Map {
        window: XWindowId,
    },
    Unmap {
        window: XWindowId,
    },
    Destroy {
        window: XWindowId,
    },
    Configure {
        window: XWindowId,
        geometry: Rect,
        above_sibling: Option<XWindowId>,
    },
    Reparent {
        window: XWindowId,
        parent: Option<XWindowId>,
    },
    Property {
        window: XWindowId,
        atom: u32,
        deleted: bool,
    },
    Restack {
        window: XWindowId,
        place: RestackPlace,
    },
}

impl XMirrorEvent {
    pub fn from_x11_event(event: &Event) -> Option<Self> {
        match event {
            Event::MapNotify(event) => Some(Self::Map {
                window: wrap_xid(event.window),
            }),
            Event::UnmapNotify(event) => Some(Self::Unmap {
                window: wrap_xid(event.window),
            }),
            Event::DestroyNotify(event) => Some(Self::Destroy {
                window: wrap_xid(event.window),
            }),
            Event::ConfigureNotify(event) => Some(Self::Configure {
                window: wrap_xid(event.window),
                geometry: Rect {
                    x: i32::from(event.x),
                    y: i32::from(event.y),
                    width: i32::from(event.width),
                    height: i32::from(event.height),
                },
                above_sibling: nonzero_window(event.above_sibling).map(wrap_xid),
            }),
            Event::ReparentNotify(event) => Some(Self::Reparent {
                window: wrap_xid(event.window),
                parent: nonzero_window(event.parent).map(wrap_xid),
            }),
            Event::PropertyNotify(event) => Some(Self::Property {
                window: wrap_xid(event.window),
                atom: event.atom,
                deleted: u8::from(event.state) == 1,
            }),
            Event::CirculateNotify(event) => Some(Self::Restack {
                window: wrap_xid(event.window),
                place: RestackPlace::from_x11(event.place),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestackPlace {
    OnTop,
    OnBottom,
}

impl RestackPlace {
    fn from_x11(place: Place) -> Self {
        if u8::from(place) == u8::from(Place::ON_BOTTOM) {
            Self::OnBottom
        } else {
            Self::OnTop
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XBridgeError {
    Connect {
        message: String,
    },
    InvalidScreen {
        screen_num: usize,
    },
    QueryExtension {
        extension: RequiredExtension,
        message: String,
    },
    QueryTree {
        window: u32,
        message: String,
    },
    WindowAttributes {
        window: u32,
        message: String,
    },
    WindowGeometry {
        window: u32,
        message: String,
    },
    InternAtom {
        atom: String,
        message: String,
    },
    GetProperty {
        window: u32,
        property: u32,
        message: String,
    },
    PoliteClose {
        window: u32,
        message: String,
    },
    CompositeVersion {
        message: String,
    },
    CompositeRedirect {
        window: u32,
        message: String,
    },
    CompositeNamePixmap {
        window: u32,
        pixmap: u32,
        message: String,
    },
    GenerateId {
        message: String,
    },
    DamageVersion {
        message: String,
    },
    DamageCreate {
        window: u32,
        damage: u32,
        message: String,
    },
    PixmapGeometry {
        pixmap: u32,
        message: String,
    },
    PixmapReadback {
        pixmap: u32,
        message: String,
    },
    TestClient {
        message: String,
    },
    RoutedInput {
        message: String,
    },
    SelectionMonitor {
        message: String,
    },
}

impl fmt::Display for XBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { message } => write!(f, "failed to connect to X display: {message}"),
            Self::InvalidScreen { screen_num } => write!(f, "invalid X screen {screen_num}"),
            Self::QueryExtension { extension, message } => {
                write!(
                    f,
                    "failed to query {} extension: {message}",
                    extension.name()
                )
            }
            Self::QueryTree { window, message } => {
                write!(
                    f,
                    "failed to query X window tree for {window:#x}: {message}"
                )
            }
            Self::WindowAttributes { window, message } => {
                write!(
                    f,
                    "failed to query X window attributes for {window:#x}: {message}"
                )
            }
            Self::WindowGeometry { window, message } => {
                write!(
                    f,
                    "failed to query X window geometry for {window:#x}: {message}"
                )
            }
            Self::InternAtom { atom, message } => {
                write!(f, "failed to intern X atom {atom}: {message}")
            }
            Self::GetProperty {
                window,
                property,
                message,
            } => {
                write!(
                    f,
                    "failed to get X property {property:#x} from {window:#x}: {message}"
                )
            }
            Self::PoliteClose { window, message } => {
                write!(
                    f,
                    "failed to request polite close for {window:#x}: {message}"
                )
            }
            Self::CompositeVersion { message } => {
                write!(f, "failed to negotiate XComposite version: {message}")
            }
            Self::CompositeRedirect { window, message } => {
                write!(
                    f,
                    "failed to redirect X window {window:#x} with XComposite: {message}"
                )
            }
            Self::CompositeNamePixmap {
                window,
                pixmap,
                message,
            } => {
                write!(
                    f,
                    "failed to name XComposite pixmap {pixmap:#x} for X window {window:#x}: {message}"
                )
            }
            Self::GenerateId { message } => {
                write!(f, "failed to allocate an X resource ID: {message}")
            }
            Self::DamageVersion { message } => {
                write!(f, "failed to negotiate X Damage version: {message}")
            }
            Self::DamageCreate {
                window,
                damage,
                message,
            } => {
                write!(
                    f,
                    "failed to create X Damage object {damage:#x} for X window {window:#x}: {message}"
                )
            }
            Self::PixmapGeometry { pixmap, message } => {
                write!(
                    f,
                    "failed to query X pixmap geometry for {pixmap:#x}: {message}"
                )
            }
            Self::PixmapReadback { pixmap, message } => {
                write!(f, "failed to read X pixmap {pixmap:#x}: {message}")
            }
            Self::TestClient { message } => {
                write!(f, "failed to run Sophia X test client: {message}")
            }
            Self::RoutedInput { message } => {
                write!(f, "failed to run Sophia routed-input smoke: {message}")
            }
            Self::SelectionMonitor { message } => {
                write!(f, "failed to monitor X selections: {message}")
            }
        }
    }
}

impl std::error::Error for XBridgeError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequiredExtension {
    Composite,
    Damage,
    XFixes,
    Shape,
    Render,
}

impl RequiredExtension {
    pub const ALL: [Self; 5] = [
        Self::Composite,
        Self::Damage,
        Self::XFixes,
        Self::Shape,
        Self::Render,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Composite => "Composite",
            Self::Damage => "DAMAGE",
            Self::XFixes => "XFIXES",
            Self::Shape => "SHAPE",
            Self::Render => "RENDER",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStatus {
    pub extension: RequiredExtension,
    pub present: bool,
    pub major_opcode: Option<u8>,
    pub first_event: Option<u8>,
    pub first_error: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceRecord {
    pub namespace: NamespaceId,
    pub label: String,
    pub source: NamespaceSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespaceSource {
    StaticConfig,
    XServer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticNamespaceConfig {
    namespaces: Vec<NamespaceRecord>,
}

impl StaticNamespaceConfig {
    pub fn new(namespaces: Vec<NamespaceRecord>) -> Self {
        Self { namespaces }
    }

    pub fn namespaces(&self) -> &[NamespaceRecord] {
        &self.namespaces
    }

    pub fn record_namespace(&mut self, record: NamespaceRecord) {
        if let Some(existing) = self
            .namespaces
            .iter_mut()
            .find(|existing| existing.namespace == record.namespace)
        {
            *existing = record;
            return;
        }

        self.namespaces.push(record);
    }

    pub fn with_discovered(mut self, records: impl IntoIterator<Item = NamespaceRecord>) -> Self {
        for record in records {
            self.record_namespace(record);
        }

        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceOwnership {
    pub window: XWindowId,
    pub namespace: NamespaceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XConnectionProbe {
    pub display_name: Option<String>,
    pub screen_num: usize,
    pub required_extensions: Vec<ExtensionStatus>,
    pub namespaces: StaticNamespaceConfig,
}

impl XConnectionProbe {
    pub fn missing_extensions(&self) -> Vec<RequiredExtension> {
        self.required_extensions
            .iter()
            .filter(|status| !status.present)
            .map(|status| status.extension)
            .collect()
    }

    pub fn has_required_extensions(&self) -> bool {
        self.missing_extensions().is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRootImport {
    pub probe: XConnectionProbe,
    pub mirror: XMirrorState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestClientConfig {
    pub display_name: Option<String>,
    pub size: Size,
    pub hold_millis: u64,
}

impl Default for TestClientConfig {
    fn default() -> Self {
        Self {
            display_name: None,
            size: Size {
                width: 320,
                height: 200,
            },
            hold_millis: 5_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestClientWindow {
    pub window: XWindowId,
    pub size: Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeReadbackReport {
    pub display_name: Option<String>,
    pub mirrored_windows: usize,
    pub surfaces: usize,
    pub renderable_layers: usize,
    pub redirect_targets: usize,
    pub readbacks: usize,
    pub total_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeReadbackCapture {
    pub report: SmokeReadbackReport,
    pub surfaces: Vec<SurfaceSnapshot>,
    pub layers: Vec<LayerSnapshot>,
    pub readbacks: Vec<CpuBufferSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAtoms {
    pub wm_state: Atom,
    pub net_client_list: Atom,
    pub wm_protocols: Atom,
    pub wm_delete_window: Atom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionAtoms {
    pub primary: Atom,
    pub secondary: Atom,
    pub clipboard: Atom,
}

impl XSelectionAtoms {
    pub const fn all(self) -> [Atom; 3] {
        [self.primary, self.secondary, self.clipboard]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XSelectionChangeKind {
    SetOwner,
    OwnerWindowDestroyed,
    OwnerClientClosed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionEvent {
    pub selection: Atom,
    pub owner: Option<XWindowId>,
    pub timestamp: u32,
    pub selection_timestamp: u32,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerRecord {
    pub selection: Atom,
    pub namespace: Option<NamespaceId>,
    pub owner: Option<XWindowId>,
    pub generation: u64,
    pub timestamp: u32,
    pub selection_timestamp: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XSelectionOwnerUpdate {
    pub previous: Option<XSelectionOwnerRecord>,
    pub current: XSelectionOwnerRecord,
    pub kind: XSelectionChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardPortalOwnerChange {
    pub source_namespace: NamespaceId,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionFailureRequest {
    pub transfer: PortalTransferId,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub time: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionPortalRequest {
    pub request: ClipboardTransferRequest,
    pub failure: ClipboardSelectionFailureRequest,
    pub property: Atom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionRequestError {
    UnknownRequestorNamespace,
    UnknownSourceOwner,
    MissingSourceNamespace,
    SameNamespace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardSelectionDispatch {
    pub portal_request: ClipboardSelectionPortalRequest,
    pub command: PortalCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionDispatchError {
    NotSelectionRequest,
    Request(ClipboardSelectionRequestError),
    Portal(PortalError),
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XSelectionMonitor {
    owners: BTreeMap<(Atom, Option<NamespaceId>), XSelectionOwnerRecord>,
}

impl XSelectionMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn owner(
        &self,
        selection: Atom,
        namespace: Option<NamespaceId>,
    ) -> Option<XSelectionOwnerRecord> {
        self.owners.get(&(selection, namespace)).copied()
    }

    pub fn current_owner_for_selection(&self, selection: Atom) -> Option<XSelectionOwnerRecord> {
        self.owners
            .values()
            .filter(|record| record.selection == selection && record.owner.is_some())
            .max_by_key(|record| record.generation)
            .copied()
    }

    pub fn apply_event(
        &mut self,
        event: XSelectionEvent,
        mirror: &XMirrorState,
    ) -> XSelectionOwnerUpdate {
        let namespace_from_owner = event
            .owner
            .and_then(|owner| mirror.namespace_for_window(owner));
        let namespace =
            namespace_from_owner.or_else(|| self.namespace_for_existing_selection(event.selection));
        let key = (event.selection, namespace);
        let previous = self.owners.get(&key).copied();
        let generation = previous
            .map(|record| record.generation.saturating_add(1))
            .unwrap_or(1);
        let current = XSelectionOwnerRecord {
            selection: event.selection,
            namespace,
            owner: event.owner,
            generation,
            timestamp: event.timestamp,
            selection_timestamp: event.selection_timestamp,
        };

        self.owners.insert(key, current);

        XSelectionOwnerUpdate {
            previous,
            current,
            kind: event.kind,
        }
    }

    fn namespace_for_existing_selection(&self, selection: Atom) -> Option<NamespaceId> {
        self.owners
            .iter()
            .find_map(|((record_selection, namespace), record)| {
                if *record_selection == selection && record.owner.is_some() {
                    *namespace
                } else {
                    None
                }
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoliteCloseOutcome {
    SentDeleteWindow { window: XWindowId },
    UnsupportedProtocol { window: XWindowId },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XClientHints {
    pub ewmh_clients: Vec<XWindowId>,
    pub icccm_clients: Vec<XWindowId>,
}

pub(crate) fn wrap_xid(window: Window) -> XWindowId {
    XWindowId::new(window, 1)
}

pub(crate) fn nonzero_window(window: Window) -> Option<Window> {
    (window != 0).then_some(window)
}

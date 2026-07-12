use super::*;

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

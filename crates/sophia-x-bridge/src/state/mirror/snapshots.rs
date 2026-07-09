use super::*;

impl XMirrorState {
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
}

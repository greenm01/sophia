use super::*;

impl HeadlessEngine {
    pub fn committed_state_from_layer(&self, layer: &LayerSnapshot) -> CommittedSurfaceState {
        CommittedSurfaceState::from_layer_snapshot(layer)
    }

    pub fn project_committed_surface_state(
        &self,
        committed: &CommittedSurfaceState,
        template: &LayerSnapshot,
    ) -> Result<LayerSnapshot, EngineError> {
        if !committed.surface.is_valid() || !template.surface.is_valid() {
            return Err(EngineError::InvalidSurface);
        }
        if committed.surface != template.surface {
            return Err(EngineError::InvalidSurface);
        }

        let mut layer = template.clone();
        layer.geometry = committed.geometry;
        layer.source = committed.buffer;
        layer.damage = committed.damage.clone();
        layer.generation = committed.committed_generation;
        Ok(layer)
    }

    pub fn project_committed_surface_states(
        &self,
        committed: &[CommittedSurfaceState],
        templates: &[LayerSnapshot],
    ) -> Result<Vec<LayerSnapshot>, EngineError> {
        let templates_by_surface = templates
            .iter()
            .map(|template| (template.surface, template))
            .collect::<BTreeMap<_, _>>();

        committed
            .iter()
            .map(|state| {
                let Some(template) = templates_by_surface.get(&state.surface) else {
                    return Err(EngineError::InvalidSurface);
                };
                self.project_committed_surface_state(state, template)
            })
            .collect()
    }
}

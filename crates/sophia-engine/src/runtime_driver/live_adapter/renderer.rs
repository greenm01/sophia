use crate::prelude::*;
use crate::{
    EngineError, HeadlessEngine, LastCommittedLayout, RenderFrameReport, SessionLayerSource,
    SessionTickReport, SessionTickRequest,
};

use super::super::observation::{
    runtime_observation_from_render_frame_report, runtime_observation_from_session_tick_report,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRendererRuntimeAdapter {
    pub layers: Vec<LayerSnapshot>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
}

impl LiveRendererRuntimeAdapter {
    pub fn from_layers(layers: Vec<LayerSnapshot>) -> Self {
        Self {
            layers,
            committed_surfaces: Vec::new(),
        }
    }

    pub fn from_committed_surface_states(
        committed_surfaces: Vec<CommittedSurfaceState>,
        layer_templates: Vec<LayerSnapshot>,
    ) -> Self {
        Self {
            layers: layer_templates,
            committed_surfaces,
        }
    }

    pub fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        let layers = if self.committed_surfaces.is_empty() {
            self.layers.clone()
        } else {
            engine.project_committed_surface_states(&self.committed_surfaces, &self.layers)?
        };

        engine.run_session_tick(
            SessionTickRequest {
                output,
                frame_serial,
                layers: SessionLayerSource::Fresh(layers),
            },
            last_committed,
        )
    }

    pub fn rendered_observation(report: &SessionTickReport) -> SessionRuntimeObservation {
        runtime_observation_from_session_tick_report(report)
    }

    pub fn from_render_frame_report(report: &RenderFrameReport) -> SessionRuntimeObservation {
        runtime_observation_from_render_frame_report(report)
    }
}

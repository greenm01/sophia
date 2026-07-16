use crate::prelude::*;
use crate::render::should_render;
use crate::{
    CpuFallbackRenderer, EngineError, FramePlanRequest, FrameRenderer, HeadlessEngine,
    RenderFrameReport, ReplayReport, ReplayStep,
};

impl HeadlessEngine {
    #[instrument(skip_all, fields(
        output = request.output.raw(),
        frame_serial = request.frame_serial,
        layer_count = layers.len()
    ))]
    pub fn plan_frame(
        &self,
        request: FramePlanRequest,
        mut layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError> {
        self.validate_output(request.output)?;

        layers.sort_by_key(|layer| layer.stack_rank);

        let mut commands = Vec::new();
        let mut damage = Region::empty();
        let mut skipped_layers = 0usize;
        let mut empty_targets = 0usize;

        for layer in &layers {
            if !layer.surface.is_valid() {
                warn!(
                    output = request.output.raw(),
                    frame_serial = request.frame_serial,
                    "rejected frame plan with invalid surface"
                );
                return Err(EngineError::InvalidSurface);
            }

            if !should_render(layer) {
                skipped_layers += 1;
                continue;
            }

            let target = layer.crop.map_or_else(
                || Region::single(layer.geometry),
                |crop| Region::single(crop),
            );

            if target.is_empty() {
                empty_targets += 1;
                continue;
            }

            damage.extend(&layer.damage);
            commands.push(RenderCommand {
                kind: RenderCommandKind::Blit,
                source: Some(layer.surface),
                output: request.output,
                target,
                clip: layer.crop.map(Region::single),
                transform: layer.transform,
                alpha: layer.opacity,
            });
        }
        let rendered_layers = commands.len();
        trace!(
            output = request.output.raw(),
            frame_serial = request.frame_serial,
            layer_count = layers.len(),
            rendered_layers,
            skipped_layers,
            empty_targets,
            "frame planning layer filter summary"
        );
        debug!(
            output = request.output.raw(),
            frame_serial = request.frame_serial,
            layer_count = layers.len(),
            render_commands = commands.len(),
            damage_rects = damage.rects.len(),
            "planned frame"
        );

        Ok(FrameSnapshot {
            output: request.output,
            output_size: self.output.size,
            output_scale: self.output.scale,
            frame_serial: request.frame_serial,
            layers,
            commands,
            damage,
        })
    }

    #[instrument(skip_all, fields(
        output = frame.output.raw(),
        frame_serial = frame.frame_serial,
        command_count = frame.commands.len()
    ))]
    pub fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError> {
        self.validate_output(frame.output)?;

        if frame.output_size != self.output.size || frame.output_scale != self.output.scale {
            warn!(
                output = frame.output.raw(),
                frame_serial = frame.frame_serial,
                "rejected frame replay with mismatched output shape"
            );
            return Err(EngineError::InvalidFrame);
        }

        let surfaces = frame
            .layers
            .iter()
            .map(|layer| layer.surface)
            .collect::<BTreeSet<_>>();
        let mut steps = Vec::with_capacity(frame.commands.len());

        for (command_index, command) in frame.commands.iter().enumerate() {
            if command.output != frame.output {
                warn!(
                    output = frame.output.raw(),
                    frame_serial = frame.frame_serial,
                    command_index,
                    command_output = command.output.raw(),
                    "rejected frame replay with command for different output"
                );
                return Err(EngineError::InvalidOutput);
            }

            if let Some(source) = command.source {
                if !source.is_valid() || !surfaces.contains(&source) {
                    warn!(
                        output = frame.output.raw(),
                        frame_serial = frame.frame_serial,
                        command_index,
                        has_source = command.source.is_some(),
                        "rejected frame replay with invalid command source"
                    );
                    return Err(EngineError::InvalidSurface);
                }
            }

            steps.push(ReplayStep {
                command_index,
                kind: command.kind,
                source: command.source,
                target: command.target.clone(),
                clip: command.clip.clone(),
                transform: command.transform,
                alpha: command.alpha,
            });
        }
        debug!(
            output = frame.output.raw(),
            frame_serial = frame.frame_serial,
            command_count = frame.commands.len(),
            replay_steps = steps.len(),
            damage_rects = frame.damage.rects.len(),
            "replayed frame"
        );

        Ok(ReplayReport {
            output: frame.output,
            output_size: frame.output_size,
            output_scale: frame.output_scale,
            frame_serial: frame.frame_serial,
            steps,
            damage: frame.damage.clone(),
        })
    }

    pub fn render_frame_with(
        &self,
        renderer: &impl FrameRenderer,
        frame: &FrameSnapshot,
    ) -> Result<RenderFrameReport, EngineError> {
        let replay = self.replay_frame(frame)?;
        renderer.render_frame(frame, replay)
    }

    pub fn render_frame(&self, frame: &FrameSnapshot) -> Result<RenderFrameReport, EngineError> {
        self.render_frame_with(&CpuFallbackRenderer, frame)
    }
}

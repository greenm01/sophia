use core::fmt;

use sophia_protocol::{
    BufferSource, FrameSnapshot, LayerSnapshot, OutputId, Region, RenderCommand, RenderCommandKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    InvalidOutput,
    InvalidSurface,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput => f.write_str("invalid output ID"),
            Self::InvalidSurface => f.write_str("invalid surface ID"),
        }
    }
}

impl std::error::Error for EngineError {}

#[derive(Clone, Copy, Debug)]
pub struct FramePlanRequest {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Default)]
pub struct HeadlessEngine;

impl HeadlessEngine {
    pub fn plan_frame(
        &self,
        request: FramePlanRequest,
        mut layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError> {
        if !request.output.is_valid() {
            return Err(EngineError::InvalidOutput);
        }

        layers.sort_by_key(|layer| layer.stack_rank);

        let mut commands = Vec::new();
        let mut damage = Region::empty();

        for layer in &layers {
            if !layer.surface.is_valid() {
                return Err(EngineError::InvalidSurface);
            }

            if !should_render(layer) {
                continue;
            }

            let target = layer.crop.map_or_else(
                || Region::single(layer.geometry),
                |crop| Region::single(crop),
            );

            if target.is_empty() {
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

        Ok(FrameSnapshot {
            output: request.output,
            frame_serial: request.frame_serial,
            layers,
            commands,
            damage,
        })
    }
}

fn should_render(layer: &LayerSnapshot) -> bool {
    layer.opacity > 0.0 && !layer.geometry.is_empty() && layer.source != BufferSource::None
}

#[cfg(test)]
fn test_layer(surface_index: u32, stack_rank: u32, x: i32, damage: Region) -> LayerSnapshot {
    use sophia_protocol::{Rect, SurfaceId, Transform};

    LayerSnapshot {
        surface: SurfaceId::new(surface_index, 1),
        window: None,
        namespace: None,
        stack_rank,
        geometry: Rect {
            x,
            y: 0,
            width: 100,
            height: 100,
        },
        source: BufferSource::CpuBuffer {
            handle: u64::from(surface_index) + 1,
        },
        damage,
        opacity: 1.0,
        crop: None,
        transform: Transform::IDENTITY,
        generation: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sophia_protocol::{Rect, SurfaceId};

    #[test]
    fn headless_engine_returns_frame_value() {
        let engine = HeadlessEngine;
        let request = FramePlanRequest {
            output: OutputId::from_raw(1),
            frame_serial: 7,
        };
        let frame = engine.plan_frame(request, Vec::new()).unwrap();

        assert_eq!(frame.output, request.output);
        assert_eq!(frame.frame_serial, 7);
        assert!(frame.layers.is_empty());
        assert!(frame.commands.is_empty());
    }

    #[test]
    fn frame_plan_sorts_layers_by_stack_rank() {
        let engine = HeadlessEngine;
        let request = FramePlanRequest {
            output: OutputId::from_raw(1),
            frame_serial: 1,
        };
        let frame = engine
            .plan_frame(
                request,
                vec![
                    test_layer(0, 20, 20, Region::empty()),
                    test_layer(1, 10, 10, Region::empty()),
                ],
            )
            .unwrap();

        assert_eq!(frame.layers[0].stack_rank, 10);
        assert_eq!(frame.layers[1].stack_rank, 20);
        assert_eq!(frame.commands[0].source, Some(frame.layers[0].surface));
    }

    #[test]
    fn frame_plan_aggregates_layer_damage() {
        let engine = HeadlessEngine;
        let request = FramePlanRequest {
            output: OutputId::from_raw(1),
            frame_serial: 1,
        };
        let frame = engine
            .plan_frame(
                request,
                vec![
                    test_layer(
                        0,
                        0,
                        0,
                        Region::single(Rect {
                            x: 0,
                            y: 0,
                            width: 10,
                            height: 10,
                        }),
                    ),
                    test_layer(
                        1,
                        1,
                        100,
                        Region::single(Rect {
                            x: 100,
                            y: 0,
                            width: 5,
                            height: 5,
                        }),
                    ),
                ],
            )
            .unwrap();

        assert_eq!(frame.damage.rects.len(), 2);
    }

    #[test]
    fn frame_plan_rejects_stale_surface() {
        let engine = HeadlessEngine;
        let request = FramePlanRequest {
            output: OutputId::from_raw(1),
            frame_serial: 1,
        };
        let mut layer = test_layer(0, 0, 0, Region::empty());
        layer.surface = SurfaceId::INVALID;

        assert_eq!(
            engine.plan_frame(request, vec![layer]),
            Err(EngineError::InvalidSurface)
        );
    }
}

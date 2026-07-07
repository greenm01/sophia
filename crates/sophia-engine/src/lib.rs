use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::{
    BufferSource, ChromeDescriptor, FrameSnapshot, LayerSnapshot, LayoutTransaction, OutputId,
    Rect, Region, RenderCommand, RenderCommandKind, Size, SurfaceId, TransactionCommit,
    TransactionId, TransactionOutcome,
};
use sophia_runtime::{SophiaErrorExt, SophiaErrorKind};
use tracing::instrument;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    InvalidOutput,
    InvalidSurface,
    InvalidFrame,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput => f.write_str("invalid output ID"),
            Self::InvalidSurface => f.write_str("invalid surface ID"),
            Self::InvalidFrame => f.write_str("invalid frame snapshot"),
        }
    }
}

impl std::error::Error for EngineError {}

impl SophiaErrorExt for EngineError {
    fn kind(&self) -> SophiaErrorKind {
        match self {
            Self::InvalidOutput => SophiaErrorKind::InvalidOutput,
            Self::InvalidSurface => SophiaErrorKind::InvalidSurface,
            Self::InvalidFrame => SophiaErrorKind::InvalidFrame,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FramePlanRequest {
    pub output: OutputId,
    pub frame_serial: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadlessOutput {
    pub id: OutputId,
    pub size: Size,
    pub scale: u32,
}

impl HeadlessOutput {
    pub const fn deterministic() -> Self {
        Self {
            id: OutputId::from_raw(1),
            size: Size {
                width: 1280,
                height: 720,
            },
            scale: 1,
        }
    }
}

impl Default for HeadlessOutput {
    fn default() -> Self {
        Self::deterministic()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayStep {
    pub command_index: usize,
    pub kind: RenderCommandKind,
    pub source: Option<SurfaceId>,
    pub target: Region,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayReport {
    pub output: OutputId,
    pub output_size: Size,
    pub output_scale: u32,
    pub frame_serial: u64,
    pub steps: Vec<ReplayStep>,
    pub damage: Region,
}

pub trait EngineBackend {
    fn output(&self) -> HeadlessOutput;

    fn plan_frame(
        &self,
        request: FramePlanRequest,
        layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError>;

    fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError>;
}

#[derive(Clone, Debug, Default)]
pub struct HeadlessEngine {
    output: HeadlessOutput,
}

#[derive(Clone, Debug, Default)]
pub struct ChromeBroker {
    descriptors: BTreeMap<SurfaceId, ChromeDescriptor>,
}

impl ChromeBroker {
    pub fn upsert(&mut self, descriptor: ChromeDescriptor) {
        self.descriptors.insert(descriptor.surface, descriptor);
    }

    pub fn get(&self, surface: SurfaceId) -> Option<&ChromeDescriptor> {
        self.descriptors.get(&surface)
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> Option<ChromeDescriptor> {
        self.descriptors.remove(&surface)
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

impl HeadlessEngine {
    pub fn new(output: HeadlessOutput) -> Self {
        Self { output }
    }

    pub fn output(&self) -> HeadlessOutput {
        self.output
    }

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
                return Err(EngineError::InvalidOutput);
            }

            if let Some(source) = command.source {
                if !source.is_valid() || !surfaces.contains(&source) {
                    return Err(EngineError::InvalidSurface);
                }
            }

            steps.push(ReplayStep {
                command_index,
                kind: command.kind,
                source: command.source,
                target: command.target.clone(),
                alpha: command.alpha,
            });
        }

        Ok(ReplayReport {
            output: frame.output,
            output_size: frame.output_size,
            output_scale: frame.output_scale,
            frame_serial: frame.frame_serial,
            steps,
            damage: frame.damage.clone(),
        })
    }

    pub fn apply_layout_transaction(
        &self,
        transaction: &LayoutTransaction,
        mut layers: Vec<LayerSnapshot>,
    ) -> Result<Vec<LayerSnapshot>, EngineError> {
        let layer_indexes = layers
            .iter()
            .enumerate()
            .map(|(index, layer)| (layer.surface, index))
            .collect::<BTreeMap<_, _>>();

        for placement in &transaction.render_positions {
            if !placement.surface.is_valid() {
                return Err(EngineError::InvalidSurface);
            }
            let Some(index) = layer_indexes.get(&placement.surface).copied() else {
                return Err(EngineError::InvalidSurface);
            };
            let layer = &mut layers[index];
            let old_geometry = layer.geometry;

            layer.geometry = placement.geometry;
            layer.stack_rank = u32::try_from(placement.z_index.max(0))
                .expect("non-negative z-index should fit u32");
            layer.crop = placement.crop;
            layer.transform = placement.transform;
            layer.damage = moved_damage(old_geometry, placement.geometry);
            layer.generation = layer.generation.saturating_add(1);
        }

        Ok(layers)
    }

    pub fn commit_layout_transaction(
        &self,
        transaction: &LayoutTransaction,
        layers: &mut Vec<LayerSnapshot>,
    ) -> TransactionCommit {
        let applied_surfaces = transaction
            .render_positions
            .iter()
            .map(|placement| placement.surface)
            .collect::<Vec<_>>();

        match self.apply_layout_transaction(transaction, layers.clone()) {
            Ok(committed) => {
                *layers = committed;
                TransactionCommit {
                    transaction: transaction.transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces,
                }
            }
            Err(EngineError::InvalidSurface) => TransactionCommit {
                transaction: transaction.transaction,
                outcome: TransactionOutcome::RejectedInvalidSurface,
                applied_surfaces: Vec::new(),
            },
            Err(_) => TransactionCommit {
                transaction: transaction.transaction,
                outcome: TransactionOutcome::RejectedStaleSurface,
                applied_surfaces: Vec::new(),
            },
        }
    }

    pub fn preserve_layout_on_wm_absent(
        &self,
        transaction: TransactionId,
        _layers: &[LayerSnapshot],
    ) -> TransactionCommit {
        TransactionCommit {
            transaction,
            outcome: TransactionOutcome::TimedOut,
            applied_surfaces: Vec::new(),
        }
    }

    fn validate_output(&self, output: OutputId) -> Result<(), EngineError> {
        if output.is_valid() && output == self.output.id {
            Ok(())
        } else {
            Err(EngineError::InvalidOutput)
        }
    }
}

impl EngineBackend for HeadlessEngine {
    fn output(&self) -> HeadlessOutput {
        HeadlessEngine::output(self)
    }

    fn plan_frame(
        &self,
        request: FramePlanRequest,
        layers: Vec<LayerSnapshot>,
    ) -> Result<FrameSnapshot, EngineError> {
        HeadlessEngine::plan_frame(self, request, layers)
    }

    fn replay_frame(&self, frame: &FrameSnapshot) -> Result<ReplayReport, EngineError> {
        HeadlessEngine::replay_frame(self, frame)
    }
}

fn should_render(layer: &LayerSnapshot) -> bool {
    layer.opacity > 0.0 && !layer.geometry.is_empty() && layer.source != BufferSource::None
}

fn moved_damage(old_geometry: Rect, new_geometry: Rect) -> Region {
    let mut damage = Region::single(old_geometry);
    damage.extend(&Region::single(new_geometry));
    damage
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
    use sophia_protocol::{
        AttentionState, DisplayLabel, IconTokenId, Rect, SurfaceId, SurfacePlacement,
        TransactionId, Transform, TrustLevel,
    };

    #[test]
    fn headless_engine_exposes_deterministic_output() {
        let engine = HeadlessEngine::default();
        let output = engine.output();

        assert_eq!(output.id, OutputId::from_raw(1));
        assert_eq!(
            output.size,
            Size {
                width: 1280,
                height: 720,
            }
        );
        assert_eq!(output.scale, 1);
    }

    #[test]
    fn headless_engine_returns_frame_value() {
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let request = FramePlanRequest {
            output: output.id,
            frame_serial: 7,
        };
        let frame = engine.plan_frame(request, Vec::new()).unwrap();

        assert_eq!(frame.output, request.output);
        assert_eq!(frame.output_size, output.size);
        assert_eq!(frame.output_scale, output.scale);
        assert_eq!(frame.frame_serial, 7);
        assert!(frame.layers.is_empty());
        assert!(frame.commands.is_empty());
    }

    #[test]
    fn frame_plan_sorts_layers_by_stack_rank() {
        let engine = HeadlessEngine::default();
        let request = FramePlanRequest {
            output: engine.output().id,
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
        let engine = HeadlessEngine::default();
        let request = FramePlanRequest {
            output: engine.output().id,
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
        let engine = HeadlessEngine::default();
        let request = FramePlanRequest {
            output: engine.output().id,
            frame_serial: 1,
        };
        let mut layer = test_layer(0, 0, 0, Region::empty());
        layer.surface = SurfaceId::INVALID;

        assert_eq!(
            engine.plan_frame(request, vec![layer]),
            Err(EngineError::InvalidSurface)
        );
    }

    #[test]
    fn frame_snapshot_replays_with_mock_surfaces() {
        let engine = HeadlessEngine::default();
        let request = FramePlanRequest {
            output: engine.output().id,
            frame_serial: 11,
        };
        let frame = engine
            .plan_frame(
                request,
                vec![
                    test_layer(0, 0, 0, Region::empty()),
                    test_layer(1, 1, 100, Region::empty()),
                ],
            )
            .unwrap();

        let replay = engine.replay_frame(&frame).unwrap();

        assert_eq!(replay.output, engine.output().id);
        assert_eq!(replay.output_size, engine.output().size);
        assert_eq!(replay.output_scale, engine.output().scale);
        assert_eq!(replay.frame_serial, 11);
        assert_eq!(replay.steps.len(), 2);
        assert_eq!(replay.steps[0].source, Some(frame.layers[0].surface));
    }

    #[test]
    fn layout_transaction_moves_and_resizes_layers_atomically() {
        let engine = HeadlessEngine::default();
        let surface = SurfaceId::new(0, 1);
        let layers = vec![test_layer(0, 0, 0, Region::empty())];
        let transaction = LayoutTransaction {
            transaction: TransactionId::from_raw(1),
            requested_sizes: Vec::new(),
            focus: Some(surface),
            render_positions: vec![SurfacePlacement {
                surface,
                geometry: Rect {
                    x: 25,
                    y: 30,
                    width: 400,
                    height: 300,
                },
                z_index: 7,
                crop: None,
                transform: Transform::IDENTITY,
            }],
            timeout_msec: 300,
        };

        let committed = engine
            .apply_layout_transaction(&transaction, layers)
            .unwrap();

        assert_eq!(committed[0].geometry.x, 25);
        assert_eq!(committed[0].geometry.width, 400);
        assert_eq!(committed[0].stack_rank, 7);
        assert_eq!(committed[0].generation, 2);
        assert_eq!(committed[0].damage.rects.len(), 2);
    }

    #[test]
    fn layout_transaction_rejects_unknown_surfaces() {
        let engine = HeadlessEngine::default();
        let transaction = LayoutTransaction {
            transaction: TransactionId::from_raw(1),
            requested_sizes: Vec::new(),
            focus: None,
            render_positions: vec![SurfacePlacement {
                surface: SurfaceId::new(99, 1),
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                },
                z_index: 0,
                crop: None,
                transform: Transform::IDENTITY,
            }],
            timeout_msec: 300,
        };

        assert_eq!(
            engine
                .apply_layout_transaction(&transaction, vec![test_layer(0, 0, 0, Region::empty())]),
            Err(EngineError::InvalidSurface)
        );
    }

    #[test]
    fn commit_layout_transaction_reports_outcome() {
        let engine = HeadlessEngine::default();
        let surface = SurfaceId::new(0, 1);
        let mut layers = vec![test_layer(0, 0, 0, Region::empty())];
        let transaction = LayoutTransaction {
            transaction: TransactionId::from_raw(44),
            requested_sizes: Vec::new(),
            focus: Some(surface),
            render_positions: vec![SurfacePlacement {
                surface,
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: 500,
                    height: 400,
                },
                z_index: 1,
                crop: Some(Rect {
                    x: 0,
                    y: 0,
                    width: 250,
                    height: 200,
                }),
                transform: Transform::IDENTITY,
            }],
            timeout_msec: 300,
        };

        let commit = engine.commit_layout_transaction(&transaction, &mut layers);

        assert_eq!(commit.transaction, TransactionId::from_raw(44));
        assert_eq!(commit.outcome, TransactionOutcome::Committed);
        assert_eq!(commit.applied_surfaces, vec![surface]);
        assert_eq!(
            layers[0].crop,
            Some(Rect {
                x: 0,
                y: 0,
                width: 250,
                height: 200,
            })
        );
    }

    #[test]
    fn absent_wm_preserves_committed_layers() {
        let engine = HeadlessEngine::default();
        let layers = vec![test_layer(0, 0, 0, Region::empty())];
        let before = layers.clone();

        let commit = engine.preserve_layout_on_wm_absent(TransactionId::from_raw(45), &layers);

        assert_eq!(commit.transaction, TransactionId::from_raw(45));
        assert_eq!(commit.outcome, TransactionOutcome::TimedOut);
        assert!(commit.applied_surfaces.is_empty());
        assert_eq!(layers, before);
    }

    #[test]
    fn frame_snapshot_replay_rejects_unknown_surface() {
        let engine = HeadlessEngine::default();
        let request = FramePlanRequest {
            output: engine.output().id,
            frame_serial: 12,
        };
        let mut frame = engine
            .plan_frame(request, vec![test_layer(0, 0, 0, Region::empty())])
            .unwrap();
        frame.commands[0].source = Some(SurfaceId::new(99, 1));

        assert_eq!(
            engine.replay_frame(&frame),
            Err(EngineError::InvalidSurface)
        );
    }

    #[test]
    fn chrome_broker_keeps_metadata_separate_from_layout() {
        let mut broker = ChromeBroker::default();
        let surface = SurfaceId::new(3, 1);

        broker.upsert(ChromeDescriptor {
            surface,
            label: Some(DisplayLabel {
                text: "Redacted Title".to_owned(),
                redacted: true,
            }),
            icon: Some(IconTokenId::from_raw(12)),
            trust_level: TrustLevel::Isolated,
            attention: AttentionState::None,
            generation: 4,
        });

        let descriptor = broker.get(surface).unwrap();

        assert_eq!(broker.len(), 1);
        assert_eq!(
            descriptor.label.as_ref().map(|label| label.redacted),
            Some(true)
        );
        assert_eq!(descriptor.icon, Some(IconTokenId::from_raw(12)));
        assert_eq!(descriptor.trust_level, TrustLevel::Isolated);
    }

    #[test]
    fn chrome_broker_removes_surface_metadata() {
        let mut broker = ChromeBroker::default();
        let surface = SurfaceId::new(4, 1);

        broker.upsert(ChromeDescriptor {
            surface,
            label: None,
            icon: None,
            trust_level: TrustLevel::Unknown,
            attention: AttentionState::None,
            generation: 1,
        });

        assert!(broker.remove_surface(surface).is_some());
        assert!(broker.get(surface).is_none());
        assert!(broker.is_empty());
    }
}

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::{
    BufferSource, ChromeActionKind, ChromeActionRequest, ChromeDescriptor, FrameSnapshot,
    LayerSnapshot, LayoutNodeSnapshot, LayoutTransaction, OutputId, Rect, Region, RenderCommand,
    RenderCommandKind, Size, SurfaceId, TransactionCommit, TransactionId, TransactionOutcome,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromeActionDecision {
    RequestPoliteClose { surface: SurfaceId },
    Rejected(ChromeActionRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeActionRejectReason {
    UnknownSurface,
    StaleGeneration,
    NotClosable,
    UnsupportedAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    ChromeAction(ChromeActionRequest),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionUpdate {
    pub chrome_decision: Option<ChromeActionDecision>,
    pub commands: Vec<SessionCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionCommand {
    RequestPoliteClose { surface: SurfaceId },
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

    pub fn validate_chrome_action(
        &self,
        request: &ChromeActionRequest,
        nodes: &[LayoutNodeSnapshot],
    ) -> ChromeActionDecision {
        validate_chrome_action(request, nodes)
    }

    pub fn handle_session_event(
        &self,
        event: SessionEvent,
        nodes: &[LayoutNodeSnapshot],
    ) -> SessionUpdate {
        handle_session_event(event, nodes)
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

pub fn validate_chrome_action(
    request: &ChromeActionRequest,
    nodes: &[LayoutNodeSnapshot],
) -> ChromeActionDecision {
    let Some(node) = nodes.iter().find(|node| node.surface == request.surface) else {
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::UnknownSurface);
    };

    if node.generation != request.generation {
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::StaleGeneration);
    }

    match request.kind {
        ChromeActionKind::CloseSurfaceRequested => {
            if node.capabilities.closable {
                ChromeActionDecision::RequestPoliteClose {
                    surface: request.surface,
                }
            } else {
                ChromeActionDecision::Rejected(ChromeActionRejectReason::NotClosable)
            }
        }
    }
}

pub fn handle_session_event(event: SessionEvent, nodes: &[LayoutNodeSnapshot]) -> SessionUpdate {
    match event {
        SessionEvent::ChromeAction(request) => {
            let decision = validate_chrome_action(&request, nodes);
            let commands = match decision {
                ChromeActionDecision::RequestPoliteClose { surface } => {
                    vec![SessionCommand::RequestPoliteClose { surface }]
                }
                ChromeActionDecision::Rejected(_) => Vec::new(),
            };

            SessionUpdate {
                chrome_decision: Some(decision),
                commands,
            }
        }
    }
}

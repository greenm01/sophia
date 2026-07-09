mod core;
mod layout;
mod rendering;
mod session_tick;
mod wm_transaction;

use crate::{EngineError, FramePlanRequest, HeadlessOutput, ReplayReport};
use sophia_protocol::{FrameSnapshot, LayerSnapshot};

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
    pub(crate) output: HeadlessOutput,
}

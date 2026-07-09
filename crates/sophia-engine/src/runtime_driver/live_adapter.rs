use crate::prelude::*;
use crate::{EngineError, HeadlessEngine, LastCommittedLayout, SessionTickReport};

use super::adapter::RuntimeDriverAdapter;

mod broker;
mod chrome;
mod intake;
mod portal;
mod renderer;
mod wm;
mod x;

pub use broker::*;
pub use chrome::*;
pub use intake::*;
pub use portal::*;
pub use renderer::*;
pub use wm::*;
pub use x::*;

impl RuntimeDriverAdapter for LiveRuntimeDriverAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.x.poll_observation())
    }

    fn request_wm_layout(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.wm.layout_observation())
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        self.renderer
            .render_frame(engine, output, frame_serial, last_committed)
    }

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.portal.drain_observation())
    }

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.chrome.present_observation())
    }
}

use crate::prelude::*;
use crate::{EngineError, HeadlessEngine, LastCommittedLayout, SessionTickReport};

use super::adapter::RuntimeDriverAdapter;

mod broker;
mod chrome;
mod intake;
mod portal;
mod renderer;
mod scanout;
mod wm;
mod x;

pub use broker::*;
pub use chrome::*;
pub use intake::*;
pub use portal::*;
pub use renderer::*;
pub use scanout::*;
pub use wm::*;
pub use x::*;

impl RuntimeDriverAdapter for LiveRuntimeDriverAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.x.poll_observation())
    }

    fn poll_x_observations(&mut self) -> Result<Vec<SessionRuntimeObservation>, EngineError> {
        Ok(self.x.poll_observations())
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

    fn submit_scanout(
        &mut self,
        frame_serial: u64,
    ) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.scanout.submit_observation(frame_serial))
    }

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.portal.drain_observation())
    }

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self.chrome.present_observation())
    }
}

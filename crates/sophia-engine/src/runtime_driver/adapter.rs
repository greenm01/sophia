use crate::prelude::*;
use crate::{
    EngineError, HeadlessEngine, LastCommittedLayout, SessionLayerSource, SessionTickReport,
    SessionTickRequest, WmTransactionUpdate,
};

use super::observation::{
    runtime_observation_from_portal_commands, runtime_observation_from_wm_transaction_update,
};
use super::types::HeadlessSessionDriverTick;

pub trait RuntimeDriverAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError>;

    fn request_wm_layout(&mut self) -> Result<SessionRuntimeObservation, EngineError>;

    fn schedule_frame(
        &mut self,
        frame_serial: u64,
    ) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(SessionRuntimeObservation::FrameScheduled { frame_serial })
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError>;

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError>;

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessRuntimeAdapter {
    pub x_event_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
}

impl HeadlessRuntimeAdapter {
    pub fn new(tick: HeadlessSessionDriverTick) -> Self {
        Self {
            x_event_count: tick.x_event_count,
            layers: tick.layers,
            wm_update: tick.wm_update,
            portal_commands: tick.portal_commands,
            chrome_command_count: tick.chrome_command_count,
        }
    }
}

impl RuntimeDriverAdapter for HeadlessRuntimeAdapter {
    fn poll_x_events(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(SessionRuntimeObservation::XEventsPolled {
            count: self.x_event_count,
        })
    }

    fn request_wm_layout(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(self
            .wm_update
            .as_ref()
            .map(runtime_observation_from_wm_transaction_update)
            .unwrap_or(SessionRuntimeObservation::WmLayoutReady))
    }

    fn render_frame(
        &mut self,
        engine: &HeadlessEngine,
        output: OutputId,
        frame_serial: u64,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        engine.run_session_tick(
            SessionTickRequest {
                output,
                frame_serial,
                layers: SessionLayerSource::Fresh(self.layers.clone()),
            },
            last_committed,
        )
    }

    fn drain_portal_commands(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(runtime_observation_from_portal_commands(
            &self.portal_commands,
        ))
    }

    fn present_chrome(&mut self) -> Result<SessionRuntimeObservation, EngineError> {
        Ok(SessionRuntimeObservation::ChromeCommandsReady {
            count: self.chrome_command_count,
        })
    }
}

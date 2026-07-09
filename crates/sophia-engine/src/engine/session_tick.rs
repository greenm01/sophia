use crate::prelude::*;
use crate::{
    ChromeActionDecision, EngineError, FrameClock, FramePlanRequest, HeadlessEngine,
    LastCommittedLayout, SessionEvent, SessionLayerSource, SessionTickReport, SessionTickRequest,
    SessionUpdate, handle_session_event, validate_chrome_action,
};

impl HeadlessEngine {
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

    pub fn run_session_tick(
        &self,
        request: SessionTickRequest,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        let (layers, restored_last_committed) = match request.layers {
            SessionLayerSource::Fresh(layers) => {
                debug!(
                    output = request.output.raw(),
                    frame_serial = request.frame_serial,
                    layer_count = layers.len(),
                    "running session tick from fresh layers"
                );
                last_committed.replace(&layers);
                (layers, false)
            }
            SessionLayerSource::RestoreLastCommitted => {
                let mut layers = Vec::new();
                last_committed.restore_into(&mut layers);
                warn!(
                    output = request.output.raw(),
                    frame_serial = request.frame_serial,
                    restored_layers = layers.len(),
                    "running session tick from last committed layout"
                );
                (layers, true)
            }
        };
        let frame = self.plan_frame(
            FramePlanRequest {
                output: request.output,
                frame_serial: request.frame_serial,
            },
            layers,
        )?;
        let replay = self.replay_frame(&frame)?;
        debug!(
            output = request.output.raw(),
            frame_serial = request.frame_serial,
            restored_last_committed,
            render_commands = frame.commands.len(),
            replay_steps = replay.steps.len(),
            "completed session tick"
        );

        Ok(SessionTickReport {
            frame,
            replay,
            restored_last_committed,
        })
    }

    pub fn run_clocked_session_tick(
        &self,
        clock: &mut impl FrameClock,
        layers: SessionLayerSource,
        last_committed: &mut LastCommittedLayout,
    ) -> Result<SessionTickReport, EngineError> {
        let tick = clock.next_frame(self.output.id);
        trace!(
            output = tick.output.raw(),
            frame_serial = tick.frame_serial,
            target_msec = tick.target_msec,
            "frame clock produced session tick"
        );

        self.run_session_tick(
            SessionTickRequest {
                output: tick.output,
                frame_serial: tick.frame_serial,
                layers,
            },
            last_committed,
        )
    }
}

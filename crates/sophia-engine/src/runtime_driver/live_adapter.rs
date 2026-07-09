use crate::prelude::*;
use crate::{
    EngineError, HeadlessEngine, LastCommittedLayout, MetadataChromeUpdate,
    NotificationChromeUpdate, RenderFrameReport, SessionLayerSource, SessionTickReport,
    SessionTickRequest, WmTransactionUpdate,
};

use super::adapter::RuntimeDriverAdapter;
use super::observation::{
    runtime_observation_from_metadata_chrome_updates,
    runtime_observation_from_notification_chrome_updates, runtime_observation_from_portal_commands,
    runtime_observation_from_render_frame_report, runtime_observation_from_session_tick_report,
    runtime_observation_from_wm_transaction_update,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveXRuntimeAdapter {
    pub pending_event_count: u32,
}

impl LiveXRuntimeAdapter {
    pub fn from_polled_event_count(count: u32) -> Self {
        Self {
            pending_event_count: count,
        }
    }

    pub fn poll_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::XEventsPolled {
            count: self.pending_event_count,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveWmRuntimeAdapter {
    pub update: Option<WmTransactionUpdate>,
}

impl LiveWmRuntimeAdapter {
    pub fn from_transaction_update(update: WmTransactionUpdate) -> Self {
        Self {
            update: Some(update),
        }
    }

    pub fn layout_observation(&self) -> SessionRuntimeObservation {
        self.update
            .as_ref()
            .map(runtime_observation_from_wm_transaction_update)
            .unwrap_or(SessionRuntimeObservation::WmLayoutReady)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveBrokerRuntimeAdapter;

impl LiveBrokerRuntimeAdapter {
    pub fn from_health_packet(packet: &BrokerHealthPacket) -> SessionRuntimeObservation {
        Self::health_observation(packet)
    }

    pub fn health_observation(packet: &BrokerHealthPacket) -> SessionRuntimeObservation {
        SessionRuntimeObservation::BrokerHealthChanged {
            broker: packet.broker,
            state: packet.state,
            generation: packet.generation,
            status_message_len: packet.message.as_deref().map(str::len).unwrap_or(0),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LivePortalRuntimeAdapter {
    pub commands: Vec<PortalCommand>,
}

impl LivePortalRuntimeAdapter {
    pub fn from_commands(commands: Vec<PortalCommand>) -> Self {
        Self { commands }
    }

    pub fn drain_observation(&self) -> SessionRuntimeObservation {
        runtime_observation_from_portal_commands(&self.commands)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveChromeRuntimeAdapter {
    pub command_count: u32,
}

impl LiveChromeRuntimeAdapter {
    pub fn from_command_count(count: u32) -> Self {
        Self {
            command_count: count,
        }
    }

    pub fn from_notification_updates<'a>(
        updates: impl IntoIterator<Item = &'a NotificationChromeUpdate>,
    ) -> Self {
        let SessionRuntimeObservation::ChromeCommandsReady { count } =
            runtime_observation_from_notification_chrome_updates(updates)
        else {
            unreachable!("notification chrome updates always map to chrome command counts");
        };

        Self::from_command_count(count)
    }

    pub fn from_metadata_updates<'a>(
        updates: impl IntoIterator<Item = &'a MetadataChromeUpdate>,
    ) -> Self {
        let SessionRuntimeObservation::ChromeCommandsReady { count } =
            runtime_observation_from_metadata_chrome_updates(updates)
        else {
            unreachable!("metadata chrome updates always map to chrome command counts");
        };

        Self::from_command_count(count)
    }

    pub fn present_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::ChromeCommandsReady {
            count: self.command_count,
        }
    }
}

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverIntake {
    pub x_event_count: u32,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverAdapter {
    pub x: LiveXRuntimeAdapter,
    pub wm: LiveWmRuntimeAdapter,
    pub portal: LivePortalRuntimeAdapter,
    pub chrome: LiveChromeRuntimeAdapter,
    pub renderer: LiveRendererRuntimeAdapter,
}

impl LiveRuntimeDriverAdapter {
    pub fn from_intake(intake: LiveRuntimeDriverIntake) -> Self {
        Self {
            x: LiveXRuntimeAdapter::from_polled_event_count(intake.x_event_count),
            wm: LiveWmRuntimeAdapter {
                update: intake.wm_update,
            },
            portal: LivePortalRuntimeAdapter::from_commands(intake.portal_commands),
            chrome: LiveChromeRuntimeAdapter::from_command_count(intake.chrome_command_count),
            renderer: LiveRendererRuntimeAdapter::from_committed_surface_states(
                intake.committed_surfaces,
                intake.layers,
            ),
        }
    }
}

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

use crate::prelude::*;
use crate::{EngineError, HeadlessEngine, LastCommittedLayout, SessionTickReport};

use super::adapter::{HeadlessRuntimeAdapter, RuntimeDriverAdapter};
use super::observation::runtime_observation_from_session_tick_report;
use super::types::{HeadlessSessionDriverReport, HeadlessSessionDriverTick};

#[derive(Clone, Debug, Default)]
pub struct HeadlessSessionDriver {
    engine: HeadlessEngine,
    runtime: SessionRuntimeLoop,
    last_committed: LastCommittedLayout,
}

impl HeadlessSessionDriver {
    pub fn new(engine: HeadlessEngine) -> Self {
        Self {
            engine,
            runtime: SessionRuntimeLoop::default(),
            last_committed: LastCommittedLayout::default(),
        }
    }

    pub fn runtime_state(&self) -> &SessionRuntimeState {
        self.runtime.state()
    }

    pub fn last_committed(&self) -> &LastCommittedLayout {
        &self.last_committed
    }

    pub fn run_tick(
        &mut self,
        request: HeadlessSessionDriverTick,
    ) -> Result<HeadlessSessionDriverReport, EngineError> {
        let output = request.output;
        let frame_serial = request.frame_serial;
        let mut adapter = HeadlessRuntimeAdapter::new(request);
        self.run_with_adapter(output, frame_serial, &mut adapter)
    }

    pub fn run_with_adapter(
        &mut self,
        output: OutputId,
        frame_serial: u64,
        adapter: &mut impl RuntimeDriverAdapter,
    ) -> Result<HeadlessSessionDriverReport, EngineError> {
        let mut executor = HeadlessSessionCommandExecutor {
            engine: &self.engine,
            runtime: &mut self.runtime,
            last_committed: &mut self.last_committed,
            output,
            frame_serial,
            adapter,
            runtime_commands: Vec::new(),
            pending_commands: Vec::new(),
            session_tick: None,
        };
        executor.run()
    }
}

struct HeadlessSessionCommandExecutor<'a> {
    engine: &'a HeadlessEngine,
    runtime: &'a mut SessionRuntimeLoop,
    last_committed: &'a mut LastCommittedLayout,
    output: OutputId,
    frame_serial: u64,
    adapter: &'a mut dyn RuntimeDriverAdapter,
    runtime_commands: Vec<SessionRuntimeCommand>,
    pending_commands: Vec<SessionRuntimeCommand>,
    session_tick: Option<SessionTickReport>,
}

impl HeadlessSessionCommandExecutor<'_> {
    fn run(&mut self) -> Result<HeadlessSessionDriverReport, EngineError> {
        self.observe([SessionRuntimeObservation::TickStarted])?;

        while let Some(command) = self.pending_commands.pop() {
            match command {
                SessionRuntimeCommand::None => {}
                SessionRuntimeCommand::PollXEvents => {
                    let observation = self.adapter.poll_x_events()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::RequestWmLayout => {
                    let observation = self.adapter.request_wm_layout()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::ScheduleFrame => {
                    let observation = self.adapter.schedule_frame(self.frame_serial)?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::RenderFrame { frame_serial } => {
                    let report = self.adapter.render_frame(
                        self.engine,
                        self.output,
                        frame_serial,
                        self.last_committed,
                    )?;
                    let observation = runtime_observation_from_session_tick_report(&report);
                    self.session_tick = Some(report);
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::DrainPortalCommands => {
                    let observation = self.adapter.drain_portal_commands()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::PresentChrome => {
                    let observation = self.adapter.present_chrome()?;
                    self.observe([observation])?;
                }
                SessionRuntimeCommand::RestartWindowManager => break,
            }
        }

        Ok(HeadlessSessionDriverReport {
            runtime_state: self.runtime.state().clone(),
            runtime_commands: self.runtime_commands.clone(),
            session_tick: self.session_tick.clone(),
            cached_layers: self.last_committed.layers().len(),
        })
    }

    fn observe(
        &mut self,
        observations: impl IntoIterator<Item = SessionRuntimeObservation>,
    ) -> Result<(), EngineError> {
        let report = self
            .runtime
            .step_observations(observations)
            .map_err(EngineError::RuntimeObservation)?;
        self.runtime_commands
            .extend(report.commands.iter().copied());
        self.pending_commands
            .extend(report.commands.into_iter().rev());
        Ok(())
    }
}

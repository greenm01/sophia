use crate::prelude::*;
use crate::{SessionTickReport, WmTransactionUpdate};

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessSessionDriverTick {
    pub output: OutputId,
    pub frame_serial: u64,
    pub x_event_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessSessionDriverReport {
    pub runtime_state: SessionRuntimeState,
    pub runtime_commands: Vec<SessionRuntimeCommand>,
    pub session_tick: Option<SessionTickReport>,
    pub cached_layers: usize,
}

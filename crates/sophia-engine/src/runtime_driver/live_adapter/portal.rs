use crate::prelude::*;

use super::super::observation::runtime_observation_from_portal_commands;

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

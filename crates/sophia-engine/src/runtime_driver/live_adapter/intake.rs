use crate::WmTransactionUpdate;
use crate::prelude::*;

use super::{
    LiveChromeRuntimeAdapter, LivePortalRuntimeAdapter, LiveRendererRuntimeAdapter,
    LiveWmRuntimeAdapter, LiveXRuntimeAdapter,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverIntake {
    pub x_event_count: u32,
    pub authority_commits: Vec<TransactionCommit>,
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
        .with_authority_commits(intake.authority_commits)
    }

    fn with_authority_commits(mut self, authority_commits: Vec<TransactionCommit>) -> Self {
        self.x.authority_commits = authority_commits;
        self
    }
}

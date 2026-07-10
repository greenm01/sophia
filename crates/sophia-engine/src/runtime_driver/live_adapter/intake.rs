use crate::prelude::*;
use crate::{HeadlessEngine, WmTransactionUpdate};

use super::{
    LiveChromeRuntimeAdapter, LivePortalRuntimeAdapter, LiveRendererRuntimeAdapter,
    LiveScanoutRuntimeAdapter, LiveWmRuntimeAdapter, LiveXRuntimeAdapter,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityTransactionIntake {
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
}

impl AuthorityTransactionIntake {
    pub fn new(transaction: TransactionId, transactions: Vec<SurfaceTransaction>) -> Self {
        Self {
            transaction,
            transactions,
        }
    }

    pub fn commit(
        &self,
        engine: &HeadlessEngine,
        committed: &mut Vec<CommittedSurfaceState>,
    ) -> TransactionCommit {
        engine.commit_surface_transactions(self.transaction, &self.transactions, committed)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverIntake {
    pub x_event_count: u32,
    pub authority_commits: Vec<TransactionCommit>,
    pub authority_batches: Vec<AuthorityTransactionIntake>,
    pub wm_update: Option<WmTransactionUpdate>,
    pub portal_commands: Vec<PortalCommand>,
    pub chrome_command_count: u32,
    pub layers: Vec<LayerSnapshot>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
    pub scanout_submit_state: Option<RuntimeScanoutState>,
    pub scanout_lifecycle_states: Vec<RuntimeScanoutState>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveRuntimeDriverAdapter {
    pub x: LiveXRuntimeAdapter,
    pub wm: LiveWmRuntimeAdapter,
    pub portal: LivePortalRuntimeAdapter,
    pub chrome: LiveChromeRuntimeAdapter,
    pub renderer: LiveRendererRuntimeAdapter,
    pub scanout: LiveScanoutRuntimeAdapter,
}

impl LiveRuntimeDriverAdapter {
    pub fn from_authority_batches(
        engine: &HeadlessEngine,
        mut intake: LiveRuntimeDriverIntake,
    ) -> Self {
        let mut committed_surfaces = intake.committed_surfaces.clone();
        intake.authority_commits.extend(
            intake
                .authority_batches
                .iter()
                .map(|batch| batch.commit(engine, &mut committed_surfaces)),
        );
        intake.committed_surfaces = committed_surfaces;
        Self::from_intake(intake)
    }

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
            scanout: LiveScanoutRuntimeAdapter::from_states(
                intake
                    .scanout_submit_state
                    .unwrap_or(RuntimeScanoutState::Submitted),
                intake.scanout_lifecycle_states,
            ),
        }
        .with_authority_commits(intake.authority_commits)
    }

    fn with_authority_commits(mut self, authority_commits: Vec<TransactionCommit>) -> Self {
        self.x.authority_commits = authority_commits;
        self
    }
}

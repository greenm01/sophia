use crate::prelude::*;
use crate::{
    MetadataChromeUpdate, NotificationChromeUpdate, RenderFrameReport, SessionTickReport,
    SlowClientVisualDecision, WmRuntimeAction, WmTransactionUpdate,
};

pub fn runtime_observation_from_wm_transaction_update(
    update: &WmTransactionUpdate,
) -> SessionRuntimeObservation {
    match update.runtime_action() {
        WmRuntimeAction::KeepRunning => SessionRuntimeObservation::WmLayoutReady,
        WmRuntimeAction::RestartWm { .. } => SessionRuntimeObservation::WmRestartRequested,
    }
}

pub fn runtime_observation_from_authority_transaction_commit(
    commit: &TransactionCommit,
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::AuthorityTransactionObserved {
        outcome: commit.outcome,
        applied_surface_count: u32::try_from(commit.applied_surfaces.len()).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_slow_client_visual_decisions(
    decisions: &[SlowClientVisualDecision],
) -> SessionRuntimeObservation {
    let preserved_count = decisions
        .iter()
        .filter(|decision| matches!(decision, SlowClientVisualDecision::PreserveCommitted { .. }))
        .count();
    let degraded_count = decisions
        .iter()
        .filter(|decision| matches!(decision, SlowClientVisualDecision::DegradeToPending { .. }))
        .count();
    let timeout_count = preserved_count.saturating_add(degraded_count);

    SessionRuntimeObservation::SlowClientVisualDecisionsObserved {
        timeout_count: u32::try_from(timeout_count).unwrap_or(u32::MAX),
        preserved_count: u32::try_from(preserved_count).unwrap_or(u32::MAX),
        degraded_count: u32::try_from(degraded_count).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_session_tick_report(
    report: &SessionTickReport,
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::FrameRendered {
        frame_serial: report.frame.frame_serial,
    }
}

pub fn runtime_observation_from_render_frame_report(
    report: &RenderFrameReport,
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::FrameRendered {
        frame_serial: report.replay.frame_serial,
    }
}

pub fn runtime_observation_from_portal_commands(
    commands: &[PortalCommand],
) -> SessionRuntimeObservation {
    SessionRuntimeObservation::PortalCommandsReady {
        count: u32::try_from(commands.len()).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_notification_chrome_updates<'a>(
    updates: impl IntoIterator<Item = &'a NotificationChromeUpdate>,
) -> SessionRuntimeObservation {
    let count = updates
        .into_iter()
        .filter(|update| {
            matches!(
                update,
                NotificationChromeUpdate::Presented { .. }
                    | NotificationChromeUpdate::Dismissed { .. }
            )
        })
        .count();

    SessionRuntimeObservation::ChromeCommandsReady {
        count: u32::try_from(count).unwrap_or(u32::MAX),
    }
}

pub fn runtime_observation_from_metadata_chrome_updates<'a>(
    updates: impl IntoIterator<Item = &'a MetadataChromeUpdate>,
) -> SessionRuntimeObservation {
    let count = updates
        .into_iter()
        .filter(|update| {
            matches!(
                update,
                MetadataChromeUpdate::Upserted { .. } | MetadataChromeUpdate::Removed { .. }
            )
        })
        .count();

    SessionRuntimeObservation::ChromeCommandsReady {
        count: u32::try_from(count).unwrap_or(u32::MAX),
    }
}

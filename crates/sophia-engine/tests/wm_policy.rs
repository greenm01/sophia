use sophia_engine::{
    WmPolicyError, WmShortcutDecision, WmShortcutRegistry, WmShortcutRouter, WmWorkspaceState,
};
use sophia_protocol::{
    OutputId, Rect, SeatId, SurfaceId, TransactionId, WM_API_VERSION, WmActionId,
    WmBindingRegistration, WmCapabilities, WmCommand, WmHello, WmModifierMask, WmResponsePacket,
    WmSessionAction, WorkspaceId,
};

fn bounds(x: i32) -> Rect {
    Rect {
        x,
        y: 0,
        width: 1280,
        height: 720,
    }
}

#[test]
fn physical_shortcut_router_tracks_super_per_seat_and_suppresses_repeats() {
    let action = WmActionId::from_raw(7);
    let registry = WmShortcutRegistry::from_hello(&WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        bindings: vec![WmBindingRegistration {
            action,
            keycode: 36,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
    })
    .unwrap();
    let mut router = WmShortcutRouter::new(registry);
    let seat = SeatId::from_raw(1);

    assert!(!router.route_key(seat, 125, true).consumed);
    assert_eq!(
        router.route_key(seat, 36, true),
        WmShortcutDecision {
            action: Some(action),
            consumed: true,
        }
    );
    assert_eq!(
        router.route_key(seat, 36, true),
        WmShortcutDecision {
            action: None,
            consumed: true,
        }
    );
    assert!(!router.route_key(seat, 125, false).consumed);
    assert_eq!(
        router.route_key(seat, 36, false),
        WmShortcutDecision {
            action: None,
            consumed: true,
        }
    );
}

#[test]
fn workspace_activation_swaps_visible_workspaces_without_mutating_source() {
    let output_one = OutputId::from_raw(1);
    let output_two = OutputId::from_raw(2);
    let state =
        WmWorkspaceState::new([(output_one, bounds(0)), (output_two, bounds(1280))], 9).unwrap();
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(9),
        commands: vec![WmCommand::ActivateWorkspace {
            output: output_one,
            workspace: WorkspaceId::from_raw(2),
        }],
        timeout_msec: 300,
    };

    let plan = state.plan_response(&response, &[]).unwrap();

    assert_eq!(
        state.output(output_one).unwrap().workspace,
        WorkspaceId::from_raw(1)
    );
    assert_eq!(
        plan.candidate.output(output_one).unwrap().workspace,
        WorkspaceId::from_raw(2)
    );
    assert_eq!(
        plan.candidate.output(output_two).unwrap().workspace,
        WorkspaceId::from_raw(1)
    );
    assert_eq!(plan.affected_outputs, vec![output_one, output_two]);
}

#[test]
fn workspace_plan_moves_focus_and_validates_named_actions_atomically() {
    let output = OutputId::from_raw(1);
    let surface = SurfaceId::new(4, 1);
    let mut state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    state
        .register_surface(surface, WorkspaceId::from_raw(1))
        .unwrap();
    let response = WmResponsePacket {
        transaction: TransactionId::from_raw(10),
        commands: vec![
            WmCommand::AssignWorkspace {
                surface,
                workspace: WorkspaceId::from_raw(2),
            },
            WmCommand::RequestSessionAction {
                action: WmSessionAction::LaunchTerminal,
                target: None,
            },
        ],
        timeout_msec: 300,
    };

    let plan = state
        .plan_response(&response, &[WmSessionAction::LaunchTerminal])
        .unwrap();

    assert_eq!(
        plan.candidate.surface_workspace(surface),
        Some(WorkspaceId::from_raw(2))
    );
    assert_eq!(
        plan.session_action,
        Some((WmSessionAction::LaunchTerminal, None))
    );
    assert_eq!(
        state.surface_workspace(surface),
        Some(WorkspaceId::from_raw(1))
    );
}

#[test]
fn workspace_plan_rejects_unadvertised_or_duplicate_side_effects() {
    let output = OutputId::from_raw(1);
    let state = WmWorkspaceState::new([(output, bounds(0))], 9).unwrap();
    let unadvertised = WmResponsePacket {
        transaction: TransactionId::from_raw(11),
        commands: vec![WmCommand::RequestSessionAction {
            action: WmSessionAction::LaunchFirefox,
            target: None,
        }],
        timeout_msec: 300,
    };
    assert_eq!(
        state.plan_response(&unadvertised, &[]),
        Err(WmPolicyError::UnadvertisedSessionAction)
    );

    let duplicate = WmResponsePacket {
        transaction: TransactionId::from_raw(12),
        commands: vec![
            WmCommand::ActivateWorkspace {
                output,
                workspace: WorkspaceId::from_raw(2),
            },
            WmCommand::ActivateWorkspace {
                output,
                workspace: WorkspaceId::from_raw(3),
            },
        ],
        timeout_msec: 300,
    };
    assert_eq!(
        state.plan_response(&duplicate, &[]),
        Err(WmPolicyError::DuplicateOutputCommand)
    );
}

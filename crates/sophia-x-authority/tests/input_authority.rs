use sophia_protocol::NamespaceId;
use sophia_x_authority::{
    X_ANY_MODIFIER, XActiveInputGrab, XInputAuthorityState, XInputGrabError, XPassiveInputGrab,
    XResourceId,
};

fn active(owner: u64) -> XActiveInputGrab {
    XActiveInputGrab {
        owner,
        window: XResourceId::new(0x100, 1),
        owner_events: false,
        pointer_mode: 1,
        keyboard_mode: 1,
        event_mask: 0x44,
    }
}

fn passive(owner: u64, detail: u8, modifiers: u16) -> XPassiveInputGrab {
    XPassiveInputGrab {
        owner,
        window: XResourceId::new(0x100, 1),
        detail,
        modifiers,
        owner_events: true,
        pointer_mode: 1,
        keyboard_mode: 1,
        event_mask: 0x0c,
    }
}

#[test]
fn active_grabs_conflict_only_inside_one_namespace() {
    let first = NamespaceId::from_raw(1);
    let second = NamespaceId::from_raw(2);
    let mut state = XInputAuthorityState::default();
    state.grab_pointer(first, active(1)).unwrap();
    assert_eq!(
        state.grab_pointer(first, active(2)),
        Err(XInputGrabError::AlreadyGrabbed)
    );
    state.grab_pointer(second, active(2)).unwrap();
    state.ungrab_pointer(first, 1);
    state.grab_pointer(first, active(2)).unwrap();
}

#[test]
fn any_detail_and_any_modifier_passive_grabs_detect_conflicts() {
    let namespace = NamespaceId::from_raw(1);
    let mut state = XInputAuthorityState::default();
    state
        .grab_key(namespace, passive(1, 0, X_ANY_MODIFIER))
        .unwrap();
    assert_eq!(
        state.grab_key(namespace, passive(2, 38, 4)),
        Err(XInputGrabError::AccessConflict)
    );
    state.ungrab_key(namespace, 1, XResourceId::new(0x100, 1), 0, X_ANY_MODIFIER);
    state.grab_key(namespace, passive(2, 38, 4)).unwrap();
}

#[test]
fn disconnect_cleanup_releases_every_owned_grab() {
    let namespace = NamespaceId::from_raw(1);
    let mut state = XInputAuthorityState::default();
    state.grab_pointer(namespace, active(1)).unwrap();
    state.grab_keyboard(namespace, active(1)).unwrap();
    state.grab_button(namespace, passive(1, 1, 0)).unwrap();
    state.grab_server(namespace, 1).unwrap();
    state.cleanup_owner(1);
    state.grab_pointer(namespace, active(2)).unwrap();
    state.grab_keyboard(namespace, active(2)).unwrap();
    state.grab_button(namespace, passive(2, 1, 0)).unwrap();
    state.grab_server(namespace, 2).unwrap();
}

#[test]
fn xi2_selection_masks_are_device_scoped_and_disconnect_cleaned() {
    let namespace = NamespaceId::from_raw(3);
    let window = XResourceId::new(0x100, 1);
    let mut state = XInputAuthorityState::default();
    state.select_xi_events(namespace, 7, window, &[(1, vec![1 << 6])]);
    assert!(state.xi_event_selected(namespace, 7, window, 2, 6));
    assert!(!state.xi_event_selected(namespace, 8, window, 2, 6));
    state.cleanup_owner(7);
    assert!(!state.xi_event_selected(namespace, 7, window, 2, 6));
}

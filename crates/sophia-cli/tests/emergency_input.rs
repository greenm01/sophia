use sophia_cli::emergency_input::{
    EVDEV_KEY_BACKSPACE, EVDEV_KEY_LEFTALT, EVDEV_KEY_LEFTCTRL, EVDEV_KEY_RIGHTALT,
    EVDEV_KEY_RIGHTCTRL, EmergencyChordAction, EmergencyChordState,
};

fn press(state: &mut EmergencyChordState, keycode: u32) -> EmergencyChordAction {
    state.observe(keycode, true)
}

fn release(state: &mut EmergencyChordState, keycode: u32) -> EmergencyChordAction {
    state.observe(keycode, false)
}

#[test]
fn first_complete_chord_arms_and_second_triggers_after_full_release() {
    let mut state = EmergencyChordState::awaiting_arm();

    assert_eq!(
        press(&mut state, EVDEV_KEY_LEFTCTRL),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_LEFTALT),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::Armed
    );
    assert!(state.is_armed());

    assert_eq!(
        release(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        release(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        release(&mut state, EVDEV_KEY_LEFTALT),
        EmergencyChordAction::None
    );
    assert_eq!(
        release(&mut state, EVDEV_KEY_LEFTCTRL),
        EmergencyChordAction::None
    );

    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_RIGHTALT),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_RIGHTCTRL),
        EmergencyChordAction::Triggered
    );
}

#[test]
fn armed_state_triggers_in_any_press_order_and_ignores_unrelated_keys() {
    let mut state = EmergencyChordState::armed();

    assert_eq!(press(&mut state, 30), EmergencyChordAction::None);
    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_RIGHTCTRL),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_LEFTALT),
        EmergencyChordAction::Triggered
    );
}

#[test]
fn partial_chords_and_repeats_do_not_trigger() {
    let mut state = EmergencyChordState::armed();

    assert_eq!(
        press(&mut state, EVDEV_KEY_LEFTCTRL),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        release(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
    assert_eq!(
        release(&mut state, EVDEV_KEY_LEFTCTRL),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_LEFTALT),
        EmergencyChordAction::None
    );
    assert_eq!(
        press(&mut state, EVDEV_KEY_BACKSPACE),
        EmergencyChordAction::None
    );
}

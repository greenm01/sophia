const SESSION_LAUNCHER: &str = include_str!("../../../tools/run_sophia_xmonad_session.sh");
const TTY3_LAUNCHER: &str = include_str!("../../../tools/start_sophia_kitty_tty3.sh");

fn offset(needle: &str) -> usize {
    SESSION_LAUNCHER
        .find(needle)
        .unwrap_or_else(|| panic!("launcher is missing {needle:?}"))
}

#[test]
fn graphical_takeover_disables_console_rendering_and_input_echo_after_guard_arming() {
    let guard_ready = offset("echo \"Emergency input guard armed.\"");
    let graphics = offset("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" graphics");
    let keyboard_off = offset("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" keyboard-off");
    let raw = offset("stty raw -echo");
    let session = offset("setsid \"${session_command[@]}\"");

    assert!(guard_ready < graphics);
    assert!(graphics < keyboard_off);
    assert!(keyboard_off < raw);
    assert!(raw < session);
}

#[test]
fn graphical_takeover_saves_and_restores_exact_tty_state() {
    let save_termios = offset("tty_state=\"$(stty -g)\"");
    let save_kd = offset("kd_mode=\"$(python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" get)\"");
    let save_keyboard =
        offset("keyboard_mode=\"$(python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" get-keyboard)\"");
    let graphics = offset("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" graphics");

    assert!(save_termios < graphics);
    assert!(save_kd < graphics);
    assert!(save_keyboard < graphics);
    assert!(
        SESSION_LAUNCHER.contains("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" \"$kd_mode\"")
    );
    assert!(SESSION_LAUNCHER.contains("stty \"$tty_state\""));
    assert!(
        SESSION_LAUNCHER
            .contains("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" \"keyboard-$keyboard_mode\"")
    );
}

#[test]
fn kitty_gate_always_retains_one_shot_composition_pixel_evidence() {
    assert!(SESSION_LAUNCHER.contains("SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=1"));
}

#[test]
fn kitty_gate_reactivates_its_originating_vt_after_display_manager_restore() {
    let restore_manager = TTY3_LAUNCHER
        .find("sudo sv up \"$display_manager\"")
        .unwrap();
    let reactivate_tty = TTY3_LAUNCHER.find("sudo chvt \"$origin_vt\"").unwrap();

    assert!(TTY3_LAUNCHER.contains("origin_tty=\"$(tty)\""));
    assert!(TTY3_LAUNCHER.contains("origin_vt=\"${origin_tty#/dev/tty}\""));
    assert!(restore_manager < reactivate_tty);
    assert!(TTY3_LAUNCHER.contains("active_vt=\"$(fgconsole 2>/dev/null || true)\""));
}

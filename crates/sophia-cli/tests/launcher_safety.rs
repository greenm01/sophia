const SESSION_LAUNCHER: &str = include_str!("../../../tools/run_sophia_xmonad_session.sh");

fn offset(needle: &str) -> usize {
    SESSION_LAUNCHER
        .find(needle)
        .unwrap_or_else(|| panic!("launcher is missing {needle:?}"))
}

#[test]
fn graphical_takeover_disables_console_rendering_and_input_echo_after_guard_arming() {
    let guard_ready = offset("echo \"Emergency input guard armed.\"");
    let graphics = offset("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" graphics");
    let raw = offset("stty raw -echo");
    let session = offset("setsid \"${session_command[@]}\"");

    assert!(guard_ready < graphics);
    assert!(graphics < raw);
    assert!(raw < session);
}

#[test]
fn graphical_takeover_saves_and_restores_exact_tty_state() {
    let save_termios = offset("tty_state=\"$(stty -g)\"");
    let save_kd = offset("kd_mode=\"$(python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" get)\"");
    let graphics = offset("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" graphics");

    assert!(save_termios < graphics);
    assert!(save_kd < graphics);
    assert!(
        SESSION_LAUNCHER.contains("python3 \"$ROOT_DIR/tools/sophia_tty_mode.py\" \"$kd_mode\"")
    );
    assert!(SESSION_LAUNCHER.contains("stty \"$tty_state\""));
}

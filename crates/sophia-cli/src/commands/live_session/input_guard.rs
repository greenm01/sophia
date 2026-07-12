use super::*;

use sophia_cli::emergency_input::{EmergencyChordAction, EmergencyChordState};

const INPUT_GUARD_GRACE_MSEC: u64 = 250;

pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let input_devices = arg_value(args, "--input-devices")
        .unwrap_or_default()
        .split(',')
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    if input_devices.is_empty()
        || input_devices.len() > 16
        || input_devices.iter().any(|path| !path.is_absolute())
    {
        return Err("sophia-session-input-guard requires 1-16 absolute input device paths".into());
    }
    let armed_file = required_absolute_path(args, "--armed-file")?;
    let triggered_file = required_absolute_path(args, "--triggered-file")?;
    let owner_pid = arg_value(args, "--owner-pid")
        .ok_or("sophia-session-input-guard requires --owner-pid=PID")?
        .parse::<u32>()
        .map_err(|error| format!("invalid input guard owner PID: {error}"))?;
    if owner_pid == 0 {
        return Err("input guard owner PID must be nonzero".into());
    }

    let mut poller = sophia_backend_live::open_native_libinput_path_poller(
        &input_devices,
        sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(SESSION_SEAT_RAW))
            .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW)),
        64,
    )?;
    let mut chord = EmergencyChordState::awaiting_arm();
    println!(
        "sophia_session_input_guard schema=1 status=ready devices={}",
        input_devices.len()
    );
    std::io::stdout().flush()?;

    loop {
        if !std::path::Path::new(&format!("/proc/{owner_pid}")).exists() {
            return Ok(());
        }
        for event in poller.poll_ready()? {
            let sophia_protocol::InputEventKind::Key { keycode, pressed } = event.kind else {
                continue;
            };
            match chord.observe(keycode, pressed) {
                EmergencyChordAction::None => {}
                EmergencyChordAction::Armed => {
                    std::fs::write(&armed_file, b"armed\n")?;
                    println!("sophia_session_input_guard schema=1 status=armed");
                    std::io::stdout().flush()?;
                }
                EmergencyChordAction::Triggered => {
                    std::fs::write(&triggered_file, b"triggered\n")?;
                    println!("sophia_session_input_guard schema=1 status=triggered");
                    std::io::stdout().flush()?;
                    std::thread::sleep(Duration::from_millis(INPUT_GUARD_GRACE_MSEC));
                    return Ok(());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn required_absolute_path(
    args: &[String],
    name: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = std::path::PathBuf::from(
        arg_value(args, name)
            .ok_or_else(|| format!("sophia-session-input-guard requires {name}=PATH"))?,
    );
    if !path.is_absolute() {
        return Err(format!("{name} must be an absolute path").into());
    }
    Ok(path)
}

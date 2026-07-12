use super::*;

use std::collections::BTreeMap;
use std::process::Stdio;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

use sophia_protocol::{AuthorityKind, SurfaceTransactionReadiness};
use sophia_x_authority::{
    XAuthorityControlAck, XAuthorityControlCommand, XAuthorityControlOutcome, XAuthorityInputEvent,
};
use sophia_x_bridge::LiveCompositeCapture;

const COMPAT_CAPTURE_INTERVAL: Duration = Duration::from_millis(33);

pub(crate) fn run_persistent_xlibre_session(
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = PersistentXtermSessionConfig::from_args(args)?;
    if config.wm_process.is_some() {
        return Err("the first xlibre compatibility gate is standalone; omit --wm-process".into());
    }
    let display = arg_value(args, "--compat-display").unwrap_or_else(|| ":178".to_owned());
    config.display.clone_from(&display);
    config.input_quiet_msec = 150;
    let client_name = arg_value(args, "--client").unwrap_or_else(|| "kitty".to_owned());
    let client = super::super::x_authority::resolve_external_probe_binary("client", &client_name)?;
    let client_args = args
        .iter()
        .filter_map(|arg| arg.strip_prefix("--client-arg="))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if client_args.len() > 64 || client_args.iter().any(|argument| argument.len() > 4_096) {
        return Err("--client-arg accepts at most 64 bounded arguments".into());
    }

    let mut native_scanout = config
        .native_scanout
        .then(PersistentNativeScanout::new)
        .transpose()?;
    let mut physical_input = if config.input_devices.is_empty() {
        None
    } else {
        Some(sophia_backend_live::open_native_libinput_path_poller(
            &config.input_devices,
            sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(SESSION_SEAT_RAW))
                .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
                .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW)),
            64,
        )?)
    };

    let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
    let (input_sender, input_receiver) = sync_channel(SESSION_KEY_CAPACITY);
    let (control_sender, control_receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (control_ack_sender, control_ack_receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let provider_display = display.clone();
    let provider = std::thread::spawn(move || {
        run_compat_provider(
            &provider_display,
            authority_sender,
            input_receiver,
            control_receiver,
            control_ack_sender,
        )
    });

    wait_for_compat_provider(&authority_receiver, &display)?;
    let mut command = std::process::Command::new(client);
    command
        .env("DISPLAY", &display)
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env_remove("WAYLAND_DISPLAY")
        .args(&client_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn()?;
    let mut process = SessionProcessGuard::child_only(child);
    println!(
        "sophia_live_session schema=8 status=running display={} client_backend=xlibre-compat client={} capture_msec={} native_presentation={} physical_input={}",
        display,
        client_name,
        COMPAT_CAPTURE_INTERVAL.as_millis(),
        if native_scanout.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if physical_input.is_some() {
            "enabled"
        } else {
            "disabled"
        },
    );

    let mut wm_session = None;
    let result = run_session_loop(
        &config,
        &authority_receiver,
        &input_sender,
        &control_sender,
        &control_ack_receiver,
        process.child_mut()?,
        &mut physical_input,
        &mut native_scanout,
        &mut wm_session,
    );
    process.terminate()?;
    drop(input_sender);
    drop(control_sender);
    drop(authority_receiver);
    let provider_result = provider
        .join()
        .map_err(|_| "XLibre compatibility provider thread panicked")?;
    provider_result.map_err(|error| format!("XLibre compatibility provider failed: {error}"))?;
    result
}

fn wait_for_compat_provider(
    receiver: &Receiver<XAuthorityObservedTransactionBatch>,
    display: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // The provider connects before the client exists and therefore normally
    // emits no batch yet. A short grace period catches authentication/socket
    // failures without adding an arbitrary client-start sleep.
    match receiver.recv_timeout(Duration::from_millis(100)) {
        Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(()),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(format!("XLibre compatibility provider could not connect to {display}").into())
        }
    }
}

fn run_compat_provider(
    display: &str,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
    input: Receiver<XAuthorityInputEvent>,
    control: Receiver<XAuthorityControlCommand>,
    control_ack: SyncSender<XAuthorityControlAck>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut capture = LiveCompositeCapture::connect(Some(display))?;
    let mut generations = BTreeMap::new();
    let mut checksums = BTreeMap::new();
    let mut serial = 1u64;
    let mut last_capture = Instant::now() - COMPAT_CAPTURE_INTERVAL;
    loop {
        loop {
            match input.try_recv() {
                Ok(XAuthorityInputEvent::Key(event)) => {
                    capture.inject_key(event.keycode, event.pressed, event.time_msec)?;
                }
                Ok(XAuthorityInputEvent::Pointer(_)) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        loop {
            match control.try_recv() {
                Ok(command) => {
                    let _ = control_ack.try_send(XAuthorityControlAck {
                        transaction: command.transaction(),
                        surface: command.surface(),
                        outcome: XAuthorityControlOutcome::AuthorityRejected,
                    });
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if last_capture.elapsed() < COMPAT_CAPTURE_INTERVAL {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        last_capture = Instant::now();
        let captured = capture.capture()?;
        if captured.readbacks.is_empty() {
            continue;
        }
        let transaction_id = TransactionId::from_raw(serial);
        serial = serial
            .checked_add(1)
            .ok_or("compat transaction ID exhausted")?;
        let mut updates = Vec::with_capacity(captured.readbacks.len());
        let mut changed_handles = std::collections::BTreeSet::new();
        for buffer in captured.readbacks {
            let checksum = buffer
                .bytes
                .iter()
                .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                });
            if checksums.get(&buffer.handle).copied() == Some(checksum) {
                continue;
            }
            checksums.insert(buffer.handle, checksum);
            changed_handles.insert(buffer.handle);
            let width = usize::try_from(buffer.size.width)?;
            let stride = width.checked_mul(4).ok_or("compat stride overflow")?;
            let generation = generations
                .get(&buffer.handle)
                .copied()
                .unwrap_or(0u64)
                .saturating_add(1);
            generations.insert(buffer.handle, generation);
            updates.push(XAuthorityCpuBufferUpdate::Replace(
                XAuthorityCpuBufferSnapshot {
                    handle: buffer.handle,
                    drawable: XResourceId::new(u64::from(buffer.pixmap), 1),
                    size: buffer.size,
                    stride: u32::try_from(stride)?,
                    format: sophia_x_authority::X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                    generation,
                    bytes: buffer.bytes,
                },
            ));
        }
        let transactions = captured
            .surfaces
            .iter()
            .filter(|surface| {
                matches!(
                    surface.source,
                    BufferSource::CpuBuffer { handle } if changed_handles.contains(&handle)
                )
            })
            .map(|surface| {
                let previous = generations
                    .get(&match surface.source {
                        BufferSource::CpuBuffer { handle } => handle,
                        _ => 0,
                    })
                    .copied()
                    .unwrap_or(1)
                    .saturating_sub(1);
                SurfaceTransaction::from_surface_snapshot(
                    transaction_id,
                    AuthorityKind::XLibrePrototype,
                    surface,
                    SurfaceTransactionReadiness::Ready,
                    250,
                    previous,
                )
            })
            .collect::<Vec<_>>();
        if transactions.is_empty() {
            continue;
        }
        if sender
            .send(XAuthorityObservedTransactionBatch {
                transaction: transaction_id,
                transactions,
                cpu_buffer_updates: updates,
            })
            .is_err()
        {
            return Ok(());
        }
    }
}

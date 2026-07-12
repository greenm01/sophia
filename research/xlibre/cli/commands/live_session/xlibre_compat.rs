use super::*;

use std::collections::BTreeMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

use sophia_protocol::{AuthorityKind, SurfaceTransactionReadiness};
use sophia_x_authority::{
    XAuthorityControlAck, XAuthorityControlCommand, XAuthorityControlOutcome, XAuthorityInputEvent,
};
use sophia_x_bridge::{LiveCompositeCapture, LiveReadbackPath, LiveXTestInput};

const COMPAT_CAPTURE_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, Default)]
struct CompatInputStats {
    keys_injected: usize,
    max_inject: Duration,
}

#[derive(Clone, Copy, Debug)]
struct CompatCaptureStats {
    full_readbacks: usize,
    patch_readbacks: usize,
    bytes_read: usize,
    max_capture: Duration,
    readback_path: LiveReadbackPath,
    shm_fallbacks: usize,
    max_readback_bytes: usize,
}

impl Default for CompatCaptureStats {
    fn default() -> Self {
        Self {
            full_readbacks: 0,
            patch_readbacks: 0,
            bytes_read: 0,
            max_capture: Duration::ZERO,
            readback_path: LiveReadbackPath::GetImageDegraded,
            shm_fallbacks: 0,
            max_readback_bytes: 0,
        }
    }
}

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
        Some(SessionPhysicalInput::Threaded(
            sophia_backend_live::open_threaded_native_libinput_path_poller(
                &config.input_devices,
                sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(
                    SESSION_SEAT_RAW,
                ))
                .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
                .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW)),
                64,
                256,
            )?,
        ))
    };
    if !config.input_devices.is_empty() {
        println!(
            "sophia_live_session_input_pipeline schema=1 status=poller_ready devices={}",
            config.input_devices.len()
        );
        std::io::stdout().flush()?;
    }

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
        "sophia_live_session schema=9 status=running display={} client_backend=xlibre-compat client={} capture_msec={} native_presentation={} physical_input={}",
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
        true,
    );
    drop(input_sender);
    drop(control_sender);
    drop(authority_receiver);
    let provider_result = provider
        .join()
        .map_err(|_| "XLibre compatibility provider thread panicked")?;
    process.terminate()?;
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
    let stop_input = Arc::new(AtomicBool::new(false));
    let input_stop = Arc::clone(&stop_input);
    let input_display = display.to_owned();
    let (input_health_sender, input_health_receiver) = std::sync::mpsc::sync_channel(1);
    let input_worker = std::thread::spawn(move || {
        let result = run_compat_input(&input_display, input, &input_stop);
        let health = result.as_ref().map(|_| ()).map_err(ToString::to_string);
        let _ = input_health_sender.try_send(health);
        result
    });

    let capture_result = run_compat_capture_provider(
        display,
        sender,
        control,
        control_ack,
        &input_health_receiver,
    );
    stop_input.store(true, Ordering::Release);
    let input_result = input_worker
        .join()
        .map_err(|_| "XLibre compatibility input worker panicked")?;
    let capture_stats = capture_result?;
    let input_stats = input_result?;
    println!(
        "sophia_xlibre_compat schema=2 status=complete capture_path={} shm_fallbacks={} full_readbacks={} patch_readbacks={} bytes_read={} max_readback_bytes={} max_capture_msec={} keys_injected={} max_inject_msec={}",
        capture_stats.readback_path.evidence_name(),
        capture_stats.shm_fallbacks,
        capture_stats.full_readbacks,
        capture_stats.patch_readbacks,
        capture_stats.bytes_read,
        capture_stats.max_readback_bytes,
        capture_stats.max_capture.as_millis(),
        input_stats.keys_injected,
        input_stats.max_inject.as_millis(),
    );
    std::io::stdout().flush()?;
    Ok(())
}

fn run_compat_input(
    display: &str,
    input: Receiver<XAuthorityInputEvent>,
    stop: &AtomicBool,
) -> Result<CompatInputStats, Box<dyn std::error::Error + Send + Sync>> {
    let injector = LiveXTestInput::connect(Some(display))?;
    let mut key_injected_reported = false;
    let mut stats = CompatInputStats::default();
    while !stop.load(Ordering::Acquire) {
        match input.recv_timeout(Duration::from_millis(5)) {
            Ok(XAuthorityInputEvent::Key(event)) => {
                let started = Instant::now();
                injector.inject_key(event.keycode, event.pressed, event.time_msec)?;
                stats.keys_injected = stats.keys_injected.saturating_add(1);
                stats.max_inject = stats.max_inject.max(started.elapsed());
                if !key_injected_reported {
                    println!("sophia_live_session_input_pipeline schema=1 status=key_injected");
                    std::io::stdout().flush()?;
                    key_injected_reported = true;
                }
            }
            Ok(XAuthorityInputEvent::Pointer(_)) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(stats),
        }
    }
    Ok(stats)
}

fn run_compat_capture_provider(
    display: &str,
    sender: SyncSender<XAuthorityObservedTransactionBatch>,
    control: Receiver<XAuthorityControlCommand>,
    control_ack: SyncSender<XAuthorityControlAck>,
    input_health: &Receiver<Result<(), String>>,
) -> Result<CompatCaptureStats, Box<dyn std::error::Error + Send + Sync>> {
    let mut capture = LiveCompositeCapture::connect(Some(display))?;
    let mut generations = BTreeMap::new();
    let mut serial = 1u64;
    let mut last_capture = Instant::now() - COMPAT_CAPTURE_INTERVAL;
    let mut stats = CompatCaptureStats::default();
    loop {
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
                Err(TryRecvError::Disconnected) => return Ok(stats),
            }
        }
        match input_health.try_recv() {
            Ok(Ok(())) => return Ok(stats),
            Ok(Err(error)) => {
                return Err(format!("XLibre compatibility input worker failed: {error}").into());
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err("XLibre compatibility input health channel disconnected".into());
            }
        }
        if last_capture.elapsed() < COMPAT_CAPTURE_INTERVAL {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        last_capture = Instant::now();
        let capture_started = Instant::now();
        let captured = capture.capture()?;
        stats.max_capture = stats.max_capture.max(capture_started.elapsed());
        stats.readback_path = captured.readback_path;
        stats.shm_fallbacks = captured.shm_fallbacks;
        stats.max_readback_bytes = captured.max_readback_bytes;
        for update in &captured.updates {
            stats.bytes_read = stats.bytes_read.saturating_add(update.byte_len());
            match update {
                sophia_x_bridge::LiveCpuBufferUpdate::Replace(_) => {
                    stats.full_readbacks = stats.full_readbacks.saturating_add(1);
                }
                sophia_x_bridge::LiveCpuBufferUpdate::Patch(_) => {
                    stats.patch_readbacks = stats.patch_readbacks.saturating_add(1);
                }
            }
        }
        if captured.updates.is_empty() {
            continue;
        }
        let transaction_id = TransactionId::from_raw(serial);
        serial = serial
            .checked_add(1)
            .ok_or("compat transaction ID exhausted")?;
        let mut updates = Vec::with_capacity(captured.updates.len());
        let mut changed_handles = std::collections::BTreeSet::new();
        for buffer in captured.updates {
            let handle = buffer.handle();
            changed_handles.insert(handle);
            let size = match &buffer {
                sophia_x_bridge::LiveCpuBufferUpdate::Replace(buffer) => buffer.size,
                sophia_x_bridge::LiveCpuBufferUpdate::Patch(buffer) => buffer.size,
            };
            let width = usize::try_from(size.width)?;
            let stride = width.checked_mul(4).ok_or("compat stride overflow")?;
            let generation = generations
                .get(&handle)
                .copied()
                .unwrap_or(0u64)
                .saturating_add(1);
            generations.insert(handle, generation);
            updates.push(match buffer {
                sophia_x_bridge::LiveCpuBufferUpdate::Replace(buffer) => {
                    XAuthorityCpuBufferUpdate::Replace(XAuthorityCpuBufferSnapshot {
                        handle: buffer.handle,
                        drawable: XResourceId::new(u64::from(buffer.pixmap), 1),
                        size: buffer.size,
                        stride: u32::try_from(stride)?,
                        format: sophia_x_authority::X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                        generation,
                        bytes: buffer.bytes,
                    })
                }
                sophia_x_bridge::LiveCpuBufferUpdate::Patch(buffer) => {
                    XAuthorityCpuBufferUpdate::Patch(XAuthorityCpuBufferPatch {
                        handle: buffer.handle,
                        drawable: XResourceId::new(u64::from(buffer.pixmap), 1),
                        size: buffer.size,
                        stride: u32::try_from(stride)?,
                        format: sophia_x_authority::X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                        generation,
                        rect: buffer.rect,
                        bytes: buffer.bytes,
                    })
                }
            });
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
            return Ok(stats);
        }
    }
}

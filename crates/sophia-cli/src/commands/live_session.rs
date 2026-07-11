use super::prelude::*;

use sophia_engine::{FocusedInputRoute, InputFocusState, NonBlockingInputPoller};
use sophia_protocol::{DeviceId, SeatId};
use sophia_x_authority::{XCoreKeyboardMapper, XCorePointerMapper};
use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};

const SESSION_AUTHORITY_CAPACITY: usize = 256;
const SESSION_KEY_CAPACITY: usize = 64;
const SESSION_INPUT_QUIET_MSEC: u64 = 100;
const SESSION_PHYSICAL_INPUT_TIMEOUT_MSEC: u64 = 5_000;
const SESSION_SEAT_RAW: u64 = 1;
const SESSION_KEYBOARD_DEVICE_RAW: u64 = 1;
const SESSION_POINTER_DEVICE_RAW: u64 = 2;

pub(crate) fn run_persistent_xterm_session(
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = PersistentXtermSessionConfig::from_args(args)?;
    let terminal = super::x_authority::resolve_external_probe_binary("xterm", &config.terminal)?;
    prepare_display_socket(&config.socket_path)?;
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

    let server_path = config.socket_path.clone();
    let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
    let (input_sender, input_receiver) = sync_channel(SESSION_KEY_CAPACITY);
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_channels(
            &server_path,
            NamespaceId::from_raw(50),
            authority_sender,
            input_receiver,
        )
    });
    super::x_authority::wait_for_socket_path(&config.socket_path)?;

    let mut terminal_command = std::process::Command::new(terminal);
    terminal_command
        .env("DISPLAY", &config.display)
        .args([
            "-cm",
            "-dc",
            "-geometry",
            "120x36",
            "-title",
            "Sophia Terminal",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if config.input_proof_requested() {
        terminal_command.args([
            "-e",
            "sh",
            "-c",
            "read line; printf 'received:%s\\n' \"$line\"; sleep 5",
        ]);
    }
    let child = terminal_command.spawn()?;
    let mut process = SessionProcessGuard::new(child, config.socket_path.clone());

    println!(
        "sophia_live_session schema=6 status=running display={} terminal=xterm runtime=persistent authority_capacity={} input_capacity={} native_presentation={} physical_input={} pointer_proof={}",
        config.display,
        SESSION_AUTHORITY_CAPACITY,
        SESSION_KEY_CAPACITY,
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
        if config.expect_physical_pointer {
            "enabled"
        } else {
            "disabled"
        },
    );

    let result = run_session_loop(
        &config,
        &authority_receiver,
        &input_sender,
        process.child_mut()?,
        &mut physical_input,
        &mut native_scanout,
    );
    process.terminate()?;
    drop(input_sender);
    let server_result = server
        .join()
        .map_err(|_| "persistent X authority server thread panicked")?;
    server_result.map_err(|error| format!("persistent X authority server failed: {error}"))?;
    result
}

#[derive(Clone, Debug)]
struct PersistentXtermSessionConfig {
    display: String,
    socket_path: std::path::PathBuf,
    terminal: String,
    max_runtime: Option<Duration>,
    max_ticks: Option<usize>,
    inject_text: Option<String>,
    expect_physical_text: Option<String>,
    expect_physical_pointer: bool,
    input_devices: Vec<std::path::PathBuf>,
    native_scanout: bool,
}

impl PersistentXtermSessionConfig {
    fn from_args(args: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let display = arg_value(args, "--display").unwrap_or_else(|| ":77".to_owned());
        let display_number = parse_display_number(&display)?;
        let max_runtime = arg_value(args, "--max-runtime-ms")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .map(Duration::from_millis);
        let max_ticks = arg_value(args, "--max-ticks")
            .as_deref()
            .map(parse_usize)
            .transpose()?;
        if max_ticks.is_some_and(|ticks| ticks == 0 || ticks > 1_000_000) {
            return Err("--max-ticks accepts a value from 1 through 1000000".into());
        }
        let inject_text = arg_value(args, "--inject-text");
        let expect_physical_text = arg_value(args, "--expect-physical-text");
        let expect_physical_pointer = args.iter().any(|arg| arg == "--expect-physical-pointer");
        let native_scanout = args.iter().any(|arg| arg == "--native-scanout");
        if native_scanout && std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
            return Err(
                "set SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 to run persistent native scanout"
                    .into(),
            );
        }
        let input_devices = arg_value(args, "--input-devices")
            .map(|paths| {
                paths
                    .split(',')
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if input_devices.len() > 16
            || input_devices
                .iter()
                .any(|path| !path.is_absolute() || path.as_os_str().is_empty())
        {
            return Err("--input-devices accepts 1-16 comma-separated absolute paths".into());
        }
        if inject_text.is_some() && expect_physical_text.is_some() {
            return Err("--inject-text and --expect-physical-text are mutually exclusive".into());
        }
        if (inject_text.is_some() || expect_physical_text.is_some())
            && max_runtime.is_none()
            && max_ticks.is_none()
        {
            return Err(
                "input proof flags require --max-runtime-ms or --max-ticks for a bounded proof"
                    .into(),
            );
        }
        if expect_physical_text.is_some() && input_devices.is_empty() {
            return Err("--expect-physical-text requires --input-devices".into());
        }
        if expect_physical_pointer && expect_physical_text.is_none() {
            return Err(
                "--expect-physical-pointer requires --expect-physical-text for visible content"
                    .into(),
            );
        }
        if let Some(text) = inject_text.as_ref().or(expect_physical_text.as_ref())
            && (text.is_empty()
                || text.len() > 24
                || !text.bytes().all(|byte| byte.is_ascii_lowercase()))
        {
            return Err("input proof text accepts 1-24 lowercase ASCII letters".into());
        }
        Ok(Self {
            display,
            socket_path: std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}")),
            terminal: arg_value(args, "--terminal").unwrap_or_else(|| "xterm".to_owned()),
            max_runtime,
            max_ticks,
            inject_text,
            expect_physical_text,
            expect_physical_pointer,
            input_devices,
            native_scanout,
        })
    }

    fn input_proof_requested(&self) -> bool {
        self.inject_text.is_some() || self.expect_physical_text.is_some()
    }
}

fn parse_display_number(display: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let raw = display
        .strip_prefix(':')
        .filter(|raw| !raw.is_empty() && raw.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| format!("invalid local X display {display:?}; expected :NUMBER"))?;
    let display_number = raw.parse::<u32>()?;
    if display_number > u16::MAX.into() {
        return Err(format!("X display number {display_number} exceeds u16").into());
    }
    Ok(display_number)
}

fn prepare_display_socket(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("/tmp/.X11-unix")?;
    if !path.exists() {
        return Ok(());
    }
    if UnixStream::connect(path).is_ok() {
        return Err(format!("X display socket {} is already active", path.display()).into());
    }
    std::fs::remove_file(path)?;
    Ok(())
}

fn run_session_loop(
    config: &PersistentXtermSessionConfig,
    authority_receiver: &Receiver<XAuthorityObservedTransactionBatch>,
    input_sender: &SyncSender<XAuthorityInputEvent>,
    child: &mut Child,
    physical_input: &mut Option<
        sophia_backend_live::NativeLibinputEventPoller<
            sophia_backend_live::NativeLibinputEventReader,
        >,
    >,
    native_scanout: &mut Option<PersistentNativeScanout>,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let deadline = config.max_runtime.map(|duration| started + duration);
    let output = native_scanout
        .as_ref()
        .map(PersistentNativeScanout::output)
        .unwrap_or_else(sophia_engine::HeadlessOutput::deterministic);
    let mut scene = PersistentCpuScene::new(output.size);
    let mut runtime = None;
    let mut last_authority_update = started;
    let mut injection_checksum = None;
    let mut physical_input_ready_at = None;
    let mut input_pixel_change = false;
    let mut pointer_checksum = None;
    let mut pointer_phase_started_at = None;
    let mut pointer_pixel_change = false;
    let mut batches = 0usize;
    let mut transactions = 0usize;
    let mut backend_ticks = 0usize;
    let mut runtime_committed = 0u64;
    let mut runtime_surfaces = 0u64;
    let mut focus = InputFocusState::new();
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut pointer = XCorePointerMapper::new();
    let mut physical_events = 0usize;
    let mut physical_keys_routed = 0usize;
    let mut physical_pointer_events = 0usize;
    let mut physical_pointer_routed = 0usize;
    let mut session_ticks = 0usize;
    let seat = SeatId::from_raw(SESSION_SEAT_RAW);

    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!("xterm exited during live session with status {status}").into());
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }
        let waiting_for_keyboard_pixels = physical_input_ready_at.is_some() && !input_pixel_change;
        let waiting_for_pointer_pixels =
            config.expect_physical_pointer && input_pixel_change && !pointer_pixel_change;
        if waiting_for_pointer_pixels && pointer_phase_started_at.is_none() {
            pointer_phase_started_at = Some(Instant::now());
        }
        let proof_wait_started_at = if waiting_for_keyboard_pixels {
            physical_input_ready_at
        } else if waiting_for_pointer_pixels {
            pointer_phase_started_at
        } else {
            None
        };
        if let Some(proof_wait_started_at) = proof_wait_started_at {
            if proof_wait_started_at.elapsed()
                >= Duration::from_millis(SESSION_PHYSICAL_INPUT_TIMEOUT_MSEC)
            {
                return Err(format!(
                    "persistent live session timed out waiting for physical input pixels: keyboard_routed={physical_keys_routed} keyboard_pixels={input_pixel_change} pointer_observed={physical_pointer_events} pointer_routed={physical_pointer_routed} pointer_pixels={pointer_pixel_change} pointer_baseline={pointer_checksum:?} final_checksum={:?}",
                    scene.last_report.as_ref().map(|report| report.checksum)
                )
                .into());
            }
        } else {
            if config
                .max_ticks
                .is_some_and(|max_ticks| session_ticks >= max_ticks)
            {
                break;
            }
            session_ticks = session_ticks.saturating_add(1);
        }

        match authority_receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(batch) => {
                last_authority_update = Instant::now();
                batches = batches.saturating_add(1);
                transactions = transactions.saturating_add(batch.transactions.len());
                scene.observe(&batch);
                let report = scene.compose()?;
                let native_frame = native_scanout.as_ref().map(|_| report.frame.clone());
                if let Some((before_frame, _)) = injection_checksum
                    && report.checksum != before_frame
                    && (config.expect_physical_text.is_none() || physical_keys_routed > 0)
                {
                    input_pixel_change = true;
                }
                if let Some(before_frame) = pointer_checksum
                    && report.checksum != before_frame
                    && physical_pointer_routed > 0
                {
                    pointer_pixel_change = true;
                }

                if runtime.is_none() {
                    runtime = Some(PersistentBackendRuntime::new(
                        output,
                        &batch.transactions,
                        native_scanout.as_mut(),
                        native_frame.clone(),
                    )?);
                }
                let runtime = runtime
                    .as_mut()
                    .expect("persistent backend runtime was initialized above");
                let tick = runtime.run_batch(&batch, native_scanout.as_mut(), native_frame)?;
                backend_ticks = backend_ticks.saturating_add(1);
                runtime_committed = tick
                    .engine
                    .runtime
                    .runtime_state
                    .authority_transactions_committed;
                runtime_surfaces = tick.engine.runtime.runtime_state.authority_surfaces_applied;
                if focus.focused_surface(seat).is_none()
                    && let Some(surface) = runtime.committed_surfaces().first()
                {
                    let _ =
                        focus.focus_surface(seat, surface.surface, runtime.committed_surfaces());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let (Some(runtime), Some(native_scanout)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    if runtime.runtime.rendered_primary_plane_scanout_in_flight() {
                        runtime.retire_native_scanout(native_scanout)?;
                    }
                    if !runtime.runtime.rendered_primary_plane_scanout_in_flight()
                        && native_scanout.exporter.pending_cpu_frame()
                    {
                        let tick = runtime.run_native_idle(native_scanout)?;
                        backend_ticks = backend_ticks.saturating_add(1);
                        runtime_committed = tick
                            .engine
                            .runtime
                            .runtime_state
                            .authority_transactions_committed;
                        runtime_surfaces =
                            tick.engine.runtime.runtime_state.authority_surfaces_applied;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("persistent X authority transaction channel disconnected".into());
            }
        }

        if let (Some(poller), Some(runtime)) = (physical_input.as_mut(), runtime.as_ref()) {
            let report = route_physical_input(
                poller,
                &focus,
                runtime.committed_surfaces(),
                &runtime.input_layers(),
                input_sender,
                &mut modifiers,
                &mut pointer,
            )?;
            physical_events = physical_events.saturating_add(report.events);
            physical_keys_routed = physical_keys_routed.saturating_add(report.keys_routed);
            physical_pointer_events = physical_pointer_events.saturating_add(report.pointer_events);
            physical_pointer_routed = physical_pointer_routed.saturating_add(report.pointer_routed);
            if report.pointer_events > 0 {
                println!(
                    "sophia_live_session_pointer schema=1 status=observed events={} routed={}",
                    report.pointer_events, report.pointer_routed
                );
                std::io::stdout().flush()?;
            }
        }

        if injection_checksum.is_none()
            && config.input_proof_requested()
            && scene.last_report.is_some()
            && last_authority_update.elapsed() >= Duration::from_millis(SESSION_INPUT_QUIET_MSEC)
        {
            injection_checksum = scene
                .last_report
                .as_ref()
                .map(|report| (report.checksum, scene.buffer_checksum()));
            if let Some(text) = config.inject_text.as_deref() {
                send_test_text(input_sender, text)?;
            } else {
                physical_input_ready_at = Some(Instant::now());
                println!(
                    "sophia_live_session_input schema=1 status=ready source=physical text={}",
                    config
                        .expect_physical_text
                        .as_deref()
                        .expect("checked above")
                );
                std::io::stdout().flush()?;
            }
        }
        if config.expect_physical_pointer
            && input_pixel_change
            && pointer_checksum.is_none()
            && scene.last_report.is_some()
            && last_authority_update.elapsed() >= Duration::from_millis(SESSION_INPUT_QUIET_MSEC)
        {
            pointer_checksum = scene.last_report.as_ref().map(|report| report.checksum);
            println!(
                "sophia_live_session_pointer schema=1 status=ready source=physical action=select"
            );
            std::io::stdout().flush()?;
        }
    }

    if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
        runtime.drain_native_scanout(native_scanout, Duration::from_secs(2))?;
    }

    let report = scene
        .last_report
        .as_ref()
        .ok_or("persistent live session received no composable X pixels")?;
    if config.input_proof_requested() && !input_pixel_change {
        return Err(format!(
            "persistent live session input did not change composed terminal pixels: baseline={injection_checksum:?} final_frame={} final_buffers={} batches={batches} transactions={transactions}",
            report.checksum,
            scene.buffer_checksum(),
        )
        .into());
    }
    if config.expect_physical_text.is_some() && physical_keys_routed == 0 {
        return Err("persistent live session received no routed physical keys".into());
    }
    if config.expect_physical_pointer && (!pointer_pixel_change || physical_pointer_routed == 0) {
        return Err(format!(
            "persistent live session pointer input did not change pixels: baseline={pointer_checksum:?} routed={physical_pointer_routed} observed={physical_pointer_events}"
        )
        .into());
    }
    println!(
        "sophia_live_session schema=6 status=bounded_complete display={} elapsed_msec={} session_ticks={} authority_batches={} authority_transactions={} authority_queue_capacity={} authority_batches_dropped=0 backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_max_nonzero_pixel_bytes={} cpu_nonzero_frames={} cpu_checksum={} injected_input={} input_pixel_change={} physical_events={} physical_keys_routed={} pointer_pixel_change={} physical_pointer_events={} physical_pointer_routed={} pointer_proof={} native_presentation={} native_submissions={} native_submit_deferred={} native_submit_failures={} native_retirements={} native_retire_failures={} native_max_in_flight_ticks={} native_max_submit_to_page_flip_msec={} native_callback_accepted={} native_callback_rejected={} native_callback_queue_saturated={} native_nonzero_exports={} native_export_attempts={} native_in_flight={} native_cleanup_pending={} physical_input={}",
        config.display,
        started.elapsed().as_millis(),
        session_ticks,
        batches,
        transactions,
        SESSION_AUTHORITY_CAPACITY,
        backend_ticks,
        runtime_committed,
        runtime_surfaces,
        report.layers_composed,
        report.nonzero_pixel_bytes,
        scene.max_nonzero_pixel_bytes,
        scene.nonzero_frames,
        report.checksum,
        config.inject_text.is_some(),
        input_pixel_change,
        physical_events,
        physical_keys_routed,
        pointer_pixel_change,
        physical_pointer_events,
        physical_pointer_routed,
        if config.expect_physical_pointer {
            "enabled"
        } else {
            "disabled"
        },
        if native_scanout.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submissions),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_deferred),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.submit_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retirements),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.retire_failures),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.max_in_flight_ticks),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.max_submit_to_page_flip.as_millis()),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_accepted),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_rejected),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.callback_queue_saturated),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.nonzero_exports),
        native_scanout
            .as_ref()
            .map_or(0, |native| native.exporter.cpu_frame_export_attempts()),
        runtime
            .as_ref()
            .is_some_and(|runtime| { runtime.runtime.rendered_primary_plane_scanout_in_flight() }),
        runtime.as_ref().is_some_and(|runtime| {
            runtime
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
        }),
        if physical_input.is_some() {
            "enabled"
        } else {
            "disabled"
        },
    );
    if let (Some(runtime), Some(native_scanout)) = (runtime.as_ref(), native_scanout.as_ref())
        && (native_scanout.submissions == 0
            || native_scanout.retirements == 0
            || native_scanout.nonzero_exports == 0
            || native_scanout.submit_failures != 0
            || native_scanout.retire_failures != 0
            || native_scanout.callback_rejected != 0
            || native_scanout.callback_queue_saturated != 0
            || runtime.runtime.rendered_primary_plane_scanout_in_flight()
            || runtime
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending())
    {
        return Err("persistent native scanout did not submit, retire, and drain cleanly".into());
    }
    Ok(())
}

struct PersistentBackendRuntime {
    runtime: sophia_backend_live::LiveBackendRuntimeAssembly,
    authority_sender: SyncSender<AuthorityTransactionIntake>,
    layers: BTreeMap<SurfaceId, SurfaceTransaction>,
}

impl PersistentBackendRuntime {
    fn new(
        output: sophia_engine::HeadlessOutput,
        first_transactions: &[SurfaceTransaction],
        native_scanout: Option<&mut PersistentNativeScanout>,
        initial_native_frame: Option<sophia_backend_live::LiveCpuComposedFrame>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
        let assembly = HeadlessCompositorBackendAssembly::new(output)
            .with_committed_surfaces(seed_committed_surfaces(first_transactions))
            .with_authority_inbox(AuthorityTransactionInbox::new(
                authority_receiver,
                SESSION_AUTHORITY_CAPACITY,
            ));
        let renderer = sophia_backend_live::LiveRendererRuntimeObservation::from_startup_status(
            sophia_backend_live::LiveRendererImportStartupStatus::from_path_statuses(
                sophia_backend_live::LiveRendererImportPathStatus::Disabled,
                sophia_backend_live::LiveRendererImportPathStatus::Disabled,
            ),
            sophia_backend_live::LiveRendererSelectionObservation::CpuFallback,
        );
        let mut runtime =
            sophia_backend_live::LiveBackendRuntimeAssembly::from_ready_headless_scanout(
                assembly, output, renderer,
            )
            .with_persistent_rendered_primary_plane_scanout();
        if let Some(native_scanout) = native_scanout {
            runtime = runtime.with_page_flip_callback_queue(
                sophia_backend_live::LivePageFlipCallbackQueue::new(
                    native_scanout.take_receiver(),
                    64,
                ),
            );
            native_scanout.initialize(
                &mut runtime,
                initial_native_frame.ok_or("persistent native scanout has no initial CPU frame")?,
            )?;
        }
        Ok(Self {
            runtime,
            authority_sender,
            layers: BTreeMap::new(),
        })
    }

    fn run_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
        native_scanout: Option<&mut PersistentNativeScanout>,
        native_frame: Option<sophia_backend_live::LiveCpuComposedFrame>,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        for transaction in &batch.transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        let intake = AuthorityTransactionIntake::new(batch.transaction, batch.transactions.clone());
        self.authority_sender
            .try_send(intake)
            .map_err(|error| match error {
                TrySendError::Full(_) => "persistent live backend authority inbox is full",
                TrySendError::Disconnected(_) => {
                    "persistent live backend authority inbox is disconnected"
                }
            })?;
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let input = compositor_tick_input(&transactions, batch.transactions.len());
        match native_scanout {
            Some(native_scanout) => {
                if let Some(frame) = native_frame {
                    native_scanout.queue_frame(frame);
                }
                native_scanout.run_tick(&mut self.runtime, input)
            }
            None => self.runtime.run_tick(input).map_err(Into::into),
        }
    }

    fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        self.runtime.assembly().committed_surfaces()
    }

    fn input_layers(&self) -> Vec<LayerSnapshot> {
        self.layers
            .values()
            .enumerate()
            .map(|(index, transaction)| LayerSnapshot {
                surface: transaction.surface,
                window: None,
                namespace: None,
                stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
                geometry: transaction.target_geometry,
                source: transaction.target_buffer,
                damage: transaction.damage.clone(),
                opacity: 1.0,
                crop: None,
                transform: Transform::IDENTITY,
                generation: transaction.previous_committed_generation,
                resize_sync: ResizeSyncCapability::ImplicitOnly,
            })
            .collect()
    }

    fn drain_native_scanout(
        &mut self,
        native_scanout: &mut PersistentNativeScanout,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let deadline = Instant::now() + timeout;
        while self.runtime.rendered_primary_plane_scanout_in_flight() && Instant::now() < deadline {
            self.retire_native_scanout(native_scanout)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        if self
            .runtime
            .rendered_primary_plane_scanout_cleanup_pending()
        {
            let _ = self
                .runtime
                .retry_tracked_rendered_primary_plane_scanout_cleanup(
                    native_scanout.session.card(),
                );
        }
        Ok(())
    }

    fn run_native_idle(
        &mut self,
        native_scanout: &mut PersistentNativeScanout,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        native_scanout.run_tick(&mut self.runtime, compositor_tick_input(&transactions, 0))
    }

    fn retire_native_scanout(
        &mut self,
        native_scanout: &mut PersistentNativeScanout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        native_scanout.retire_ready(&mut self.runtime)
    }
}

fn compositor_tick_input(
    transactions: &[SurfaceTransaction],
    x_event_count: usize,
) -> CompositorBackendTickInput {
    CompositorBackendTickInput {
        x_event_count: u32::try_from(x_event_count).unwrap_or(u32::MAX),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layer_templates: super::x_authority::layer_templates_from_surface_transactions(
            transactions,
        ),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    }
}

struct PersistentNativeScanout {
    session: sophia_backend_live::RealAtomicScanoutPageFlipSession,
    exporter: sophia_backend_live::NativeGbmRenderedScanoutBufferDiscoveryExporter<
        sophia_backend_live::RealAtomicScanoutRenderDeviceDiscovery,
    >,
    sender: SyncSender<sophia_backend_live::LivePageFlipCallback>,
    receiver: Option<Receiver<sophia_backend_live::LivePageFlipCallback>>,
    output: sophia_engine::HeadlessOutput,
    submissions: usize,
    submit_deferred: usize,
    submit_failures: usize,
    retirements: usize,
    retire_failures: usize,
    max_in_flight_ticks: u64,
    submitted_at: Option<Instant>,
    max_submit_to_page_flip: Duration,
    callback_accepted: usize,
    callback_rejected: usize,
    callback_queue_saturated: usize,
    pending_nonzero_pixel_bytes: usize,
    nonzero_exports: usize,
}

impl PersistentNativeScanout {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = sophia_backend_live::RealAtomicScanoutSmokeConfig::default_primary_output()
            .ok_or("persistent native scanout config is invalid")?;
        let selection = sophia_backend_live::select_real_atomic_scanout_card();
        let size = selection
            .selection
            .map(|selection| selection.size())
            .ok_or("persistent native scanout could not select a KMS output")?;
        let mut session_result =
            selection.into_page_flip_session(config.slot, config.output, config.authority);
        let session = session_result
            .session
            .take()
            .ok_or("persistent native scanout could not open a page-flip session")?;
        let discovery = session.render_device_discovery()?;
        let exporter =
            sophia_backend_live::NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery)
                .with_preferred_modifiers(session.preferred_xrgb8888_scanout_modifiers());
        let (sender, receiver) = sync_channel(64);
        Ok(Self {
            session,
            exporter,
            sender,
            receiver: Some(receiver),
            output: sophia_engine::HeadlessOutput {
                id: config.output,
                size,
                scale: 1,
            },
            submissions: 0,
            submit_deferred: 0,
            submit_failures: 0,
            retirements: 0,
            retire_failures: 0,
            max_in_flight_ticks: 0,
            submitted_at: None,
            max_submit_to_page_flip: Duration::ZERO,
            callback_accepted: 0,
            callback_rejected: 0,
            callback_queue_saturated: 0,
            pending_nonzero_pixel_bytes: 0,
            nonzero_exports: 0,
        })
    }

    const fn output(&self) -> sophia_engine::HeadlessOutput {
        self.output
    }

    fn take_receiver(&mut self) -> Receiver<sophia_backend_live::LivePageFlipCallback> {
        self.receiver
            .take()
            .expect("native page-flip receiver must attach once")
    }

    fn run_tick(
        &mut self,
        runtime: &mut sophia_backend_live::LiveBackendRuntimeAssembly,
        input: CompositorBackendTickInput,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let export_attempts_before = self.exporter.cpu_frame_export_attempts();
        let report = self.session.run_native_gbm_runtime_tick(
            runtime,
            input,
            &mut self.exporter,
            &self.sender,
            64,
            64,
        )?;
        if self.exporter.cpu_frame_export_attempts() > export_attempts_before
            && self.pending_nonzero_pixel_bytes > 0
        {
            self.nonzero_exports = self.nonzero_exports.saturating_add(1);
        }
        if !self.exporter.pending_cpu_frame() {
            self.pending_nonzero_pixel_bytes = 0;
        }
        if let Some(retire) = report.tick.rendered_primary_plane_scanout_retire {
            self.observe_retire(retire);
        }
        if let Some(submit) = report.tick.rendered_primary_plane_scanout_submit {
            use sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
            match submit.status {
                Status::SubmittedWaitingForPageFlip => {
                    self.submissions = self.submissions.saturating_add(1);
                    self.submitted_at = Some(Instant::now());
                }
                Status::AlreadyInFlight | Status::CleanupPending => {
                    self.submit_deferred = self.submit_deferred.saturating_add(1);
                }
                _ => self.submit_failures = self.submit_failures.saturating_add(1),
            }
        }
        self.max_in_flight_ticks = self
            .max_in_flight_ticks
            .max(report.tick.rendered_primary_plane_scanout_in_flight_ticks);
        self.observe_callbacks(report.tick.page_flip_callbacks);
        Ok(report.tick)
    }

    fn retire_ready(
        &mut self,
        runtime: &mut sophia_backend_live::LiveBackendRuntimeAssembly,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = self
            .session
            .poll_native_page_flip_events(&self.sender, 64, 64);
        let report =
            runtime.drain_rendered_primary_plane_page_flip_callbacks_with(self.session.card());
        self.observe_callbacks(report.page_flip_callbacks);
        if let Some(retire) = report.rendered_primary_plane_scanout_retire {
            self.observe_retire(retire);
        }
        Ok(())
    }

    fn observe_retire(
        &mut self,
        retire: sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutRetireReport,
    ) {
        use sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus as Status;
        match retire.status {
            Status::RetiredAfterPageFlip => {
                self.retirements = self.retirements.saturating_add(1);
                if let Some(submitted_at) = self.submitted_at.take() {
                    self.max_submit_to_page_flip =
                        self.max_submit_to_page_flip.max(submitted_at.elapsed());
                }
            }
            Status::NoSubmission | Status::WaitingForAcceptedPageFlip => {}
            Status::ResourceRetireFailed => {
                self.retire_failures = self.retire_failures.saturating_add(1);
            }
        }
    }

    fn observe_callbacks(&mut self, report: sophia_backend_live::LivePageFlipCallbackQueueReport) {
        self.callback_accepted = self.callback_accepted.saturating_add(report.accepted);
        self.callback_rejected = self
            .callback_rejected
            .saturating_add(report.rejected_unexpected_output + report.rejected_stale_frame_serial);
        self.callback_queue_saturated = self
            .callback_queue_saturated
            .saturating_add(usize::from(report.max_reached));
    }

    fn initialize(
        &mut self,
        runtime: &mut sophia_backend_live::LiveBackendRuntimeAssembly,
        frame: sophia_backend_live::LiveCpuComposedFrame,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.queue_frame(frame);
        let export_attempts_before = self.exporter.cpu_frame_export_attempts();
        self.session
            .initialize_persistent_native_gbm_scanout(runtime, &mut self.exporter)
            .map_err(|evidence| {
                format!("persistent native initial modeset failed: {evidence:?}")
            })?;
        if self.exporter.cpu_frame_export_attempts() > export_attempts_before
            && self.pending_nonzero_pixel_bytes > 0
        {
            self.nonzero_exports = self.nonzero_exports.saturating_add(1);
        }
        if !self.exporter.pending_cpu_frame() {
            self.pending_nonzero_pixel_bytes = 0;
        }
        self.submissions = self.submissions.saturating_add(1);
        Ok(())
    }

    fn queue_frame(&mut self, frame: sophia_backend_live::LiveCpuComposedFrame) {
        self.pending_nonzero_pixel_bytes = frame.bytes.iter().filter(|byte| **byte != 0).count();
        self.exporter.set_pending_cpu_frame(frame);
    }
}

fn seed_committed_surfaces(transactions: &[SurfaceTransaction]) -> Vec<CommittedSurfaceState> {
    let mut surfaces = BTreeMap::new();
    for transaction in transactions {
        surfaces
            .entry(transaction.surface)
            .or_insert(CommittedSurfaceState {
                surface: transaction.surface,
                committed_generation: transaction.previous_committed_generation,
                geometry: transaction.target_geometry,
                buffer: transaction.target_buffer,
                damage: Region::empty(),
            });
    }
    surfaces.into_values().collect()
}

struct PersistentCpuScene {
    output_size: Size,
    buffers: BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
    surfaces: BTreeMap<SurfaceId, (Rect, u64)>,
    last_report: Option<sophia_backend_live::LiveCpuCompositionReport>,
    max_nonzero_pixel_bytes: usize,
    nonzero_frames: usize,
}

impl PersistentCpuScene {
    fn new(output_size: Size) -> Self {
        Self {
            output_size,
            buffers: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            last_report: None,
            max_nonzero_pixel_bytes: 0,
            nonzero_frames: 0,
        }
    }

    fn observe(&mut self, batch: &XAuthorityObservedTransactionBatch) {
        for buffer in &batch.cpu_buffers {
            let replace = self
                .buffers
                .get(&buffer.handle)
                .is_none_or(|current| buffer.generation >= current.generation);
            if replace {
                self.buffers.insert(buffer.handle, buffer.clone());
            }
        }
        for transaction in &batch.transactions {
            if let BufferSource::CpuBuffer { handle } = transaction.target_buffer {
                self.surfaces
                    .insert(transaction.surface, (transaction.target_geometry, handle));
            }
        }
    }

    fn compose(
        &mut self,
    ) -> Result<&sophia_backend_live::LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        let layers = self
            .surfaces
            .values()
            .filter_map(|(geometry, handle)| {
                let buffer = self.buffers.get(handle)?;
                Some(sophia_backend_live::LiveCpuCompositionLayer {
                    geometry: *geometry,
                    buffer: sophia_backend_live::LiveCpuBufferSource {
                        handle: buffer.handle,
                        size: buffer.size,
                        stride: buffer.stride,
                        format: buffer.format,
                        generation: buffer.generation,
                        bytes: buffer.bytes.clone(),
                    },
                })
            })
            .collect::<Vec<_>>();
        self.last_report = Some(
            sophia_backend_live::compose_live_cpu_frame(self.output_size, &layers)
                .map_err(|error| format!("persistent CPU composition failed: {error:?}"))?,
        );
        let nonzero_pixel_bytes = self
            .last_report
            .as_ref()
            .expect("assigned above")
            .nonzero_pixel_bytes;
        self.max_nonzero_pixel_bytes = self.max_nonzero_pixel_bytes.max(nonzero_pixel_bytes);
        self.nonzero_frames = self
            .nonzero_frames
            .saturating_add(usize::from(nonzero_pixel_bytes > 0));
        Ok(self.last_report.as_ref().expect("assigned above"))
    }

    fn buffer_checksum(&self) -> u64 {
        self.buffers
            .values()
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, buffer| {
                buffer.bytes.iter().fold(hash, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                })
            })
    }
}

fn send_test_text(
    sender: &SyncSender<XAuthorityInputEvent>,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut time_msec = 1u32;
    for keycode in text
        .bytes()
        .map(super::x_authority::x11_keycode_for_ascii)
        .chain(std::iter::once(Some(36)))
    {
        let keycode = keycode.ok_or("test input has no core X keycode")?;
        for pressed in [true, false] {
            sender.try_send(
                XAuthorityKeyEvent {
                    keycode,
                    pressed,
                    state: 0,
                    time_msec,
                }
                .into(),
            )?;
            time_msec = time_msec.saturating_add(1);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PhysicalInputRouteReport {
    events: usize,
    keys_routed: usize,
    pointer_events: usize,
    pointer_routed: usize,
}

fn route_physical_input(
    poller: &mut sophia_backend_live::NativeLibinputEventPoller<
        sophia_backend_live::NativeLibinputEventReader,
    >,
    focus: &InputFocusState,
    committed_surfaces: &[CommittedSurfaceState],
    input_layers: &[LayerSnapshot],
    input_sender: &SyncSender<XAuthorityInputEvent>,
    modifiers: &mut XCoreKeyboardMapper,
    pointer: &mut XCorePointerMapper,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let events = poller.poll_ready()?;
    let mut report = PhysicalInputRouteReport {
        events: events.len(),
        keys_routed: 0,
        pointer_events: 0,
        pointer_routed: 0,
    };
    for event in events {
        match event.kind {
            sophia_protocol::InputEventKind::Key { keycode, pressed } => {
                let FocusedInputRoute::Routed(event) =
                    focus.route_keyboard_event(event, committed_surfaces)
                else {
                    continue;
                };
                let Some((keycode, state)) = modifiers.map_evdev_key(keycode, pressed) else {
                    continue;
                };
                input_sender.try_send(
                    XAuthorityKeyEvent {
                        keycode,
                        pressed,
                        state,
                        time_msec: u32::try_from(event.time_msec).unwrap_or(u32::MAX),
                    }
                    .into(),
                )?;
                report.keys_routed = report.keys_routed.saturating_add(1);
            }
            kind @ (sophia_protocol::InputEventKind::PointerMotion
            | sophia_protocol::InputEventKind::PointerButton { .. }) => {
                report.pointer_events = report.pointer_events.saturating_add(1);
                let route = sophia_engine::hit_test_scene_surface_for_input(&event, input_layers);
                if route.target_surface != focus.focused_surface(event.seat) {
                    continue;
                }
                let (Some(global), Some(local)) = (event.global_position, route.local_position)
                else {
                    continue;
                };
                let Some(surface) = route.target_surface else {
                    continue;
                };
                let (event_kind, state) = match kind {
                    sophia_protocol::InputEventKind::PointerMotion => {
                        (XAuthorityPointerEventKind::Motion, pointer.state())
                    }
                    sophia_protocol::InputEventKind::PointerButton { button, pressed } => {
                        let Some((button, state)) = pointer.map_evdev_button(button, pressed)
                        else {
                            continue;
                        };
                        (
                            XAuthorityPointerEventKind::Button { button, pressed },
                            state,
                        )
                    }
                    sophia_protocol::InputEventKind::Key { .. } => unreachable!(),
                };
                input_sender.try_send(XAuthorityInputEvent::Pointer(XAuthorityPointerEvent {
                    kind: event_kind,
                    surface,
                    root_x: pointer_coordinate(global.x),
                    root_y: pointer_coordinate(global.y),
                    event_x: pointer_coordinate(local.x),
                    event_y: pointer_coordinate(local.y),
                    state,
                    time_msec: u32::try_from(event.time_msec).unwrap_or(u32::MAX),
                }))?;
                report.pointer_routed = report.pointer_routed.saturating_add(1);
            }
        }
    }
    Ok(report)
}

fn pointer_coordinate(value: f64) -> i16 {
    if !value.is_finite() {
        return 0;
    }
    value
        .round()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

struct SessionProcessGuard {
    child: Option<Child>,
    socket_path: std::path::PathBuf,
}

impl SessionProcessGuard {
    fn new(child: Child, socket_path: std::path::PathBuf) -> Self {
        Self {
            child: Some(child),
            socket_path,
        }
    }

    fn child_mut(&mut self) -> Result<&mut Child, Box<dyn std::error::Error>> {
        self.child
            .as_mut()
            .ok_or_else(|| "xterm child missing".into())
    }

    fn terminate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        match std::fs::remove_file(&self.socket_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
}

impl Drop for SessionProcessGuard {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

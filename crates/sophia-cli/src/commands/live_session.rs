use super::prelude::*;

use std::collections::BTreeMap;
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};

const SESSION_AUTHORITY_CAPACITY: usize = 256;
const SESSION_KEY_CAPACITY: usize = 64;
const SESSION_INPUT_QUIET_MSEC: u64 = 100;

pub(crate) fn run_persistent_xterm_session(
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = PersistentXtermSessionConfig::from_args(args)?;
    let terminal = super::x_authority::resolve_external_probe_binary("xterm", &config.terminal)?;
    prepare_display_socket(&config.socket_path)?;

    let server_path = config.socket_path.clone();
    let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
    let (key_sender, key_receiver) = sync_channel(SESSION_KEY_CAPACITY);
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_channels(
            &server_path,
            NamespaceId::from_raw(50),
            authority_sender,
            key_receiver,
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
    if config.inject_text.is_some() {
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
        "sophia_live_session schema=1 status=running display={} terminal=xterm runtime=persistent authority_capacity={} key_capacity={} native_presentation=pending physical_input=pending",
        config.display, SESSION_AUTHORITY_CAPACITY, SESSION_KEY_CAPACITY,
    );

    let result = run_session_loop(
        &config,
        &authority_receiver,
        &key_sender,
        process.child_mut()?,
    );
    process.terminate()?;
    drop(key_sender);
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
    inject_text: Option<String>,
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
        let inject_text = arg_value(args, "--inject-text");
        if inject_text.is_some() && max_runtime.is_none() {
            return Err("--inject-text requires --max-runtime-ms for a bounded proof".into());
        }
        if let Some(text) = &inject_text
            && (text.is_empty()
                || text.len() > 24
                || !text.bytes().all(|byte| byte.is_ascii_lowercase()))
        {
            return Err("--inject-text accepts 1-24 lowercase ASCII letters".into());
        }
        Ok(Self {
            display,
            socket_path: std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}")),
            terminal: arg_value(args, "--terminal").unwrap_or_else(|| "xterm".to_owned()),
            max_runtime,
            inject_text,
        })
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
    key_sender: &SyncSender<XAuthorityKeyEvent>,
    child: &mut Child,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let deadline = config.max_runtime.map(|duration| started + duration);
    let output = sophia_engine::HeadlessOutput::deterministic();
    let mut scene = PersistentCpuScene::new(output.size);
    let mut runtime = None;
    let mut last_authority_update = started;
    let mut injection_checksum = None;
    let mut input_pixel_change = false;
    let mut batches = 0usize;
    let mut transactions = 0usize;
    let mut backend_ticks = 0usize;
    let mut runtime_committed = 0u64;
    let mut runtime_surfaces = 0u64;

    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!("xterm exited during live session with status {status}").into());
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }

        match authority_receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(batch) => {
                last_authority_update = Instant::now();
                batches = batches.saturating_add(1);
                transactions = transactions.saturating_add(batch.transactions.len());
                scene.observe(&batch);
                let report = scene.compose()?;
                if let Some((before_frame, _)) = injection_checksum
                    && report.checksum != before_frame
                {
                    input_pixel_change = true;
                }

                let runtime = runtime.get_or_insert_with(|| {
                    PersistentBackendRuntime::new(output, &batch.transactions)
                });
                let tick = runtime.run_batch(&batch)?;
                backend_ticks = backend_ticks.saturating_add(1);
                runtime_committed = tick
                    .engine
                    .runtime
                    .runtime_state
                    .authority_transactions_committed;
                runtime_surfaces = tick.engine.runtime.runtime_state.authority_surfaces_applied;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("persistent X authority transaction channel disconnected".into());
            }
        }

        if injection_checksum.is_none()
            && config.inject_text.is_some()
            && scene.last_report.is_some()
            && last_authority_update.elapsed() >= Duration::from_millis(SESSION_INPUT_QUIET_MSEC)
        {
            injection_checksum = scene
                .last_report
                .as_ref()
                .map(|report| (report.checksum, scene.buffer_checksum()));
            send_test_text(
                key_sender,
                config.inject_text.as_deref().expect("checked above"),
            )?;
        }
    }

    let report = scene
        .last_report
        .as_ref()
        .ok_or("persistent live session received no composable X pixels")?;
    if config.inject_text.is_some() && !input_pixel_change {
        return Err(format!(
            "persistent live session input did not change composed terminal pixels: baseline={injection_checksum:?} final_frame={} final_buffers={} batches={batches} transactions={transactions}",
            report.checksum,
            scene.buffer_checksum(),
        )
        .into());
    }
    println!(
        "sophia_live_session schema=1 status=bounded_complete display={} elapsed_msec={} authority_batches={} authority_transactions={} backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_checksum={} injected_input={} input_pixel_change={} native_presentation=pending physical_input=pending",
        config.display,
        started.elapsed().as_millis(),
        batches,
        transactions,
        backend_ticks,
        runtime_committed,
        runtime_surfaces,
        report.layers_composed,
        report.nonzero_pixel_bytes,
        report.checksum,
        config.inject_text.is_some(),
        input_pixel_change,
    );
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
    ) -> Self {
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
        Self {
            runtime: sophia_backend_live::LiveBackendRuntimeAssembly::from_ready_headless_scanout(
                assembly, output, renderer,
            ),
            authority_sender,
            layers: BTreeMap::new(),
        }
    }

    fn run_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
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
        self.runtime
            .run_tick(CompositorBackendTickInput {
                x_event_count: u32::try_from(batch.transactions.len()).unwrap_or(u32::MAX),
                authority_batches: Vec::new(),
                wm_update: None,
                portal_commands: Vec::new(),
                chrome_command_count: 0,
                layer_templates: super::x_authority::layer_templates_from_surface_transactions(
                    &transactions,
                ),
                scanout_submit_state: None,
                scanout_lifecycle_states: Vec::new(),
            })
            .map_err(Into::into)
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
}

impl PersistentCpuScene {
    fn new(output_size: Size) -> Self {
        Self {
            output_size,
            buffers: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            last_report: None,
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
    sender: &SyncSender<XAuthorityKeyEvent>,
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
            sender.try_send(XAuthorityKeyEvent {
                keycode,
                pressed,
                state: 0,
                time_msec,
            })?;
            time_msec = time_msec.saturating_add(1);
        }
    }
    Ok(())
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

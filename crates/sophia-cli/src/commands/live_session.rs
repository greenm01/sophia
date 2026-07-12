use super::prelude::*;

use sophia_cli::input_proof::{PhysicalTextProof, PhysicalTextProofEvent};
use sophia_engine::{
    FocusedInputRoute, InputFocusDecision, InputFocusState, NonBlockingInputPoller,
};
use sophia_protocol::{DeviceId, OutputId, SeatId, WmManageSurface};
use sophia_x_authority::{
    XAuthorityControlAck, XAuthorityControlCommand, XAuthorityControlOutcome, XAuthorityInputEvent,
    XAuthorityPointerEvent, XAuthorityPointerEventKind, XCoreKeyboardMapper, XCorePointerMapper,
    run_x11_core_socket_server_once_session_channels,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};

const SESSION_AUTHORITY_CAPACITY: usize = 256;
const SESSION_KEY_CAPACITY: usize = 64;
const SESSION_CONTROL_CAPACITY: usize = 32;
const SESSION_INPUT_QUIET_MSEC: u64 = 500;
const SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC: u64 = 15_000;
const SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC: u64 = 5_000;
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
    let mut wm_session = LiveWmSession::from_config(&config)?;

    let server_path = config.socket_path.clone();
    let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
    let (input_sender, input_receiver) = sync_channel(SESSION_KEY_CAPACITY);
    let (control_sender, control_receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (control_ack_sender, control_ack_receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_session_channels(
            &server_path,
            NamespaceId::from_raw(50),
            authority_sender,
            input_receiver,
            control_receiver,
            control_ack_sender,
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
            "120x36+80+60",
            "-title",
            "Sophia Terminal",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(proof_text) = config
        .inject_text
        .as_deref()
        .or(config.expect_physical_text.as_deref())
    {
        terminal_command
            .args([
                "-e",
                "sh",
                "-c",
                "printf 'type %s then Return: ' \"$1\"; IFS= read -r line; printf '\\nreceived:%s\\n' \"$line\"; sleep 5",
                "sophia-input-proof",
            ])
            .arg(proof_text);
    } else if let Some(program) = config.terminal_exec.as_deref() {
        terminal_command
            .env_remove("ENV")
            .env_remove("BASH_ENV")
            .arg("-e")
            .arg(program)
            .args(&config.terminal_exec_args);
    }
    let child = terminal_command.spawn()?;
    let mut process = SessionProcessGuard::new(child, config.socket_path.clone());

    println!(
        "sophia_live_session schema=7 status=running display={} terminal=xterm runtime=persistent authority_capacity={} input_capacity={} control_capacity={} native_presentation={} physical_input={} pointer_proof={} wm_policy={}",
        config.display,
        SESSION_AUTHORITY_CAPACITY,
        SESSION_KEY_CAPACITY,
        SESSION_CONTROL_CAPACITY,
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
        if wm_session.is_some() {
            "external"
        } else {
            "disabled"
        },
    );
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_outputs schema=2 status=ready discovered={} presentation={} native_owned={} multi_output_scanout=enabled layout=extended_horizontal",
            native_scanout.discovered_outputs,
            native_scanout.presentation_outputs,
            native_scanout.heads.len(),
        );
    }

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
    terminal_exec: Option<String>,
    terminal_exec_args: Vec<String>,
    max_runtime: Option<Duration>,
    max_ticks: Option<usize>,
    inject_text: Option<String>,
    expect_physical_text: Option<String>,
    expect_physical_pointer: bool,
    exit_after_input_proof: bool,
    input_devices: Vec<std::path::PathBuf>,
    native_scanout: bool,
    wm_process: Option<String>,
    wm_process_args: Vec<String>,
    wm_socket_path: std::path::PathBuf,
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
        let terminal_exec = arg_value(args, "--terminal-exec");
        let terminal_exec_args = args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--terminal-exec-arg="))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if terminal_exec.is_none() && !terminal_exec_args.is_empty() {
            return Err("--terminal-exec-arg requires --terminal-exec".into());
        }
        if terminal_exec_args.len() > 32
            || terminal_exec_args
                .iter()
                .any(|argument| argument.len() > 4_096)
        {
            return Err("--terminal-exec accepts at most 32 bounded arguments".into());
        }
        let expect_physical_pointer = args.iter().any(|arg| arg == "--expect-physical-pointer");
        let exit_after_input_proof = args.iter().any(|arg| arg == "--exit-after-input-proof");
        let native_scanout = args.iter().any(|arg| arg == "--native-scanout");
        let wm_process = arg_value(args, "--wm-process");
        let wm_process_args = args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--wm-process-arg="))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if wm_process.is_none() && !wm_process_args.is_empty() {
            return Err("--wm-process-arg requires --wm-process".into());
        }
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
        if terminal_exec.is_some() && (inject_text.is_some() || expect_physical_text.is_some()) {
            return Err("--terminal-exec cannot be combined with input-proof commands".into());
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
        if exit_after_input_proof && inject_text.is_none() && expect_physical_text.is_none() {
            return Err("--exit-after-input-proof requires an input proof".into());
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
            terminal_exec,
            terminal_exec_args,
            max_runtime,
            max_ticks,
            inject_text,
            expect_physical_text,
            expect_physical_pointer,
            exit_after_input_proof,
            input_devices,
            native_scanout,
            wm_process,
            wm_process_args,
            wm_socket_path: std::env::temp_dir().join(format!(
                "sophia-live-wm-{}-{display_number}.sock",
                std::process::id()
            )),
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

struct LiveWmSession {
    supervisor: ProcessSupervisor,
    supervisor_state: sophia_runtime::SupervisorState,
    restart_policy: RestartPolicy,
    socket_path: std::path::PathBuf,
    transport: Option<WmSocketTransport>,
    next_transaction: u64,
    requests: usize,
    committed: usize,
    last_committed_at: Option<Instant>,
    restarts: usize,
    degraded: bool,
}

struct LiveWmProposal {
    transaction: TransactionId,
    layers: Vec<LayerSnapshot>,
    requested_sizes: BTreeMap<SurfaceId, Size>,
    focus: Option<SurfaceId>,
    timeout: Duration,
    update: WmTransactionUpdate,
    moved_surfaces: usize,
}

impl LiveWmSession {
    fn from_config(
        config: &PersistentXtermSessionConfig,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Some(process) = config.wm_process.as_deref() else {
            return Ok(None);
        };
        let _ = std::fs::remove_file(&config.wm_socket_path);
        let socket_arg = format!("--socket={}", config.wm_socket_path.display());
        let spec = config.wm_process_args.iter().fold(
            ProcessLaunchSpec::new(process)
                .arg("serve-socket")
                .arg(socket_arg),
            |spec, argument| spec.arg(argument),
        );
        let mut session = Self {
            supervisor: ProcessSupervisor::new(SupervisedProcessKind::WindowManager, spec),
            supervisor_state: sophia_runtime::SupervisorState::new(
                SupervisedProcessKind::WindowManager,
            ),
            restart_policy: RestartPolicy::default(),
            socket_path: config.wm_socket_path.clone(),
            transport: None,
            next_transaction: 1,
            requests: 0,
            committed: 0,
            last_committed_at: None,
            restarts: 0,
            degraded: false,
        };
        session.start(SupervisorEvent::StartRequested)?;
        println!("sophia_live_wm schema=1 status=ready adapter=external socket=private restarts=0");
        Ok(Some(session))
    }

    fn start(&mut self, event: SupervisorEvent) -> Result<(), Box<dyn std::error::Error>> {
        let (state, command) =
            update_supervisor(self.supervisor_state.clone(), event, self.restart_policy);
        self.supervisor_state = state;
        let start_event = self
            .supervisor
            .apply(command)?
            .ok_or("WM supervisor did not start the configured process")?;
        let (state, _) = update_supervisor(
            self.supervisor_state.clone(),
            start_event,
            self.restart_policy,
        );
        self.supervisor_state = state;
        super::x_authority::wait_for_socket_path(&self.socket_path)?;
        let stream = UnixStream::connect(&self.socket_path)?;
        self.transport = Some(WmSocketTransport::new(
            stream,
            WmSocketTransportConfig {
                response_timeout: Duration::from_millis(500),
            },
        ));
        let (state, _) = update_supervisor(
            self.supervisor_state.clone(),
            SupervisorEvent::ProcessHealthy,
            self.restart_policy,
        );
        self.supervisor_state = state;
        Ok(())
    }

    fn poll_restart(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<Option<LiveWmProposal>, Box<dyn std::error::Error>> {
        if self.degraded || self.supervisor.poll()?.is_none() {
            return Ok(None);
        }
        self.transport = None;
        self.restarts = self.restarts.saturating_add(1);
        if let Err(error) = self.start(SupervisorEvent::ProcessExited) {
            if self.committed == 0 {
                return Err(error);
            }
            self.degraded = true;
            println!(
                "sophia_live_wm schema=1 status=degraded reason=restart_failed preserved_layout=true"
            );
            return Ok(None);
        }
        println!(
            "sophia_live_wm schema=1 status=restarted restarts={} preserved_layout=true",
            self.restarts
        );
        if layout.layers.is_empty() {
            Ok(None)
        } else {
            self.request_relayout(layout, output).map(Some)
        }
    }

    fn request_manage(
        &mut self,
        surface: SurfaceId,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let node = layout
            .layers
            .get(&surface)
            .ok_or("new WM surface is missing from live layout")?;
        let workspace = WorkspaceId::from_raw(1);
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::ManageSurface(WmManageSurface {
                node: live_layout_node(node, workspace),
                output: output.id,
                workspace,
                bounds: output_bounds(output),
            }),
        };
        self.request(request, layout, output)
    }

    fn request_relayout(
        &mut self,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let workspace = WorkspaceId::from_raw(1);
        let request = WmRequestPacket {
            transaction: self.mint_transaction()?,
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: output.id,
                workspace,
                bounds: output_bounds(output),
                nodes: layout
                    .layers
                    .values()
                    .map(|layer| live_layout_node(layer, workspace))
                    .collect(),
            }),
        };
        self.request(request, layout, output)
    }

    fn request(
        &mut self,
        request: WmRequestPacket,
        layout: &PersistentLiveLayout,
        output: sophia_engine::HeadlessOutput,
    ) -> Result<LiveWmProposal, Box<dyn std::error::Error>> {
        let response = self
            .transport
            .as_mut()
            .ok_or("WM transport is unavailable")?
            .request(&request)?;
        self.requests = self.requests.saturating_add(1);
        if response.commands.len() > 8_192 {
            return Err("WM response exceeds the live command limit".into());
        }
        let transaction = response.into_layout_transaction();
        validate_live_wm_transaction(&transaction, layout, output_bounds(output))?;
        let mut proposed = layout.layers.values().cloned().collect::<Vec<_>>();
        let engine = HeadlessEngine::new(output);
        let commit = engine.commit_layout_transaction(&transaction, &mut proposed);
        if commit.outcome != TransactionOutcome::Committed {
            return Err(format!("Engine rejected live WM proposal: {:?}", commit.outcome).into());
        }
        let requested_sizes = transaction
            .requested_sizes
            .iter()
            .map(|request| (request.surface, request.size))
            .collect();
        let moved_surfaces = proposed
            .iter()
            .filter(|layer| {
                layout
                    .layers
                    .get(&layer.surface)
                    .is_some_and(|current| current.geometry != layer.geometry)
            })
            .count();
        let timeout = Duration::from_millis(u64::from(transaction.timeout_msec.clamp(100, 2_000)));
        Ok(LiveWmProposal {
            transaction: transaction.transaction,
            layers: proposed,
            requested_sizes,
            focus: transaction.focus,
            timeout,
            update: WmTransactionUpdate {
                commit,
                ipc_error: None,
            },
            moved_surfaces,
        })
    }

    fn mint_transaction(&mut self) -> Result<TransactionId, Box<dyn std::error::Error>> {
        let transaction = TransactionId::from_raw(self.next_transaction);
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or("WM transaction ID space exhausted")?;
        Ok(transaction)
    }

    fn mark_committed(&mut self) {
        self.committed = self.committed.saturating_add(1);
        self.last_committed_at = Some(Instant::now());
    }
}

impl Drop for LiveWmSession {
    fn drop(&mut self) {
        let _ = self.supervisor.terminate();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

struct PendingLiveWmLayout {
    transaction: TransactionId,
    layers: Vec<LayerSnapshot>,
    requested_sizes: BTreeMap<SurfaceId, Size>,
    focus: Option<SurfaceId>,
    deadline: Instant,
    update: WmTransactionUpdate,
    moved_surfaces: usize,
}

#[derive(Default)]
struct PersistentLiveLayout {
    layers: BTreeMap<SurfaceId, LayerSnapshot>,
    authority_sizes: BTreeMap<SurfaceId, Size>,
    unmanaged_surfaces: BTreeSet<SurfaceId>,
    pending: Option<PendingLiveWmLayout>,
    focus_to_apply: Option<(TransactionId, SurfaceId)>,
    stage_new_surfaces_offset: bool,
}

impl PersistentLiveLayout {
    fn new(stage_new_surfaces_offset: bool) -> Self {
        Self {
            stage_new_surfaces_offset,
            ..Self::default()
        }
    }

    fn observe_authority_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Vec<SurfaceId> {
        let mut new_surfaces = Vec::new();
        for (index, transaction) in batch.transactions.iter().enumerate() {
            let size = Size {
                width: transaction.target_geometry.width,
                height: transaction.target_geometry.height,
            };
            self.authority_sizes.insert(transaction.surface, size);
            match self.layers.get_mut(&transaction.surface) {
                Some(layer) => {
                    layer.source = transaction.target_buffer;
                    layer.damage = transaction.damage.clone();
                    layer.generation = transaction.previous_committed_generation.saturating_add(1);
                }
                None => {
                    new_surfaces.push(transaction.surface);
                    self.unmanaged_surfaces.insert(transaction.surface);
                    let mut geometry = transaction.target_geometry;
                    if self.stage_new_surfaces_offset {
                        geometry.x = geometry.x.saturating_add(80);
                        geometry.y = geometry.y.saturating_add(60);
                    }
                    self.layers.insert(
                        transaction.surface,
                        LayerSnapshot {
                            surface: transaction.surface,
                            window: None,
                            namespace: None,
                            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
                            geometry,
                            source: transaction.target_buffer,
                            damage: transaction.damage.clone(),
                            opacity: 1.0,
                            crop: None,
                            transform: Transform::IDENTITY,
                            generation: transaction.previous_committed_generation.saturating_add(1),
                            resize_sync: ResizeSyncCapability::ImplicitOnly,
                        },
                    );
                }
            }
        }
        new_surfaces
    }

    fn take_unmanaged_surfaces(&mut self) -> Vec<SurfaceId> {
        std::mem::take(&mut self.unmanaged_surfaces)
            .into_iter()
            .collect()
    }

    fn stage(
        &mut self,
        mut proposal: LiveWmProposal,
        control_sender: &SyncSender<XAuthorityControlCommand>,
        control_ack_receiver: &Receiver<XAuthorityControlAck>,
    ) -> Result<Option<WmTransactionUpdate>, Box<dyn std::error::Error>> {
        proposal
            .requested_sizes
            .retain(|surface, size| self.authority_sizes.get(surface) != Some(size));
        for (surface, size) in &proposal.requested_sizes {
            control_sender.try_send(XAuthorityControlCommand::ConfigureSurface {
                transaction: proposal.transaction,
                surface: *surface,
                size: *size,
            })?;
        }
        for _ in 0..proposal.requested_sizes.len() {
            let acknowledgement = control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
            if acknowledgement.transaction != proposal.transaction
                || acknowledgement.outcome != XAuthorityControlOutcome::Applied
            {
                return Err(format!(
                    "X Authority rejected WM configure transaction {} for surface {:?}: {:?}",
                    acknowledgement.transaction.raw(),
                    acknowledgement.surface,
                    acknowledgement.outcome
                )
                .into());
            }
        }
        let ready = proposal
            .requested_sizes
            .iter()
            .all(|(surface, size)| self.authority_sizes.get(surface) == Some(size));
        if ready {
            return Ok(Some(self.commit_proposal(proposal)));
        }
        self.pending = Some(PendingLiveWmLayout {
            transaction: proposal.transaction,
            layers: proposal.layers,
            requested_sizes: proposal.requested_sizes,
            focus: proposal.focus,
            deadline: Instant::now() + proposal.timeout,
            update: proposal.update,
            moved_surfaces: proposal.moved_surfaces,
        });
        Ok(None)
    }

    fn resolve_pending(&mut self) -> Option<WmTransactionUpdate> {
        let pending = self.pending.as_ref()?;
        let ready = pending
            .requested_sizes
            .iter()
            .all(|(surface, size)| self.authority_sizes.get(surface) == Some(size));
        if !ready {
            return None;
        }
        let pending = self.pending.take().expect("checked above");
        Some(self.commit_pending(pending))
    }

    fn expire_pending(&mut self) -> Option<WmTransactionUpdate> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.deadline)
        {
            return None;
        }
        let pending = self.pending.take().expect("checked above");
        let resize_state = pending
            .requested_sizes
            .iter()
            .map(|(surface, expected)| {
                let observed = self.authority_sizes.get(surface).copied().unwrap_or(Size {
                    width: 0,
                    height: 0,
                });
                format!(
                    "{}x{}:{}x{}",
                    expected.width, expected.height, observed.width, observed.height
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "sophia_live_wm schema=1 status=layout_timeout transaction={} preserved_layout=true resize_state={}",
            pending.transaction.raw(),
            resize_state,
        );
        Some(WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: pending.transaction,
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
            ipc_error: None,
        })
    }

    fn commit_proposal(&mut self, proposal: LiveWmProposal) -> WmTransactionUpdate {
        let pending = PendingLiveWmLayout {
            transaction: proposal.transaction,
            layers: proposal.layers,
            requested_sizes: proposal.requested_sizes,
            focus: proposal.focus,
            deadline: Instant::now(),
            update: proposal.update,
            moved_surfaces: proposal.moved_surfaces,
        };
        self.commit_pending(pending)
    }

    fn commit_pending(&mut self, pending: PendingLiveWmLayout) -> WmTransactionUpdate {
        self.layers = pending
            .layers
            .into_iter()
            .map(|layer| (layer.surface, layer))
            .collect();
        if let Some(surface) = pending.focus {
            self.focus_to_apply = Some((pending.transaction, surface));
        }
        println!(
            "sophia_live_wm schema=1 status=layout_committed transaction={} surfaces={} moved_surfaces={} configure_acks={} outcome={:?}",
            pending.transaction.raw(),
            self.layers.len(),
            pending.moved_surfaces,
            pending.requested_sizes.len(),
            pending.update.commit.outcome
        );
        pending.update
    }

    fn projected_batch(
        &self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> XAuthorityObservedTransactionBatch {
        let mut projected = batch.clone();
        for transaction in &mut projected.transactions {
            if let Some(layer) = self.layers.get(&transaction.surface) {
                transaction.target_geometry = layer.geometry;
            }
        }
        projected
    }
}

fn output_bounds(output: sophia_engine::HeadlessOutput) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: output.size.width,
        height: output.size.height,
    }
}

fn live_layout_node(layer: &LayerSnapshot, workspace: WorkspaceId) -> LayoutNodeSnapshot {
    LayoutNodeSnapshot {
        surface: layer.surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry: layer.geometry,
        generation: layer.generation,
    }
}

fn validate_live_wm_transaction(
    transaction: &sophia_protocol::LayoutTransaction,
    layout: &PersistentLiveLayout,
    bounds: Rect,
) -> Result<(), Box<dyn std::error::Error>> {
    for placement in &transaction.render_positions {
        if !layout.layers.contains_key(&placement.surface)
            || placement.geometry.is_empty()
            || !rect_is_within(bounds, placement.geometry)
        {
            return Err("live WM returned an unknown, empty, or out-of-bounds placement".into());
        }
    }
    for request in &transaction.requested_sizes {
        if !layout.layers.contains_key(&request.surface)
            || request.size.width <= 0
            || request.size.height <= 0
            || request.size.width > i32::from(u16::MAX)
            || request.size.height > i32::from(u16::MAX)
        {
            return Err("live WM returned an invalid surface size request".into());
        }
    }
    if transaction
        .focus
        .is_some_and(|surface| !layout.layers.contains_key(&surface))
    {
        return Err("live WM returned an unknown focus surface".into());
    }
    Ok(())
}

fn rect_is_within(bounds: Rect, geometry: Rect) -> bool {
    let Some(bounds_right) = bounds.x.checked_add(bounds.width) else {
        return false;
    };
    let Some(bounds_bottom) = bounds.y.checked_add(bounds.height) else {
        return false;
    };
    let Some(right) = geometry.x.checked_add(geometry.width) else {
        return false;
    };
    let Some(bottom) = geometry.y.checked_add(geometry.height) else {
        return false;
    };
    geometry.x >= bounds.x
        && geometry.y >= bounds.y
        && right <= bounds_right
        && bottom <= bounds_bottom
}

fn run_session_loop(
    config: &PersistentXtermSessionConfig,
    authority_receiver: &Receiver<XAuthorityObservedTransactionBatch>,
    input_sender: &SyncSender<XAuthorityInputEvent>,
    control_sender: &SyncSender<XAuthorityControlCommand>,
    control_ack_receiver: &Receiver<XAuthorityControlAck>,
    child: &mut Child,
    physical_input: &mut Option<
        sophia_backend_live::NativeLibinputEventPoller<
            sophia_backend_live::NativeLibinputEventReader,
        >,
    >,
    native_scanout: &mut Option<PersistentNativeScanout>,
    wm_session: &mut Option<LiveWmSession>,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let deadline = config.max_runtime.map(|duration| started + duration);
    let outputs = native_scanout
        .as_ref()
        .map(PersistentNativeScanout::outputs)
        .unwrap_or_else(|| vec![sophia_engine::HeadlessOutput::deterministic()]);
    let output = outputs[0];
    let mut scene = PersistentCpuScene::new(output.size);
    let mut layout = PersistentLiveLayout::new(wm_session.is_some());
    let mut runtime = None;
    let mut last_authority_update = started;
    let mut injection_checksum = None;
    let mut physical_input_ready_at: Option<Instant> = None;
    let mut physical_text_proof = config
        .expect_physical_text
        .as_deref()
        .map(PhysicalTextProof::new)
        .transpose()?;
    let mut physical_sequence_completed_at: Option<Instant> = None;
    let mut physical_input_completion_reported = false;
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
            if status.success() {
                break;
            }
            return Err(format!("xterm exited during live session with status {status}").into());
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }
        let physical_sequence_complete = physical_text_proof
            .as_ref()
            .is_none_or(PhysicalTextProof::is_complete);
        let waiting_for_keyboard_sequence =
            physical_input_ready_at.is_some() && !physical_sequence_complete;
        let waiting_for_keyboard_pixels = physical_sequence_complete
            && physical_sequence_completed_at.is_some()
            && !input_pixel_change;
        let waiting_for_pointer_pixels = config.expect_physical_pointer
            && physical_sequence_complete
            && input_pixel_change
            && !pointer_pixel_change;
        if waiting_for_pointer_pixels && pointer_phase_started_at.is_none() {
            pointer_phase_started_at = Some(Instant::now());
        }
        if waiting_for_keyboard_sequence {
            let ready_at = physical_input_ready_at.expect("checked above");
            if ready_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC) {
                let proof = physical_text_proof.as_ref().expect("checked above");
                return Err(format!(
                    "persistent live session timed out waiting for exact physical input sequence: matched_events={} expected_events={} keyboard_routed={physical_keys_routed}",
                    proof.matched_events(),
                    proof.expected_events(),
                )
                .into());
            }
        } else if waiting_for_keyboard_pixels {
            let completed_at = physical_sequence_completed_at.expect("checked above");
            if completed_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC)
            {
                return Err(format!(
                    "persistent live session timed out waiting for pixels after exact physical input: keyboard_routed={physical_keys_routed} final_checksum={:?}",
                    scene.last_report.as_ref().map(|report| report.checksum)
                )
                .into());
            }
        } else if waiting_for_pointer_pixels {
            let started_at = pointer_phase_started_at.expect("set above");
            if started_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC) {
                return Err(format!(
                    "persistent live session timed out waiting for physical pointer pixels: pointer_observed={physical_pointer_events} pointer_routed={physical_pointer_routed} pointer_baseline={pointer_checksum:?} final_checksum={:?}",
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
                let _ = layout.observe_authority_batch(&batch);
                let mut wm_update = layout.resolve_pending().or_else(|| layout.expire_pending());
                if wm_update
                    .as_ref()
                    .is_some_and(|update| update.commit.outcome == TransactionOutcome::Committed)
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    wm_session.mark_committed();
                }
                if let Some(wm_session) = wm_session.as_mut() {
                    if let Some(proposal) = wm_session.poll_restart(&layout, output)? {
                        wm_update = layout.stage(proposal, control_sender, control_ack_receiver)?;
                    }
                }
                let batch = layout.projected_batch(&batch);
                scene.observe(&batch)?;
                let report = scene.compose()?.clone();
                let native_frames = native_scanout
                    .as_ref()
                    .map(|_| scene.frames_for_outputs(&outputs))
                    .transpose()?;
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
                        &outputs,
                        &batch.transactions,
                        native_scanout.as_mut(),
                        native_frames.clone(),
                    )?);
                }
                let runtime = runtime
                    .as_mut()
                    .expect("persistent backend runtime was initialized above");
                let tick =
                    runtime.run_batch(&batch, native_scanout.as_mut(), native_frames, wm_update)?;
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
                if let Some((transaction, surface)) = layout.focus_to_apply.take()
                    && focus.focus_surface(seat, surface, runtime.committed_surfaces())
                        == InputFocusDecision::Focused
                {
                    println!(
                        "sophia_live_wm schema=1 status=focus_committed transaction={} target=surface",
                        transaction.raw()
                    );
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let _ = layout.expire_pending();
                if let Some(wm_session) = wm_session.as_mut()
                    && let Some(proposal) = wm_session.poll_restart(&layout, output)?
                {
                    let _ = layout.stage(proposal, control_sender, control_ack_receiver)?;
                }
                if layout.pending.is_none()
                    && last_authority_update.elapsed()
                        >= Duration::from_millis(SESSION_INPUT_QUIET_MSEC)
                    && let Some(wm_session) = wm_session.as_mut()
                {
                    for surface in layout.take_unmanaged_surfaces() {
                        let proposal = wm_session.request_manage(surface, &layout, output)?;
                        if layout
                            .stage(proposal, control_sender, control_ack_receiver)?
                            .is_some()
                        {
                            wm_session.mark_committed();
                        }
                    }
                }
                if let (Some(runtime), Some(native_scanout)) =
                    (runtime.as_mut(), native_scanout.as_mut())
                {
                    if runtime.native_scanout_in_flight() {
                        runtime.retire_native_scanout(native_scanout)?;
                    }
                    if !runtime.native_scanout_in_flight()
                        && native_scanout
                            .heads
                            .iter()
                            .any(|head| head.exporter.pending_cpu_frame())
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

        if let (Some(poller), Some(runtime)) = (physical_input.as_mut(), runtime.as_ref())
            && (config.expect_physical_text.is_none() || physical_input_ready_at.is_some())
        {
            let report = route_physical_input(
                poller,
                &focus,
                runtime.committed_surfaces(),
                &runtime.input_layers(),
                input_sender,
                &mut modifiers,
                &mut pointer,
                physical_text_proof.as_mut(),
            )?;
            physical_events = physical_events.saturating_add(report.events);
            physical_keys_routed = physical_keys_routed.saturating_add(report.keys_routed);
            physical_pointer_events = physical_pointer_events.saturating_add(report.pointer_events);
            physical_pointer_routed = physical_pointer_routed.saturating_add(report.pointer_routed);
            if physical_sequence_completed_at.is_none()
                && physical_text_proof
                    .as_ref()
                    .is_some_and(PhysicalTextProof::is_complete)
            {
                physical_sequence_completed_at = Some(Instant::now());
            }
            if report.pointer_events > 0 {
                println!(
                    "sophia_live_session_pointer schema=1 status=observed events={} routed={}",
                    report.pointer_events, report.pointer_routed
                );
                std::io::stdout().flush()?;
            }
        }

        if !physical_input_completion_reported
            && input_pixel_change
            && let (Some(text), Some(proof)) = (
                config.expect_physical_text.as_deref(),
                physical_text_proof.as_ref(),
            )
            && proof.is_complete()
        {
            println!(
                "sophia_live_session_input schema=2 status=complete source=physical text={} expected_events={} matched_events={} pixel_change=true",
                text,
                proof.expected_events(),
                proof.matched_events(),
            );
            std::io::stdout().flush()?;
            physical_input_completion_reported = true;
        }

        let input_baseline_presented = scene.last_report.as_ref().is_some_and(|report| {
            report.nonzero_pixel_bytes > 0
                && native_scanout.as_ref().is_none_or(|native| {
                    native.heads.first().is_some_and(|head| {
                        head.presented_checksum == report.checksum && head.nonzero_exports > 0
                    })
                })
        });
        if injection_checksum.is_none()
            && config.input_proof_requested()
            && input_baseline_presented
            && (last_authority_update.elapsed() >= Duration::from_millis(SESSION_INPUT_QUIET_MSEC)
                || wm_session.as_ref().is_some_and(|wm| {
                    wm.last_committed_at.is_some_and(|committed| {
                        committed.elapsed() >= Duration::from_millis(SESSION_INPUT_QUIET_MSEC)
                    })
                }))
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
            && physical_input_completion_reported
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
        if config.exit_after_input_proof
            && input_pixel_change
            && (config.expect_physical_text.is_none() || physical_input_completion_reported)
            && (!config.expect_physical_pointer || pointer_pixel_change)
        {
            break;
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
    if config.expect_physical_text.is_some()
        && (!physical_text_proof
            .as_ref()
            .is_some_and(PhysicalTextProof::is_complete)
            || !physical_input_completion_reported)
    {
        return Err("persistent live session did not complete exact physical text proof".into());
    }
    if config.expect_physical_pointer && (!pointer_pixel_change || physical_pointer_routed == 0) {
        return Err(format!(
            "persistent live session pointer input did not change pixels: baseline={pointer_checksum:?} routed={physical_pointer_routed} observed={physical_pointer_events}"
        )
        .into());
    }
    if let Some(wm_session) = wm_session.as_ref()
        && wm_session.committed == 0
    {
        return Err("live session ended without a committed external WM layout".into());
    }
    println!(
        "sophia_live_session schema=7 status=bounded_complete display={} elapsed_msec={} session_ticks={} authority_batches={} authority_transactions={} authority_queue_capacity={} authority_batches_dropped=0 backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_max_nonzero_pixel_bytes={} cpu_nonzero_frames={} cpu_checksum={} injected_input={} input_pixel_change={} physical_events={} physical_keys_routed={} pointer_pixel_change={} physical_pointer_events={} physical_pointer_routed={} pointer_proof={} native_presentation={} native_submissions={} native_submit_deferred={} native_submit_failures={} native_retirements={} native_retire_failures={} native_max_in_flight_ticks={} native_max_submit_to_page_flip_msec={} native_callback_accepted={} native_callback_rejected={} native_callback_queue_saturated={} native_nonzero_exports={} native_export_attempts={} native_in_flight={} native_cleanup_pending={} physical_input={} wm_policy={} wm_requests={} wm_committed={} wm_restarts={} wm_degraded={}",
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
            .map_or(0, PersistentNativeScanout::export_attempts),
        runtime
            .as_ref()
            .is_some_and(PersistentBackendRuntime::native_scanout_in_flight),
        runtime
            .as_ref()
            .is_some_and(PersistentBackendRuntime::native_cleanup_pending),
        if physical_input.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if wm_session.is_some() {
            "external"
        } else {
            "disabled"
        },
        wm_session.as_ref().map_or(0, |wm| wm.requests),
        wm_session.as_ref().map_or(0, |wm| wm.committed),
        wm_session.as_ref().map_or(0, |wm| wm.restarts),
        wm_session.as_ref().is_some_and(|wm| wm.degraded),
    );
    if let (Some(runtime), Some(native_scanout)) = (runtime.as_ref(), native_scanout.as_ref())
        && (native_scanout.submissions == 0
            || native_scanout.retirements == 0
            || native_scanout.nonzero_exports == 0
            || native_scanout.submit_failures != 0
            || native_scanout.retire_failures != 0
            || native_scanout.callback_rejected != 0
            || native_scanout.callback_queue_saturated != 0
            || native_scanout.vsync_overlap_rejections != 0
            || native_scanout.page_flip_phase_rejections != 0
            || runtime.native_scanout_in_flight()
            || runtime.native_cleanup_pending())
    {
        return Err(format!(
            "persistent native scanout did not submit, retire, and drain cleanly: overlap_rejections={} phase_rejections={}",
            native_scanout.vsync_overlap_rejections,
            native_scanout.page_flip_phase_rejections,
        )
        .into());
    }
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_vsync schema=1 status=complete outputs={} overlap_rejections={} phase_rejections={} policy=page_flip_paced",
            native_scanout.heads.len(),
            native_scanout.vsync_overlap_rejections,
            native_scanout.page_flip_phase_rejections,
        );
        for head in &native_scanout.heads {
            println!(
                "sophia_live_output schema=1 status=complete output={} checksum={} submissions={} retirements={} callbacks={} nonzero_exports={}",
                head.output.id.raw(),
                head.last_checksum,
                head.submissions,
                head.retirements,
                head.callback_accepted,
                head.nonzero_exports,
            );
        }
        if native_scanout.heads.iter().any(|head| {
            head.submissions == 0
                || head.retirements == 0
                || head.callback_accepted == 0
                || head.nonzero_exports == 0
        }) {
            return Err(
                "one or more native outputs did not present and retire independently".into(),
            );
        }
        let mut checksums = native_scanout
            .heads
            .iter()
            .map(|head| head.last_checksum)
            .collect::<Vec<_>>();
        checksums.sort_unstable();
        checksums.dedup();
        if checksums.len() != native_scanout.heads.len() {
            return Err("native output frames are not independently distinguishable".into());
        }
    }
    Ok(())
}

struct PersistentOutputRuntime {
    runtime: sophia_backend_live::LiveBackendRuntimeAssembly,
    authority_sender: SyncSender<AuthorityTransactionIntake>,
}

struct PersistentBackendRuntime {
    outputs: BTreeMap<OutputId, PersistentOutputRuntime>,
    layers: BTreeMap<SurfaceId, SurfaceTransaction>,
}

impl PersistentBackendRuntime {
    fn new(
        outputs: &[sophia_engine::HeadlessOutput],
        first_transactions: &[SurfaceTransaction],
        mut native_scanout: Option<&mut PersistentNativeScanout>,
        initial_native_frames: Option<Vec<sophia_backend_live::LiveCpuComposedFrame>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if outputs.is_empty() || outputs.len() > sophia_backend_live::LIVE_RENDERED_OUTPUT_CAPACITY
        {
            return Err("persistent backend runtime requires 1-16 outputs".into());
        }
        let mut initial_native_frames = initial_native_frames.unwrap_or_default().into_iter();
        let mut output_runtimes = BTreeMap::new();
        for (index, output) in outputs.iter().copied().enumerate() {
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
            if let Some(native_scanout) = native_scanout.as_deref_mut() {
                runtime = runtime.with_page_flip_callback_queue(
                    sophia_backend_live::LivePageFlipCallbackQueue::new(
                        native_scanout.take_receiver(index),
                        64,
                    ),
                );
                let selection = native_scanout.selection(index);
                if !runtime.configure_native_output_selection(output.id, selection) {
                    return Err("persistent native output selection was not registered".into());
                }
                native_scanout.initialize(
                    index,
                    &mut runtime,
                    initial_native_frames
                        .next()
                        .ok_or("persistent native scanout has no initial CPU frame")?,
                )?;
            }
            output_runtimes.insert(
                output.id,
                PersistentOutputRuntime {
                    runtime,
                    authority_sender,
                },
            );
        }
        Ok(Self {
            outputs: output_runtimes,
            layers: BTreeMap::new(),
        })
    }

    fn run_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
        mut native_scanout: Option<&mut PersistentNativeScanout>,
        native_frames: Option<Vec<sophia_backend_live::LiveCpuComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        for transaction in &batch.transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        let intake = AuthorityTransactionIntake::new(batch.transaction, batch.transactions.clone());
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let mut native_frames = native_frames.unwrap_or_default().into_iter();
        let mut first_report = None;
        for (index, output) in self.outputs.values_mut().enumerate() {
            output
                .authority_sender
                .try_send(intake.clone())
                .map_err(|error| match error {
                    TrySendError::Full(_) => "persistent live backend authority inbox is full",
                    TrySendError::Disconnected(_) => {
                        "persistent live backend authority inbox is disconnected"
                    }
                })?;
            let input =
                compositor_tick_input(&transactions, batch.transactions.len(), wm_update.clone());
            let report = match native_scanout.as_deref_mut() {
                Some(native_scanout) => {
                    if let Some(frame) = native_frames.next() {
                        native_scanout.queue_frame(index, frame);
                    }
                    native_scanout.run_tick(index, &mut output.runtime, input)?
                }
                None => output.runtime.run_tick(input)?,
            };
            if first_report.is_none() {
                first_report = Some(report);
            }
        }
        first_report.ok_or_else(|| "persistent backend runtime has no outputs".into())
    }

    fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        self.outputs
            .values()
            .next()
            .expect("persistent backend runtime has at least one output")
            .runtime
            .assembly()
            .committed_surfaces()
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
        while self.native_scanout_in_flight() && Instant::now() < deadline {
            self.retire_native_scanout(native_scanout)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        for (index, output) in self.outputs.values_mut().enumerate() {
            if output
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
            {
                let _ = output
                    .runtime
                    .retry_tracked_rendered_primary_plane_scanout_cleanup(
                        native_scanout.card(index),
                    );
            }
        }
        Ok(())
    }

    fn run_native_idle(
        &mut self,
        native_scanout: &mut PersistentNativeScanout,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let mut first_report = None;
        for (index, output) in self.outputs.values_mut().enumerate() {
            if !native_scanout.pending_frame(index) {
                continue;
            }
            let report = native_scanout.run_tick(
                index,
                &mut output.runtime,
                compositor_tick_input(&transactions, 0, None),
            )?;
            if first_report.is_none() {
                first_report = Some(report);
            }
        }
        first_report.ok_or_else(|| "persistent native idle tick had no pending output".into())
    }

    fn retire_native_scanout(
        &mut self,
        native_scanout: &mut PersistentNativeScanout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (index, output) in self.outputs.values_mut().enumerate() {
            native_scanout.retire_ready(index, &mut output.runtime)?;
        }
        Ok(())
    }

    fn native_scanout_in_flight(&self) -> bool {
        self.outputs
            .values()
            .any(|output| output.runtime.rendered_primary_plane_scanout_in_flight())
    }

    fn native_cleanup_pending(&self) -> bool {
        self.outputs.values().any(|output| {
            output
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
        })
    }
}

fn compositor_tick_input(
    transactions: &[SurfaceTransaction],
    x_event_count: usize,
    wm_update: Option<WmTransactionUpdate>,
) -> CompositorBackendTickInput {
    CompositorBackendTickInput {
        x_event_count: u32::try_from(x_event_count).unwrap_or(u32::MAX),
        authority_batches: Vec::new(),
        wm_update,
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
    groups: Vec<PersistentNativeGroup>,
    heads: Vec<PersistentNativeHead>,
    discovered_outputs: usize,
    presentation_outputs: usize,
    submissions: usize,
    submit_deferred: usize,
    submit_failures: usize,
    retirements: usize,
    retire_failures: usize,
    max_in_flight_ticks: u64,
    max_submit_to_page_flip: Duration,
    callback_accepted: usize,
    callback_rejected: usize,
    callback_queue_saturated: usize,
    nonzero_exports: usize,
    presentation: sophia_engine::OutputPresentationRegistry,
    presentation_started: Instant,
    vsync_overlap_rejections: usize,
    page_flip_phase_rejections: usize,
}

struct PersistentNativeGroup {
    session: sophia_backend_live::RealAtomicScanoutPageFlipSession,
    sender: SyncSender<sophia_backend_live::LivePageFlipCallback>,
    receiver: Receiver<sophia_backend_live::LivePageFlipCallback>,
}

struct PersistentNativeHead {
    group: usize,
    selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    exporter: sophia_backend_live::NativeGbmRenderedScanoutBufferDiscoveryExporter<
        sophia_backend_live::RealAtomicScanoutRenderDeviceDiscovery,
    >,
    sender: SyncSender<sophia_backend_live::LivePageFlipCallback>,
    receiver: Option<Receiver<sophia_backend_live::LivePageFlipCallback>>,
    output: sophia_engine::HeadlessOutput,
    submitted_at: Option<Instant>,
    pending_nonzero_pixel_bytes: usize,
    last_checksum: u64,
    submitted_checksum: Option<u64>,
    presented_checksum: u64,
    submissions: usize,
    retirements: usize,
    callback_accepted: usize,
    nonzero_exports: usize,
    scheduled_frame: Option<u64>,
}

impl PersistentNativeScanout {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let authority = sophia_backend_live::RealAtomicScanoutSmokeConfig::default_primary_output()
            .ok_or("persistent native scanout config is invalid")?
            .authority;
        let selection = sophia_backend_live::select_real_atomic_scanout_cards();
        let mut sessions = selection.into_page_flip_sessions(authority);
        if sessions.status != sophia_backend_live::RealAtomicScanoutPageFlipSessionSetStatus::Ready
        {
            return Err(format!(
                "persistent native scanout could not open all KMS outputs: {:?}",
                sessions.status
            )
            .into());
        }
        let outputs = sophia_engine::discover_drm_kms_outputs_from_sysfs("/sys/class/drm")?;
        if sessions.output_count != outputs.len() {
            return Err(format!(
                "persistent native ownership is partial: discovered={} native={}",
                outputs.len(),
                sessions.output_count
            )
            .into());
        }
        let mut presentation_outputs = sophia_engine::DrmKmsOutputRegistry::new();
        for session in &sessions.sessions {
            for (selection, output_id) in session
                .selections()
                .iter()
                .copied()
                .zip(session.outputs().iter().copied())
            {
                let Some(descriptor) = outputs
                    .outputs()
                    .find(|descriptor| descriptor.connector_id == selection.connector_id())
                    .copied()
                else {
                    return Err(format!(
                        "persistent native output has no Engine connector match: connector={}",
                        selection.connector_id(),
                    )
                    .into());
                };
                let descriptor = sophia_engine::DrmKmsOutputDescriptor {
                    output: output_id,
                    ..descriptor
                };
                if presentation_outputs.upsert(descriptor)
                    == sophia_engine::DrmKmsOutputRegistryUpdate::CapacityExceeded
                {
                    return Err("persistent native presentation output capacity exceeded".into());
                }
            }
        }
        if presentation_outputs.len() != sessions.output_count {
            return Err(format!(
                "persistent native connector mapping is incomplete: mapped={} native={}",
                presentation_outputs.len(),
                sessions.output_count,
            )
            .into());
        }
        let presentation =
            sophia_engine::OutputPresentationRegistry::from_outputs(&presentation_outputs);
        let mut groups = Vec::new();
        let mut heads = Vec::new();
        for session in sessions.sessions.drain(..) {
            let group = groups.len();
            for (selection, output_id) in session
                .selections()
                .iter()
                .copied()
                .zip(session.outputs().iter().copied())
            {
                let size = selection.size();
                let discovery = session.render_device_discovery()?;
                let exporter =
                    sophia_backend_live::NativeGbmRenderedScanoutBufferDiscoveryExporter::new(
                        discovery,
                    )
                    .with_preferred_modifiers(
                        session.preferred_xrgb8888_scanout_modifiers_for_selection(selection),
                    );
                let (sender, receiver) = sync_channel(64);
                heads.push(PersistentNativeHead {
                    group,
                    selection,
                    exporter,
                    sender,
                    receiver: Some(receiver),
                    output: sophia_engine::HeadlessOutput {
                        id: output_id,
                        size,
                        scale: 1,
                    },
                    submitted_at: None,
                    pending_nonzero_pixel_bytes: 0,
                    last_checksum: 0,
                    submitted_checksum: None,
                    presented_checksum: 0,
                    submissions: 0,
                    retirements: 0,
                    callback_accepted: 0,
                    nonzero_exports: 0,
                    scheduled_frame: None,
                });
            }
            let (sender, receiver) = sync_channel(64);
            groups.push(PersistentNativeGroup {
                session,
                sender,
                receiver,
            });
        }
        heads.sort_by_key(|head| head.output.id);
        Ok(Self {
            groups,
            heads,
            discovered_outputs: outputs.len(),
            presentation_outputs: presentation.outputs().count(),
            submissions: 0,
            submit_deferred: 0,
            submit_failures: 0,
            retirements: 0,
            retire_failures: 0,
            max_in_flight_ticks: 0,
            max_submit_to_page_flip: Duration::ZERO,
            callback_accepted: 0,
            callback_rejected: 0,
            callback_queue_saturated: 0,
            nonzero_exports: 0,
            presentation,
            presentation_started: Instant::now(),
            vsync_overlap_rejections: 0,
            page_flip_phase_rejections: 0,
        })
    }

    fn outputs(&self) -> Vec<sophia_engine::HeadlessOutput> {
        self.heads.iter().map(|head| head.output).collect()
    }

    fn selection(&self, index: usize) -> sophia_backend_live::LibdrmNativePrimaryPlaneSelection {
        self.heads[index].selection
    }

    fn card(&self, index: usize) -> &sophia_backend_live::RealAtomicScanoutCard {
        self.groups[self.heads[index].group].session.card()
    }

    fn take_receiver(
        &mut self,
        index: usize,
    ) -> Receiver<sophia_backend_live::LivePageFlipCallback> {
        self.heads[index]
            .receiver
            .take()
            .expect("native page-flip receiver must attach once")
    }

    fn run_tick(
        &mut self,
        index: usize,
        runtime: &mut sophia_backend_live::LiveBackendRuntimeAssembly,
        input: CompositorBackendTickInput,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let group = self.heads[index].group;
        self.poll_group_callbacks(group)?;
        let (report, exported_nonzero) = {
            let groups = &mut self.groups;
            let head = &mut self.heads[index];
            let export_attempts_before = head.exporter.cpu_frame_export_attempts();
            let report = runtime
                .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
                    input,
                    groups[group].session.card(),
                    &mut head.exporter,
                )?;
            let exported_nonzero = head.exporter.cpu_frame_export_attempts()
                > export_attempts_before
                && head.pending_nonzero_pixel_bytes > 0;
            if !head.exporter.pending_cpu_frame() {
                head.pending_nonzero_pixel_bytes = 0;
            }
            (report, exported_nonzero)
        };
        if exported_nonzero {
            self.nonzero_exports = self.nonzero_exports.saturating_add(1);
            self.heads[index].nonzero_exports = self.heads[index].nonzero_exports.saturating_add(1);
        }
        if let Some(retire) = report.rendered_primary_plane_scanout_retire {
            self.observe_retire(index, retire);
        }
        self.observe_callbacks(index, report.page_flip_callbacks);
        if let Some(submit) = report.rendered_primary_plane_scanout_submit {
            use sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
            match submit.status {
                Status::SubmittedWaitingForPageFlip => {
                    self.submissions = self.submissions.saturating_add(1);
                    self.heads[index].submissions = self.heads[index].submissions.saturating_add(1);
                    self.heads[index].submitted_at = Some(Instant::now());
                    self.heads[index].submitted_checksum = Some(self.heads[index].last_checksum);
                    let output = self.heads[index].output.id;
                    let _ = self.presentation.mark_damage(output);
                    match self.presentation.schedule(output) {
                        sophia_engine::OutputPresentationSchedule::Scheduled(frame) => {
                            self.heads[index].scheduled_frame = Some(frame.frame_serial);
                        }
                        _ => {
                            self.vsync_overlap_rejections =
                                self.vsync_overlap_rejections.saturating_add(1);
                        }
                    }
                }
                Status::AlreadyInFlight | Status::CleanupPending => {
                    self.submit_deferred = self.submit_deferred.saturating_add(1);
                }
                _ => self.submit_failures = self.submit_failures.saturating_add(1),
            }
        }
        self.max_in_flight_ticks = self
            .max_in_flight_ticks
            .max(report.rendered_primary_plane_scanout_in_flight_ticks);
        Ok(report)
    }

    fn retire_ready(
        &mut self,
        index: usize,
        runtime: &mut sophia_backend_live::LiveBackendRuntimeAssembly,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let group = self.heads[index].group;
        self.poll_group_callbacks(group)?;
        let report = runtime.drain_rendered_primary_plane_page_flip_callbacks_with(
            self.groups[group].session.card(),
        );
        self.observe_callbacks(index, report.page_flip_callbacks);
        if let Some(retire) = report.rendered_primary_plane_scanout_retire {
            self.observe_retire(index, retire);
        }
        Ok(())
    }

    fn observe_retire(
        &mut self,
        index: usize,
        retire: sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutRetireReport,
    ) {
        use sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus as Status;
        match retire.status {
            Status::RetiredAfterPageFlip => {
                self.retirements = self.retirements.saturating_add(1);
                self.heads[index].retirements = self.heads[index].retirements.saturating_add(1);
                if let Some(submitted_at) = self.heads[index].submitted_at.take() {
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

    fn observe_callbacks(
        &mut self,
        index: usize,
        report: sophia_backend_live::LivePageFlipCallbackQueueReport,
    ) {
        self.callback_accepted = self.callback_accepted.saturating_add(report.accepted);
        self.heads[index].callback_accepted = self.heads[index]
            .callback_accepted
            .saturating_add(report.accepted);
        if report.accepted > 0 {
            if let Some(checksum) = self.heads[index].submitted_checksum.take() {
                self.heads[index].presented_checksum = checksum;
            }
            let output = self.heads[index].output.id;
            if let Some(kernel_sequence) = report
                .last_accepted
                .and_then(|accepted| accepted.event.frame_serial)
            {
                let presentation_msec =
                    u64::try_from(self.presentation_started.elapsed().as_millis())
                        .unwrap_or(u64::MAX);
                if !matches!(
                    self.presentation
                        .observe_page_flip(output, kernel_sequence, presentation_msec),
                    sophia_engine::OutputPresentationFeedback::Accepted { .. }
                ) {
                    self.page_flip_phase_rejections =
                        self.page_flip_phase_rejections.saturating_add(1);
                }
            }
            if let Some(frame_serial) = self.heads[index].scheduled_frame.take()
                && !matches!(
                    self.presentation.retire(output, frame_serial),
                    sophia_engine::OutputPresentationRetire::Retired { .. }
                )
            {
                self.page_flip_phase_rejections = self.page_flip_phase_rejections.saturating_add(1);
            }
        }
        self.callback_rejected = self
            .callback_rejected
            .saturating_add(report.rejected_unexpected_output + report.rejected_stale_frame_serial);
        self.callback_queue_saturated = self
            .callback_queue_saturated
            .saturating_add(usize::from(report.max_reached));
    }

    fn initialize(
        &mut self,
        index: usize,
        runtime: &mut sophia_backend_live::LiveBackendRuntimeAssembly,
        frame: sophia_backend_live::LiveCpuComposedFrame,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.queue_frame(index, frame);
        let group = self.heads[index].group;
        let groups = &mut self.groups;
        let head = &mut self.heads[index];
        let export_attempts_before = head.exporter.cpu_frame_export_attempts();
        groups[group]
            .session
            .initialize_persistent_native_gbm_scanout_for_selection(
                runtime,
                &mut head.exporter,
                head.selection,
            )
            .map_err(|evidence| {
                format!("persistent native initial modeset failed: {evidence:?}")
            })?;
        if head.exporter.cpu_frame_export_attempts() > export_attempts_before
            && head.pending_nonzero_pixel_bytes > 0
        {
            self.nonzero_exports = self.nonzero_exports.saturating_add(1);
            head.nonzero_exports = head.nonzero_exports.saturating_add(1);
        }
        if !head.exporter.pending_cpu_frame() {
            head.pending_nonzero_pixel_bytes = 0;
        }
        self.submissions = self.submissions.saturating_add(1);
        head.submissions = head.submissions.saturating_add(1);
        head.presented_checksum = head.last_checksum;
        Ok(())
    }

    fn queue_frame(&mut self, index: usize, frame: sophia_backend_live::LiveCpuComposedFrame) {
        let head = &mut self.heads[index];
        head.pending_nonzero_pixel_bytes = frame.bytes.iter().filter(|byte| **byte != 0).count();
        head.last_checksum = frame
            .bytes
            .iter()
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
            });
        head.exporter.set_pending_cpu_frame(frame);
    }

    fn pending_frame(&self, index: usize) -> bool {
        self.heads[index].exporter.pending_cpu_frame()
    }

    fn export_attempts(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.exporter.cpu_frame_export_attempts())
            .sum()
    }

    fn poll_group_callbacks(&mut self, group: usize) -> Result<(), Box<dyn std::error::Error>> {
        let callbacks = {
            let group = &mut self.groups[group];
            let _ = group
                .session
                .poll_native_page_flip_events(&group.sender, 64, 64);
            let mut callbacks = Vec::new();
            loop {
                match group.receiver.try_recv() {
                    Ok(callback) => callbacks.push(callback),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return Err("native card callback router disconnected".into());
                    }
                }
            }
            callbacks
        };
        for callback in callbacks {
            let Some(head) = self
                .heads
                .iter()
                .find(|head| head.output.id == callback.output)
            else {
                return Err("native callback referenced an unknown output".into());
            };
            head.sender
                .try_send(callback)
                .map_err(|error| match error {
                    TrySendError::Full(_) => "native output callback queue is full",
                    TrySendError::Disconnected(_) => "native output callback queue is disconnected",
                })?;
        }
        Ok(())
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

    fn observe(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for update in &batch.cpu_buffer_updates {
            let stale = self
                .buffers
                .get(&update.handle())
                .is_some_and(|current| update.generation() < current.generation);
            if !stale {
                update.apply_to(&mut self.buffers)?;
            }
        }
        for transaction in &batch.transactions {
            if let BufferSource::CpuBuffer { handle } = transaction.target_buffer {
                self.surfaces
                    .insert(transaction.surface, (transaction.target_geometry, handle));
            }
        }
        Ok(())
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

    fn frames_for_outputs(
        &self,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<Vec<sophia_backend_live::LiveCpuComposedFrame>, Box<dyn std::error::Error>> {
        let primary = self
            .last_report
            .as_ref()
            .ok_or("persistent CPU scene has no composed primary frame")?;
        let mut frames = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            if index == 0 && output.size == primary.frame.size {
                frames.push(primary.frame.clone());
                continue;
            }
            let marker_size = Size {
                width: output.size.width.min(64).max(1),
                height: output.size.height.min(64).max(1),
            };
            let marker_width = usize::try_from(marker_size.width)?;
            let marker_height = usize::try_from(marker_size.height)?;
            let marker_stride = marker_width
                .checked_mul(4)
                .ok_or("marker stride overflow")?;
            let marker_byte = u8::try_from((index + 1).min(255)).unwrap_or(255);
            let marker = sophia_backend_live::LiveCpuCompositionLayer {
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: marker_size.width,
                    height: marker_size.height,
                },
                buffer: sophia_backend_live::LiveCpuBufferSource {
                    handle: 0x5350_4800u64.saturating_add(index as u64),
                    size: marker_size,
                    stride: u32::try_from(marker_stride)?,
                    format: sophia_backend_live::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                    generation: 1,
                    bytes: vec![marker_byte; marker_stride.saturating_mul(marker_height)],
                },
            };
            frames.push(
                sophia_backend_live::compose_live_cpu_frame(output.size, &[marker])
                    .map_err(|error| format!("secondary output composition failed: {error:?}"))?
                    .frame,
            );
        }
        Ok(frames)
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
    mut physical_text_proof: Option<&mut PhysicalTextProof>,
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
                if let Some(proof) = physical_text_proof.as_deref_mut() {
                    if proof.is_complete() {
                        continue;
                    }
                    let observed = PhysicalTextProofEvent {
                        keycode,
                        pressed,
                        state,
                    };
                    if let Err(mismatch) = proof.observe(observed) {
                        return Err(format!(
                            "physical text proof sequence mismatch at event {}: expected keycode={} pressed={} state={} observed keycode={} pressed={} state={}",
                            mismatch.event_index,
                            mismatch.expected.keycode,
                            mismatch.expected.pressed,
                            mismatch.expected.state,
                            mismatch.observed.keycode,
                            mismatch.observed.pressed,
                            mismatch.observed.state,
                        )
                        .into());
                    }
                }
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

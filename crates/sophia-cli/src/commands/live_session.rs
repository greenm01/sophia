use super::prelude::*;

use sophia_cli::emergency_input::{EmergencyChordAction, EmergencyChordState};
use sophia_cli::input_proof::{PhysicalTextProof, PhysicalTextProofEvent};
use sophia_cli::resize_transaction::ResizeRollbackCoordinator;
use sophia_engine::{
    FocusedInputRoute, InputFocusDecision, InputFocusState, NonBlockingInputPoller,
};
use sophia_protocol::{
    ClientAdmissionContext, DeviceId, NamespaceCapabilities, NamespaceId, NamespaceProfile,
    OutputId, Point, SeatId, WmManageSurface,
};
use sophia_runtime::NamespaceRegistry;
use sophia_x_authority::{
    XAuthorityClientControlAck, XAuthorityClientControlCommand, XAuthorityClientInputDelivery,
    XAuthorityClientSurfaceRoutes, XAuthorityControlCommand, XAuthorityControlOutcome,
    XAuthorityInputDeliveryId, XAuthorityInputDeliveryOutcome, XAuthorityRoutedInput,
    XCoreKeyboardMapper, XPresentCompletionMode, XServerFrontendAdmissionError,
    XServerFrontendAdmissionPolicy, XServerFrontendAdmissionRequest, XServerFrontendConfig,
    XServerFrontendProtocolRouter, XServerFrontendRenderDeviceError,
    XServerFrontendRenderDeviceProvider, XServerFrontendRouteBroker, XServerFrontendServiceCommand,
    XServerFrontendSetupAuthorization, run_x_server_frontend_routed_until_stopped,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) mod input_guard;

const SESSION_AUTHORITY_CAPACITY: usize = 256;
const SESSION_KEY_CAPACITY: usize = 64;
const SESSION_CONTROL_CAPACITY: usize = 32;
const SESSION_INPUT_QUIET_MSEC: u64 = 500;
const SESSION_PHYSICAL_SEQUENCE_TIMEOUT_MSEC: u64 = 15_000;
const SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC: u64 = 5_000;
const SESSION_INPUT_DELIVERY_TIMEOUT_MSEC: u64 = 1_000;
const SESSION_SEAT_RAW: u64 = 1;
const SESSION_KEYBOARD_DEVICE_RAW: u64 = 1;
const SESSION_POINTER_DEVICE_RAW: u64 = 2;
const PRIMARY_INPUT_PROOF_SCRIPT: &str = r#"printf 'type %s then Return: ' "$1"; IFS= read -r line; umask 077; printf '%s' "$line" > "$2"; printf '\nreceived:%s\n' "$line"; sleep 300"#;
const SECONDARY_POINTER_WITNESS_SCRIPT: &str = r#"saved=$(stty -g); stty raw -echo; printf '\033[?1000h\033[?1006hPointer witness: click here\r\n'; dd bs=1 count=1 >/dev/null 2>&1; printf '\033[?1000l\033[?1006l'; stty "$saved"; printf 'Pointer input received\n'; sleep 300"#;
static NEXT_SESSION_GENERATION: AtomicU64 = AtomicU64::new(1);

struct LiveXAdmissionPolicy {
    registry: Arc<Mutex<NamespaceRegistry>>,
    namespace: NamespaceId,
    session_user_id: u32,
}

impl XServerFrontendAdmissionPolicy for LiveXAdmissionPolicy {
    fn admit(
        &self,
        request: XServerFrontendAdmissionRequest,
    ) -> Result<ClientAdmissionContext, XServerFrontendAdmissionError> {
        let peer = request
            .peer_credentials
            .ok_or(XServerFrontendAdmissionError::Denied)?;
        if peer.user_id != self.session_user_id {
            return Err(XServerFrontendAdmissionError::Denied);
        }
        self.registry
            .lock()
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)?
            .admit(self.namespace, request.setup_authentication)
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)
    }

    fn revoke(&self, context: ClientAdmissionContext) -> Result<(), XServerFrontendAdmissionError> {
        if context.namespace.id != self.namespace {
            return Err(XServerFrontendAdmissionError::Unavailable);
        }
        self.registry
            .lock()
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)?
            .revoke_admission(context.client_id)
            .map(|_| ())
            .map_err(|_| XServerFrontendAdmissionError::Unavailable)
    }
}

struct LiveXRenderDeviceProvider {
    device: std::fs::File,
}

impl XServerFrontendRenderDeviceProvider for LiveXRenderDeviceProvider {
    fn open_render_device_fd(
        &self,
    ) -> Result<std::os::fd::OwnedFd, XServerFrontendRenderDeviceError> {
        use std::os::fd::AsRawFd as _;

        let proc_path = format!("/proc/self/fd/{}", self.device.as_raw_fd());
        let selected_node = std::fs::read_link(&proc_path)
            .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)?;
        let selected_name = selected_node
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(XServerFrontendRenderDeviceError::Unavailable)?;

        let render_node = if selected_name.starts_with("renderD") {
            selected_node
        } else {
            let selected_device =
                std::fs::canonicalize(format!("/sys/class/drm/{selected_name}/device"))
                    .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)?;
            std::fs::read_dir("/sys/class/drm")
                .map_err(|_| XServerFrontendRenderDeviceError::Unavailable)?
                .filter_map(Result::ok)
                .take(64)
                .find_map(|entry| {
                    let name = entry.file_name();
                    let name = name.to_str()?;
                    if !name.starts_with("renderD") {
                        return None;
                    }
                    let device = std::fs::canonicalize(entry.path().join("device")).ok()?;
                    (device == selected_device).then(|| std::path::Path::new("/dev/dri").join(name))
                })
                .ok_or(XServerFrontendRenderDeviceError::Unavailable)?
        };

        // A fresh render-node open gives each DRI3 client its own DRM file
        // description and withholds the compositor's primary/KMS node.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(render_node)
            .map(std::os::fd::OwnedFd::from)
            .map_err(|_| XServerFrontendRenderDeviceError::OpenFailed)
    }
}

struct LiveXAuthorityFile {
    path: Option<std::path::PathBuf>,
}

struct LiveInputProofResult {
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl LiveInputProofResult {
    fn create(display_number: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut nonce = [0u8; 8];
        fill_session_random(&mut nonce)?;
        let suffix = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let directory = std::env::temp_dir().join(format!(
            "sophia-input-proof-{}-{display_number}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&directory)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let path = directory.join("received");
        Ok(Self { directory, path })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn received(&self) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for LiveInputProofResult {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

impl LiveXAuthorityFile {
    fn create(display_number: u32) -> Result<(Self, [u8; 16]), Box<dyn std::error::Error>> {
        Self::create_in(&live_xauthority_directory(), display_number)
    }

    fn create_in(
        directory: &std::path::Path,
        display_number: u32,
    ) -> Result<(Self, [u8; 16]), Box<dyn std::error::Error>> {
        let mut cookie = [0u8; 16];
        fill_session_random(&mut cookie)?;
        let mut nonce = [0u8; 8];
        fill_session_random(&mut nonce)?;
        let suffix = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = directory.join(format!(
            ".sophia-Xauthority-{}-{display_number}-{suffix}",
            std::process::id()
        ));
        let record = encode_live_xauthority_record(display_number, cookie)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let create_result = (|| -> Result<(), Box<dyn std::error::Error>> {
            file.write_all(&record)?;
            file.sync_all()?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(())
        })();
        if let Err(error) = create_result {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        Ok((Self { path: Some(path) }, cookie))
    }

    fn path(&self) -> &std::path::Path {
        self.path
            .as_deref()
            .expect("live Xauthority path is retained until cleanup")
    }

    fn remove(&mut self) -> Result<(), std::io::Error> {
        let Some(path) = self.path.take() else {
            return Ok(());
        };
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for LiveXAuthorityFile {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

fn live_xauthority_directory() -> std::path::PathBuf {
    let effective_user = rustix::process::geteuid().as_raw();
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| {
                metadata.is_dir()
                    && metadata.uid() == effective_user
                    && metadata.permissions().mode() & 0o077 == 0
            })
        })
        .unwrap_or_else(std::env::temp_dir)
}

fn fill_session_random(bytes: &mut [u8]) -> Result<(), std::io::Error> {
    let mut filled = 0;
    while filled < bytes.len() {
        let written =
            rustix::rand::getrandom(&mut bytes[filled..], rustix::rand::GetRandomFlags::empty())?;
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "kernel random source returned no bytes",
            ));
        }
        filled += written;
    }
    Ok(())
}

fn encode_live_xauthority_record(
    display_number: u32,
    cookie: [u8; 16],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const FAMILY_LOCAL: u16 = 256;
    let system = rustix::system::uname();
    let hostname = system.nodename().to_bytes();
    let display = display_number.to_string();
    let mut record = Vec::with_capacity(64 + hostname.len());
    record.extend_from_slice(&FAMILY_LOCAL.to_be_bytes());
    push_xauthority_field(&mut record, hostname)?;
    push_xauthority_field(&mut record, display.as_bytes())?;
    push_xauthority_field(&mut record, b"MIT-MAGIC-COOKIE-1")?;
    push_xauthority_field(&mut record, &cookie)?;
    Ok(record)
}

fn push_xauthority_field(
    output: &mut Vec<u8>,
    field: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let len = u16::try_from(field.len()).map_err(|_| "Xauthority field exceeds u16")?;
    output.extend_from_slice(&len.to_be_bytes());
    output.extend_from_slice(field);
    Ok(())
}

enum SessionPhysicalInput {
    Direct(
        sophia_backend_live::NativeLibinputEventPoller<
            sophia_backend_live::NativeLibinputEventReader,
        >,
    ),
}

impl NonBlockingInputPoller for SessionPhysicalInput {
    fn poll_ready(&mut self) -> std::io::Result<Vec<sophia_protocol::InputEventPacket>> {
        match self {
            Self::Direct(poller) => poller.poll_ready(),
        }
    }
}

impl SessionPhysicalInput {
    fn stats(&self) -> sophia_backend_live::ThreadedNativeInputStats {
        match self {
            Self::Direct(_) => Default::default(),
        }
    }
}

pub(crate) fn run_persistent_xterm_session(
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = PersistentXtermSessionConfig::from_args(args)?;
    let terminal = super::x_authority::resolve_external_probe_binary("xterm", &config.terminal)?;
    prepare_display_socket(&config.socket_path)?;
    let display_number = parse_display_number(&config.display)?;
    let (mut xauthority, xauthority_cookie) = LiveXAuthorityFile::create(display_number)?;
    let mut native_scanout = config
        .native_scanout
        .then(PersistentNativeScanout::new)
        .transpose()?;
    let mut physical_input = if config.input_devices.is_empty() {
        None
    } else {
        Some(SessionPhysicalInput::Direct(
            sophia_backend_live::open_native_libinput_path_poller(
                &config.input_devices,
                sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(
                    SESSION_SEAT_RAW,
                ))
                .with_keyboard_device(DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW))
                .with_pointer_device(DeviceId::from_raw(SESSION_POINTER_DEVICE_RAW)),
                64,
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
    let mut wm_session = LiveWmSession::from_config(&config)?;
    let initial_outputs = native_scanout
        .as_ref()
        .map(PersistentNativeScanout::outputs)
        .unwrap_or_else(|| vec![sophia_engine::HeadlessOutput::deterministic()]);
    let output_topology = output_topology_from_engine_outputs(&initial_outputs)?;

    let server_path = config.socket_path.clone();
    let session_generation = NEXT_SESSION_GENERATION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |generation| {
            generation.checked_add(1)
        })
        .map_err(|_| "Sophia session generation exhausted")?;
    let namespace_registry = Arc::new(Mutex::new(NamespaceRegistry::new(session_generation)?));
    let x_namespace = namespace_registry
        .lock()
        .map_err(|_| "Sophia namespace registry lock was poisoned")?
        .create_namespace(config.namespace_profile, config.namespace_capabilities);
    let session_user_id = rustix::process::geteuid().as_raw();
    let admission_policy = Arc::new(LiveXAdmissionPolicy {
        registry: namespace_registry.clone(),
        namespace: x_namespace.id,
        session_user_id,
    });
    let mut frontend_config =
        XServerFrontendConfig::new_with_namespace_context(&server_path, x_namespace)?
            .with_output_topology(output_topology.clone())?
            .with_xkb_config(config.xkb_config.clone())?
            .with_setup_authorization(XServerFrontendSetupAuthorization::MitMagicCookie(
                xauthority_cookie,
            ))
            .with_admission_policy(admission_policy);
    if let Some(native_scanout) = native_scanout.as_ref() {
        frontend_config =
            frontend_config.with_render_device_provider(Arc::new(LiveXRenderDeviceProvider {
                device: native_scanout.clone_render_device_file()?,
            }));
    }
    let (authority_sender, authority_receiver) = sync_channel(SESSION_AUTHORITY_CAPACITY);
    let (control_ack_sender, control_ack_receiver) = sync_channel(SESSION_CONTROL_CAPACITY);
    let (input_delivery_sender, input_delivery_receiver) = sync_channel(SESSION_KEY_CAPACITY);
    let broker =
        XServerFrontendRouteBroker::with_control_and_input_delivery_senders_and_xkb_config(
            NonZeroUsize::new(SESSION_KEY_CAPACITY).expect("session route capacity is nonzero"),
            control_ack_sender,
            input_delivery_sender,
            config.xkb_config.clone(),
        )?;
    let input_sender = broker.routed_input_sender();
    let control_sender = broker.control_sender();
    let protocol_router = broker.protocol_router();
    let (service_command_sender, service_command_receiver) = sync_channel(1);
    let mut server = Some(std::thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            frontend_config,
            authority_sender,
            broker,
            service_command_receiver,
        )
    }));
    wait_for_x_server_socket(&config.socket_path, &mut server)?;

    let input_proof_result = config
        .input_proof_requested()
        .then(|| LiveInputProofResult::create(display_number))
        .transpose()?;
    let mut terminal_command = std::process::Command::new(&terminal);
    terminal_command
        .env("DISPLAY", &config.display)
        .env("XAUTHORITY", xauthority.path())
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
                PRIMARY_INPUT_PROOF_SCRIPT,
                "sophia-input-proof",
            ])
            .arg(proof_text)
            .arg(
                input_proof_result
                    .as_ref()
                    .expect("input proof result exists with proof text")
                    .path(),
            );
    } else if let Some(program) = config.terminal_exec.as_deref() {
        terminal_command
            .env_remove("ENV")
            .env_remove("BASH_ENV")
            .arg("-e")
            .arg(program)
            .args(&config.terminal_exec_args);
    }
    let child = terminal_command.spawn()?;
    let mut process = SessionProcessGuard::new(child, Vec::new(), config.socket_path.clone());
    // Admit one primary-client transaction before launching the secondary
    // proof client. Otherwise optimized startup lets both xterms race for the
    // first committed surface, making initial focus nondeterministic.
    let initial_authority_batch = if config.secondary_terminal {
        Some(
            authority_receiver
                .recv_timeout(Duration::from_secs(5))
                .map_err(|error| {
                    format!("primary xterm did not publish a startup frame: {error}")
                })?,
        )
    } else {
        None
    };
    if config.secondary_terminal {
        process.add_secondary_child(spawn_secondary_xterm(
            &terminal,
            &config.display,
            xauthority.path(),
            config
                .inject_text
                .as_deref()
                .or(config.expect_physical_text.as_deref()),
        )?);
    }

    let mut randr_witness = config
        .inject_output_size
        .map(|_| open_randr_update_witness(&config.socket_path, xauthority_cookie))
        .transpose()?;
    let mut output_notifications = 0usize;
    if let Some(size) = config.inject_output_size {
        let mut snapshot = output_topology.clone();
        snapshot.generation = snapshot.generation.saturating_add(1);
        let primary_id = snapshot.primary;
        let primary = snapshot
            .outputs
            .iter_mut()
            .find(|entry| entry.output == primary_id)
            .ok_or("live output injection lost the primary output")?;
        primary.logical.width = size.width;
        primary.logical.height = size.height;
        primary.pixel_size = size;
        snapshot
            .validate()
            .map_err(|error| format!("invalid --inject-output-size topology: {error:?}"))?;
        let (ack_sender, ack_receiver) = sync_channel(1);
        service_command_sender.send(XServerFrontendServiceCommand::UpdateOutputTopology {
            snapshot,
            acknowledgement: ack_sender,
        })?;
        let outcome = ack_receiver.recv_timeout(Duration::from_secs(1))?;
        let notifications = match outcome {
            sophia_x_authority::XAuthorityOutputUpdateOutcome::Applied {
                notifications, ..
            } => notifications,
            outcome => {
                return Err(format!("live output injection was rejected: {outcome:?}").into());
            }
        };
        output_notifications = notifications;
        let witness = randr_witness
            .as_mut()
            .ok_or("live output injection lost its RandR witness")?;
        confirm_randr_update_witness(witness, size)?;
        println!(
            "sophia_live_output_update schema=3 status=applied width={} height={} notifications={} witness=true",
            size.width, size.height, notifications
        );
    }

    println!(
        "sophia_live_session schema=7 status=running display={} terminal=xterm runtime=persistent authority_capacity={} input_capacity={} control_capacity={} native_presentation={} physical_input={} pointer_proof={} secondary_terminal={} wm_policy={} namespace_profile={} namespace_request_capabilities={} namespace_publish_capabilities={}",
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
        if config.secondary_terminal {
            "enabled"
        } else {
            "disabled"
        },
        if wm_session.is_some() {
            "external"
        } else {
            "disabled"
        },
        match config.namespace_profile {
            NamespaceProfile::ClassicShared => "classic_shared",
            NamespaceProfile::Confined => "confined",
        },
        config.namespace_capabilities.request_bits(),
        config.namespace_capabilities.publish_bits(),
    );
    if let Some(native_scanout) = native_scanout.as_ref() {
        println!(
            "sophia_live_outputs schema=2 status=ready discovered={} presentation={} native_owned={} multi_output_scanout=enabled layout=extended_horizontal",
            native_scanout.discovered_outputs,
            native_scanout.presentation_outputs,
            native_scanout.heads.len(),
        );
    }

    let (primary_child, secondary_children) = process.children_mut()?;
    let result = run_session_loop(
        &config,
        &authority_receiver,
        &input_sender,
        &control_sender,
        &control_ack_receiver,
        &input_delivery_receiver,
        primary_child,
        secondary_children,
        &mut physical_input,
        &mut native_scanout,
        &mut wm_session,
        protocol_router,
        input_proof_result.as_ref(),
        false,
        initial_authority_batch,
        output_notifications,
    );
    drop(randr_witness);
    // Stop frontend routing before terminating its clients. Pointer motion can
    // leave a bounded burst in the Engine ingress queue; killing xterm first
    // turns that normal shutdown backlog into a client-queue disconnect.
    let _ = service_command_sender.send(XServerFrontendServiceCommand::StopAccepting);
    drop(input_sender);
    drop(control_sender);
    process.terminate()?;
    let server_result = server
        .take()
        .expect("X Server Frontend handle is retained after startup")
        .join()
        .map_err(|_| "persistent X authority server thread panicked")?;
    server_result.map_err(|error| format!("persistent X authority server failed: {error}"))?;
    namespace_registry
        .lock()
        .map_err(|_| "Sophia namespace registry lock was poisoned")?
        .revoke_namespace(x_namespace.id)?;
    let xauthority_cleanup = xauthority.remove();
    result?;
    xauthority_cleanup?;
    Ok(())
}

fn output_topology_from_engine_outputs(
    outputs: &[sophia_engine::HeadlessOutput],
) -> Result<sophia_protocol::OutputTopologySnapshot, Box<dyn std::error::Error>> {
    let primary = outputs
        .first()
        .ok_or("live session requires at least one Engine output")?
        .id;
    let mut logical_x = 0i32;
    let entries = outputs
        .iter()
        .map(|output| {
            let scale = output.scale.max(1);
            let scale_i32 = i32::try_from(scale).unwrap_or(i32::MAX);
            let logical_size = Size {
                width: output.size.width.saturating_div(scale_i32).max(1),
                height: output.size.height.saturating_div(scale_i32).max(1),
            };
            let logical = Rect {
                x: logical_x,
                y: 0,
                width: logical_size.width,
                height: logical_size.height,
            };
            logical_x = logical_x.saturating_add(logical_size.width);
            sophia_protocol::OutputTopologyEntry {
                output: output.id,
                logical,
                pixel_size: output.size,
                scale,
                refresh_millihz: 60_000,
            }
        })
        .collect();
    let snapshot = sophia_protocol::OutputTopologySnapshot {
        generation: 1,
        primary,
        outputs: entries,
    };
    snapshot
        .validate()
        .map_err(|error| -> Box<dyn std::error::Error> {
            format!("invalid live Engine output topology: {error:?}").into()
        })?;
    Ok(snapshot)
}

#[derive(Clone, Debug)]
struct PersistentXtermSessionConfig {
    display: String,
    socket_path: std::path::PathBuf,
    terminal: String,
    terminal_exec: Option<String>,
    terminal_exec_args: Vec<String>,
    secondary_terminal: bool,
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
    input_quiet_msec: u64,
    namespace_profile: NamespaceProfile,
    namespace_capabilities: NamespaceCapabilities,
    xkb_config: sophia_x_authority::XkbRmlvoConfig,
    inject_output_size: Option<Size>,
    inject_surface_resize: Option<Size>,
    m4_first_acquire_delay: Option<Duration>,
    m4_reject_first_present: bool,
    m4_diagnose_first_mixed_export: bool,
}

#[derive(Debug)]
pub(crate) struct NativeEglMixedSmokeComplete {
    pub status: sophia_backend_live::LiveRendererScanoutBufferExportStatus,
    pub detail: sophia_backend_live::LiveRendererScanoutBufferExportDetail,
    pub cpu_layers: usize,
    pub dmabuf_layers: usize,
    pub live_sources: usize,
    pub live_fences: usize,
    pub live_transactions: usize,
}

impl NativeEglMixedSmokeComplete {
    pub fn reduced_log_line(&self, child_outcome: &str) -> String {
        format!(
            "sophia_native_egl_mixed schema=1 case=mixed status={:?} stage={:?} cpu_layers={} dmabuf_layers={} child_outcome={} live_sources={} live_fences={} live_transactions={}",
            self.status,
            self.detail,
            self.cpu_layers,
            self.dmabuf_layers,
            child_outcome,
            self.live_sources,
            self.live_fences,
            self.live_transactions,
        )
    }
}

impl std::fmt::Display for NativeEglMixedSmokeComplete {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reduced_log_line("completed"))
    }
}

impl std::error::Error for NativeEglMixedSmokeComplete {}

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
        let secondary_terminal = args.iter().any(|arg| arg == "--secondary-terminal");
        let exit_after_input_proof = args.iter().any(|arg| arg == "--exit-after-input-proof");
        let native_scanout = args.iter().any(|arg| arg == "--native-scanout");
        let namespace_profile = match arg_value(args, "--namespace-profile").as_deref() {
            None | Some("classic") | Some("classic-shared") => NamespaceProfile::ClassicShared,
            Some("confined") => NamespaceProfile::Confined,
            Some(profile) => {
                return Err(format!(
                    "unsupported namespace profile {profile:?}; expected classic or confined"
                )
                .into());
            }
        };
        let defaults = sophia_x_authority::XkbRmlvoConfig::default();
        let xkb_config = sophia_x_authority::XkbRmlvoConfig {
            rules: arg_value(args, "--xkb-rules").unwrap_or(defaults.rules),
            model: arg_value(args, "--xkb-model").unwrap_or(defaults.model),
            layout: arg_value(args, "--xkb-layout").unwrap_or(defaults.layout),
            variant: arg_value(args, "--xkb-variant").unwrap_or(defaults.variant),
            options: arg_value(args, "--xkb-options").unwrap_or(defaults.options),
        };
        xkb_config.validate()?;
        let inject_output_size = arg_value(args, "--inject-output-size")
            .as_deref()
            .map(parse_output_size)
            .transpose()?;
        let inject_surface_resize = arg_value(args, "--inject-surface-resize")
            .as_deref()
            .map(parse_output_size)
            .transpose()?;
        let m4_first_acquire_delay = arg_value(args, "--m4-first-acquire-delay-ms")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .map(Duration::from_millis);
        if m4_first_acquire_delay.is_some_and(|delay| delay.is_zero() || delay.as_millis() > 2_000)
        {
            return Err("--m4-first-acquire-delay-ms accepts 1-2000 milliseconds".into());
        }
        let m4_reject_first_present = args.iter().any(|arg| arg == "--m4-reject-first-present");
        let m4_diagnose_first_mixed_export = args
            .iter()
            .any(|arg| arg == "--m4-diagnose-first-mixed-export");
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
        if (m4_first_acquire_delay.is_some()
            || m4_reject_first_present
            || m4_diagnose_first_mixed_export)
            && (!native_scanout || terminal_exec.is_none())
        {
            return Err(
                "M4 Present proof controls require --native-scanout and --terminal-exec".into(),
            );
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
            secondary_terminal,
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
            input_quiet_msec: SESSION_INPUT_QUIET_MSEC,
            namespace_profile,
            namespace_capabilities: NamespaceCapabilities::NONE,
            xkb_config,
            inject_output_size,
            inject_surface_resize,
            m4_first_acquire_delay,
            m4_reject_first_present,
            m4_diagnose_first_mixed_export,
        })
    }

    fn input_proof_requested(&self) -> bool {
        self.inject_text.is_some() || self.expect_physical_text.is_some()
    }
}

fn parse_output_size(value: &str) -> Result<Size, Box<dyn std::error::Error>> {
    let (width, height) = value
        .split_once('x')
        .ok_or("--inject-output-size expects WIDTHxHEIGHT")?;
    let size = Size {
        width: width.parse()?,
        height: height.parse()?,
    };
    if size.width <= 0 || size.height <= 0 || size.width > 16_384 || size.height > 16_384 {
        return Err("--inject-output-size accepts dimensions from 1 through 16384".into());
    }
    Ok(size)
}

fn open_randr_update_witness(
    socket_path: &std::path::Path,
    cookie: [u8; 16],
) -> Result<std::os::unix::net::UnixStream, Box<dyn std::error::Error>> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket_path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let auth_name = b"MIT-MAGIC-COOKIE-1";
    let mut setup = Vec::with_capacity(48);
    setup.extend_from_slice(&[b'l', 0]);
    setup.extend_from_slice(&11u16.to_le_bytes());
    setup.extend_from_slice(&0u16.to_le_bytes());
    setup.extend_from_slice(&(auth_name.len() as u16).to_le_bytes());
    setup.extend_from_slice(&(cookie.len() as u16).to_le_bytes());
    setup.extend_from_slice(&[0, 0]);
    setup.extend_from_slice(auth_name);
    setup.resize((setup.len() + 3) & !3, 0);
    setup.extend_from_slice(&cookie);
    stream.write_all(&setup)?;
    stream.flush()?;

    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    if header[0] != 1 {
        return Err("RandR witness X11 setup was rejected".into());
    }
    let extra = usize::from(u16::from_le_bytes([header[6], header[7]])) * 4;
    let mut body = vec![0; extra];
    stream.read_exact(&mut body)?;

    let root = sophia_x_authority::X_SETUP_DEFAULT_ROOT;
    let mut select = Vec::with_capacity(12);
    select.extend_from_slice(&[
        sophia_x_authority::X_RANDR_MAJOR_OPCODE,
        sophia_x_authority::X_RANDR_SELECT_INPUT_MINOR_OPCODE,
    ]);
    select.extend_from_slice(&3u16.to_le_bytes());
    select.extend_from_slice(&root.to_le_bytes());
    select.extend_from_slice(&0x47u16.to_le_bytes());
    select.extend_from_slice(&[0, 0]);
    stream.write_all(&select)?;
    // A reply-producing core request is a deterministic barrier proving the
    // preceding void RandR selection was dispatched before Engine updates.
    stream.write_all(&[43, 0, 1, 0])?;
    stream.flush()?;
    let mut barrier = [0u8; 32];
    stream.read_exact(&mut barrier)?;
    if barrier[0] != 1 {
        return Err("RandR witness barrier request failed".into());
    }
    Ok(stream)
}

fn confirm_randr_update_witness(
    stream: &mut std::os::unix::net::UnixStream,
    size: Size,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut event = [0u8; 32];
    stream.read_exact(&mut event)?;
    if event[0] != sophia_x_authority::X_RANDR_FIRST_EVENT
        || u16::from_le_bytes([event[24], event[25]]) != u16::try_from(size.width)?
        || u16::from_le_bytes([event[26], event[27]]) != u16::try_from(size.height)?
    {
        return Err(format!("RandR witness received an unexpected update: {event:?}").into());
    }
    Ok(())
}

fn spawn_secondary_xterm(
    terminal: &std::path::Path,
    display: &str,
    xauthority: &std::path::Path,
    input_proof: Option<&str>,
) -> Result<Child, Box<dyn std::error::Error>> {
    let mut command = std::process::Command::new(terminal);
    command
        .env("DISPLAY", display)
        .env("XAUTHORITY", xauthority)
        .args([
            "-cm",
            "-dc",
            "-geometry",
            "100x28+420+90",
            "-title",
            "Sophia Secondary Terminal",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if input_proof.is_some() {
        command.args(["-e", "sh", "-c", SECONDARY_POINTER_WITNESS_SCRIPT]);
    } else {
        command.args([
            "-e",
            "sh",
            "-c",
            "printf 'Sophia secondary terminal\\n'; sleep 300",
        ]);
    }
    Ok(command.spawn()?)
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

fn wait_for_x_server_socket(
    path: &std::path::Path,
    server: &mut Option<
        std::thread::JoinHandle<Result<(), sophia_x_authority::X11SetupSocketError>>,
    >,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        if server
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            return match server.take().expect("checked above").join() {
                Ok(Ok(())) => Err("X Server Frontend exited before creating its socket".into()),
                Ok(Err(error)) => Err(format!(
                    "X Server Frontend failed before creating {}: {error}",
                    path.display()
                )
                .into()),
                Err(_) => Err("X Server Frontend panicked before creating its socket".into()),
            };
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "timed out waiting for X authority socket {}",
        path.display()
    )
    .into())
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
    staged_transactions: BTreeMap<SurfaceId, SurfaceTransaction>,
    staged_cpu_buffer_updates: Vec<XAuthorityCpuBufferUpdate>,
}

#[derive(Default)]
struct PersistentLiveLayout {
    layers: BTreeMap<SurfaceId, LayerSnapshot>,
    resize: ResizeRollbackCoordinator,
    client_routes: XAuthorityClientSurfaceRoutes,
    unmanaged_surfaces: BTreeSet<SurfaceId>,
    pending: Option<PendingLiveWmLayout>,
    focus_to_apply: Option<(TransactionId, SurfaceId)>,
    stage_new_surfaces_offset: bool,
    center_first_surface_in: Option<Size>,
    committed_resize_replay: Option<(Vec<SurfaceTransaction>, Vec<XAuthorityCpuBufferUpdate>)>,
}

impl PersistentLiveLayout {
    fn new(stage_new_surfaces_offset: bool, center_first_surface_in: Option<Size>) -> Self {
        Self {
            stage_new_surfaces_offset,
            center_first_surface_in,
            ..Self::default()
        }
    }

    fn observe_authority_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Vec<SurfaceId> {
        self.client_routes.observe(batch);
        self.remove_surfaces(&batch.removed_surfaces);
        let mut new_surfaces = Vec::new();
        for (index, transaction) in batch.transactions.iter().enumerate() {
            let size = Size {
                width: transaction.target_geometry.width,
                height: transaction.target_geometry.height,
            };
            if !self.resize.accept_observation(transaction.surface, size) {
                continue;
            }
            let staged_for_resize = self.pending.as_ref().is_some_and(|pending| {
                pending.requested_sizes.get(&transaction.surface) == Some(&size)
            });
            if staged_for_resize {
                let pending = self.pending.as_mut().expect("checked above");
                pending
                    .staged_transactions
                    .insert(transaction.surface, transaction.clone());
                if let Some(layer) = pending
                    .layers
                    .iter_mut()
                    .find(|layer| layer.surface == transaction.surface)
                {
                    layer.source = transaction.target_buffer;
                    layer.damage = transaction.damage.clone();
                    layer.generation = transaction.previous_committed_generation.saturating_add(1);
                }
                continue;
            }
            self.resize.record_committed(transaction.surface, size);
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
                    } else if let Some(output) = self.center_first_surface_in.take() {
                        geometry = center_geometry_without_scaling(geometry, output);
                    }
                    self.layers.insert(
                        transaction.surface,
                        LayerSnapshot {
                            surface: transaction.surface,
                            authority_local_id: None,
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
        if let Some(pending) = self.pending.as_mut() {
            let staged_handles = pending
                .staged_transactions
                .values()
                .filter_map(|transaction| match transaction.target_buffer {
                    BufferSource::CpuBuffer { handle } => Some(handle),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            pending.staged_cpu_buffer_updates.extend(
                batch
                    .cpu_buffer_updates
                    .iter()
                    .filter(|update| staged_handles.contains(&update.handle()))
                    .cloned(),
            );
        }
        new_surfaces
    }

    fn remove_surfaces(&mut self, removed_surfaces: &[SurfaceId]) {
        if removed_surfaces.is_empty() {
            return;
        }
        self.layers
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for surface in removed_surfaces {
            self.resize.remove(*surface);
        }
        self.unmanaged_surfaces
            .retain(|surface| !removed_surfaces.contains(surface));
        if self
            .focus_to_apply
            .is_some_and(|(_, surface)| removed_surfaces.contains(&surface))
        {
            self.focus_to_apply = None;
        }
        if let Some(pending) = self.pending.as_mut() {
            pending
                .layers
                .retain(|layer| !removed_surfaces.contains(&layer.surface));
            pending
                .requested_sizes
                .retain(|surface, _| !removed_surfaces.contains(surface));
            if pending
                .focus
                .is_some_and(|surface| removed_surfaces.contains(&surface))
            {
                pending.focus = None;
            }
        }
    }

    fn take_unmanaged_surfaces(&mut self) -> Vec<SurfaceId> {
        std::mem::take(&mut self.unmanaged_surfaces)
            .into_iter()
            .collect()
    }

    fn stage(
        &mut self,
        mut proposal: LiveWmProposal,
        control_sender: &SyncSender<XAuthorityClientControlCommand>,
        control_ack_receiver: &Receiver<XAuthorityClientControlAck>,
    ) -> Result<Option<WmTransactionUpdate>, Box<dyn std::error::Error>> {
        if self.pending.is_some() {
            return Err("live WM attempted to overlap resize transactions".into());
        }
        proposal
            .requested_sizes
            .retain(|surface, size| self.resize.committed_size(*surface) != Some(*size));
        for (surface, size) in &proposal.requested_sizes {
            let client = self
                .client_routes
                .client_for_surface(*surface)
                .ok_or("live WM configure has no X11 client route for its surface")?;
            control_sender.try_send(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::ConfigureSurface {
                    transaction: proposal.transaction,
                    surface: *surface,
                    size: *size,
                },
            })?;
        }
        for _ in 0..proposal.requested_sizes.len() {
            let acknowledgement = control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
            let expected_client = self
                .client_routes
                .client_for_surface(acknowledgement.acknowledgement.surface);
            if acknowledgement.acknowledgement.transaction != proposal.transaction
                || acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered
                || expected_client != Some(acknowledgement.client)
            {
                return Err(format!(
                    "X Authority rejected WM configure transaction {} for surface {:?}: {:?}",
                    acknowledgement.acknowledgement.transaction.raw(),
                    acknowledgement.acknowledgement.surface,
                    acknowledgement.acknowledgement.outcome
                )
                .into());
            }
        }
        let ready = proposal
            .requested_sizes
            .iter()
            .all(|(surface, size)| self.resize.committed_size(*surface) == Some(*size));
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
            staged_transactions: BTreeMap::new(),
            staged_cpu_buffer_updates: Vec::new(),
        });
        Ok(None)
    }

    fn resolve_pending(&mut self) -> Option<WmTransactionUpdate> {
        let pending = self.pending.as_ref()?;
        let ready = pending.requested_sizes.iter().all(|(surface, size)| {
            pending
                .staged_transactions
                .get(surface)
                .is_some_and(|transaction| {
                    transaction.target_geometry.width == size.width
                        && transaction.target_geometry.height == size.height
                })
        });
        if !ready {
            return None;
        }
        let pending = self.pending.take().expect("checked above");
        Some(self.commit_pending(pending))
    }

    fn expire_pending(
        &mut self,
        control_sender: &SyncSender<XAuthorityClientControlCommand>,
        control_ack_receiver: &Receiver<XAuthorityClientControlAck>,
    ) -> Result<Option<WmTransactionUpdate>, Box<dyn std::error::Error>> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.deadline)
        {
            return Ok(None);
        }
        let pending = self.pending.take().expect("checked above");
        let rollback = self
            .resize
            .begin_rollback(pending.requested_sizes.keys().copied())?;
        let rollback_transaction = rollback
            .first()
            .map(|request| request.transaction)
            .unwrap_or(pending.transaction);
        for request in rollback {
            let surface = request.surface;
            let size = request.size;
            let client = self
                .client_routes
                .client_for_surface(surface)
                .ok_or("live WM rollback has no X11 client route")?;
            control_sender.try_send(XAuthorityClientControlCommand {
                client,
                command: XAuthorityControlCommand::ConfigureSurface {
                    transaction: rollback_transaction,
                    surface,
                    size,
                },
            })?;
        }
        for _ in 0..pending.requested_sizes.len() {
            let acknowledgement = control_ack_receiver.recv_timeout(Duration::from_millis(500))?;
            if acknowledgement.acknowledgement.transaction != rollback_transaction
                || acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered
                || self
                    .client_routes
                    .client_for_surface(acknowledgement.acknowledgement.surface)
                    != Some(acknowledgement.client)
            {
                return Err("X Authority rejected live WM rollback configure".into());
            }
        }
        let resize_state = pending
            .requested_sizes
            .iter()
            .map(|(surface, expected)| {
                let observed = self.resize.committed_size(*surface).unwrap_or(Size {
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
            "sophia_live_wm schema=1 status=layout_timeout transaction={} preserved_layout=true rollback_transaction={} rollback_configures={} resize_state={}",
            pending.transaction.raw(),
            rollback_transaction.raw(),
            pending.requested_sizes.len(),
            resize_state,
        );
        Ok(Some(WmTransactionUpdate {
            commit: TransactionCommit {
                transaction: pending.transaction,
                outcome: TransactionOutcome::TimedOut,
                applied_surfaces: Vec::new(),
            },
            ipc_error: None,
        }))
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
            staged_transactions: BTreeMap::new(),
            staged_cpu_buffer_updates: Vec::new(),
        };
        self.commit_pending(pending)
    }

    fn commit_pending(&mut self, pending: PendingLiveWmLayout) -> WmTransactionUpdate {
        if !pending.staged_transactions.is_empty() {
            for transaction in pending.staged_transactions.values() {
                self.resize.record_committed(
                    transaction.surface,
                    Size {
                        width: transaction.target_geometry.width,
                        height: transaction.target_geometry.height,
                    },
                );
            }
            self.committed_resize_replay = Some((
                pending.staged_transactions.values().cloned().collect(),
                pending.staged_cpu_buffer_updates.clone(),
            ));
        }
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
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> XAuthorityObservedTransactionBatch {
        let mut projected = batch.clone();
        if let Some(pending) = self.pending.as_ref() {
            let staged_surfaces = pending
                .staged_transactions
                .keys()
                .copied()
                .collect::<BTreeSet<_>>();
            let staged_handles = pending
                .staged_transactions
                .values()
                .filter_map(|transaction| match transaction.target_buffer {
                    BufferSource::CpuBuffer { handle } => Some(handle),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            projected
                .transactions
                .retain(|transaction| !staged_surfaces.contains(&transaction.surface));
            projected
                .cpu_buffer_updates
                .retain(|update| !staged_handles.contains(&update.handle()));
        }
        let rollback_surfaces = self.resize.rollback_surfaces().collect::<BTreeSet<_>>();
        let rollback_handles = projected
            .transactions
            .iter()
            .filter(|transaction| rollback_surfaces.contains(&transaction.surface))
            .filter_map(|transaction| match transaction.target_buffer {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        projected
            .transactions
            .retain(|transaction| !rollback_surfaces.contains(&transaction.surface));
        projected
            .cpu_buffer_updates
            .retain(|update| !rollback_handles.contains(&update.handle()));
        if let Some((transactions, updates)) = self.committed_resize_replay.take() {
            let surfaces = transactions
                .iter()
                .map(|transaction| transaction.surface)
                .collect::<BTreeSet<_>>();
            let handles = updates
                .iter()
                .map(XAuthorityCpuBufferUpdate::handle)
                .collect::<BTreeSet<_>>();
            projected
                .transactions
                .retain(|transaction| !surfaces.contains(&transaction.surface));
            projected
                .cpu_buffer_updates
                .retain(|update| !handles.contains(&update.handle()));
            projected.transactions.extend(transactions);
            projected.cpu_buffer_updates.extend(updates);
        }
        for transaction in &mut projected.transactions {
            if let Some(layer) = self.layers.get(&transaction.surface) {
                transaction.target_geometry = layer.geometry;
            }
        }
        projected
    }
}

fn center_geometry_without_scaling(mut geometry: Rect, output: Size) -> Rect {
    geometry.x = output.width.saturating_sub(geometry.width).max(0) / 2;
    geometry.y = output.height.saturating_sub(geometry.height).max(0) / 2;
    geometry
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

fn successful_primary_exit_ends_session(input_proof_requested: bool) -> bool {
    !input_proof_requested
}

fn global_runtime_deadline_ends_session(input_proof_requested: bool) -> bool {
    !input_proof_requested
}

fn physical_input_may_route_after_primary_exit(
    primary_child_exited: bool,
    focused_surface: Option<SurfaceId>,
    proof_surface: Option<SurfaceId>,
) -> bool {
    !primary_child_exited || focused_surface != proof_surface
}

fn authority_transaction_count(transactions: &[SurfaceTransaction]) -> usize {
    transactions.len()
}

fn record_runtime_commits(committed: u64, accepted_transactions: usize) -> u64 {
    committed.saturating_add(u64::try_from(accepted_transactions).unwrap_or(u64::MAX))
}

fn physical_input_pixels_already_changed(
    baseline_checksum: Option<u64>,
    current_checksum: Option<u64>,
    input_surface_changed: bool,
) -> bool {
    input_surface_changed
        && baseline_checksum
            .zip(current_checksum)
            .is_some_and(|(baseline, current)| baseline != current)
}

fn run_session_loop(
    config: &PersistentXtermSessionConfig,
    authority_receiver: &Receiver<XAuthorityObservedTransactionBatch>,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    control_sender: &SyncSender<XAuthorityClientControlCommand>,
    control_ack_receiver: &Receiver<XAuthorityClientControlAck>,
    input_delivery_receiver: &Receiver<XAuthorityClientInputDelivery>,
    child: &mut Child,
    secondary_children: &mut [Child],
    physical_input: &mut Option<SessionPhysicalInput>,
    native_scanout: &mut Option<PersistentNativeScanout>,
    wm_session: &mut Option<LiveWmSession>,
    protocol_router: XServerFrontendProtocolRouter,
    input_proof_result: Option<&LiveInputProofResult>,
    require_startup_focus: bool,
    mut initial_authority_batch: Option<XAuthorityObservedTransactionBatch>,
    output_notifications: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let deadline = config.max_runtime.map(|duration| started + duration);
    let outputs = native_scanout
        .as_ref()
        .map(PersistentNativeScanout::outputs)
        .unwrap_or_else(|| vec![sophia_engine::HeadlessOutput::deterministic()]);
    let output = outputs[0];
    let mut scene = PersistentCpuScene::new(output.size);
    let mut layout = PersistentLiveLayout::new(
        wm_session.is_some(),
        require_startup_focus.then_some(output.size),
    );
    let mut runtime: Option<PersistentBackendRuntime> = None;
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
    let mut input_surface = None;
    let mut input_surface_generation = None;
    let mut input_surface_pixel_change = false;
    let mut input_proof_started_at = None;
    let mut input_change_submission_baseline = None;
    let mut input_presented_latency = None;
    let mut pointer_checksum = None;
    let mut pointer_phase_started_at = None;
    let mut pointer_pixel_change = false;
    let mut batches = 0usize;
    let mut transactions = 0usize;
    let mut cpu_buffer_updates = 0usize;
    let mut input_batch_baseline = None;
    let mut input_cpu_update_baseline = None;
    let mut backend_ticks = 0usize;
    let mut runtime_committed = 0u64;
    let mut runtime_surfaces = 0u64;
    let mut focus = InputFocusState::new();
    let mut modifiers = XCoreKeyboardMapper::new();
    let mut emergency_chord = EmergencyChordState::armed();
    let mut pointer = SessionPointerPlacement::default();
    let mut physical_events = 0usize;
    let mut physical_keys_routed = 0usize;
    let mut physical_pointer_events = 0usize;
    let mut physical_pointer_routed = 0usize;
    let mut session_ticks = 0usize;
    let seat = SeatId::from_raw(SESSION_SEAT_RAW);
    let mut focus_deadline_started_at = None;
    let mut focus_ready_reported = false;
    let mut focus_ready_at: Option<Instant> = None;
    let mut focused_client_ready = wm_session.is_some();
    let mut focused_client_control: Option<(TransactionId, SurfaceId)> = None;
    let mut next_focus_control_transaction = 1_000_000u64;
    let mut resize_proof: Option<(TransactionId, SurfaceId, Size)> = None;
    let mut resize_proof_complete = false;
    let mut key_observed_reported = false;
    let mut key_routed_reported = false;
    let mut max_compose = Duration::ZERO;
    let mut next_input_delivery = 1u64;
    let mut pending_input_deliveries = BTreeSet::new();
    let mut input_events_expected = 0usize;
    let mut input_events_flushed = 0usize;
    let mut input_delivery_wait_started_at: Option<Instant> = None;
    let mut input_delivery_source: Option<&'static str> = None;
    let mut input_flush_latency: Option<Duration> = None;
    let mut post_input_deadline: Option<Instant> = None;
    let mut terminal_content_ready = false;
    let mut startup_ready_msec = None;
    let mut terminal_content_ready_reported = false;
    let mut input_text_match = false;
    let mut primary_child_exited = false;

    macro_rules! drain_physical_input {
        () => {{
            let mut emergency_exit = false;
            if let (Some(poller), Some(runtime)) = (physical_input.as_mut(), runtime.as_ref())
                && (config.expect_physical_text.is_none() || physical_input_ready_at.is_some())
            {
                let report = route_physical_input(
                    poller,
                    &focus,
                    runtime.committed_surfaces(),
                    &runtime.input_layers(),
                    &layout.client_routes,
                    input_sender,
                    &mut modifiers,
                    &mut emergency_chord,
                    &mut pointer,
                    !config.expect_physical_pointer || pointer_checksum.is_some(),
                    config.expect_physical_pointer,
                    &mut next_input_delivery,
                    physical_text_proof.as_mut(),
                )?;
                physical_events = physical_events.saturating_add(report.events);
                physical_keys_routed = physical_keys_routed.saturating_add(report.keys_routed);
                physical_pointer_events =
                    physical_pointer_events.saturating_add(report.pointer_events);
                physical_pointer_routed =
                    physical_pointer_routed.saturating_add(report.pointer_routed);
                input_events_expected =
                    input_events_expected.saturating_add(report.deliveries.len());
                pending_input_deliveries.extend(report.deliveries.iter().copied());
                if !key_observed_reported && report.keys_observed > 0 {
                    println!("sophia_live_session_input_pipeline schema=1 status=key_observed");
                    std::io::stdout().flush()?;
                    key_observed_reported = true;
                }
                if !key_routed_reported && report.keys_routed > 0 {
                    println!("sophia_live_session_input_pipeline schema=1 status=key_routed");
                    std::io::stdout().flush()?;
                    key_routed_reported = true;
                }
                if report.emergency_exit {
                    println!("sophia_live_session_input_pipeline schema=1 status=emergency_exit");
                    std::io::stdout().flush()?;
                    emergency_exit = true;
                }
                if physical_sequence_completed_at.is_none()
                    && physical_text_proof
                        .as_ref()
                        .is_some_and(|proof| proof.is_complete())
                {
                    let completed_at = Instant::now();
                    physical_sequence_completed_at = Some(completed_at);
                    input_delivery_wait_started_at = Some(completed_at);
                    input_delivery_source = Some("physical");
                    // Keep the baseline captured immediately before physical
                    // input became ready. Xterm can render the earlier letters
                    // before the poller observes Return; rebasing here discards
                    // that causal pixel evidence and can falsely report a
                    // static terminal after exact text delivery succeeded.
                    if physical_input_pixels_already_changed(
                        injection_checksum,
                        scene.last_report.as_ref().map(|report| report.checksum),
                        input_surface_pixel_change,
                    ) {
                        input_pixel_change = true;
                    }
                }
                if report.pointer_events > 0 {
                    println!(
                        "sophia_live_session_pointer schema=1 status=observed events={} routed={}",
                        report.pointer_events, report.pointer_routed
                    );
                    std::io::stdout().flush()?;
                }
            }
            emergency_exit
        }};
    }

    macro_rules! drain_input_deliveries {
        () => {{
            while let Ok(delivery) = input_delivery_receiver.try_recv() {
                if !pending_input_deliveries.remove(&delivery.delivery) {
                    continue;
                }
                match delivery.outcome {
                    XAuthorityInputDeliveryOutcome::Flushed => {
                        input_events_flushed = input_events_flushed.saturating_add(1);
                    }
                    XAuthorityInputDeliveryOutcome::RouteRejected
                    | XAuthorityInputDeliveryOutcome::WriteFailed => {
                        return Err(format!(
                            "persistent live session X11 input delivery failed: outcome={:?} client={}",
                            delivery.outcome,
                            delivery.client.raw(),
                        )
                        .into());
                    }
                }
            }
            if let Some(wait_started) = input_delivery_wait_started_at
                && !pending_input_deliveries.is_empty()
                && wait_started.elapsed() >= Duration::from_millis(SESSION_INPUT_DELIVERY_TIMEOUT_MSEC)
            {
                return Err(format!(
                    "persistent live session timed out waiting for X11 input delivery: expected={input_events_expected} flushed={input_events_flushed} pending={}",
                    pending_input_deliveries.len(),
                )
                .into());
            }
            if input_delivery_wait_started_at.is_some()
                && pending_input_deliveries.is_empty()
                && input_proof_started_at.is_none()
            {
                let flushed_at = Instant::now();
                input_flush_latency = input_delivery_wait_started_at
                    .map(|started| flushed_at.saturating_duration_since(started));
                input_proof_started_at = Some(flushed_at);
                post_input_deadline = Some(
                    flushed_at + Duration::from_millis(SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC),
                );
                println!(
                    "sophia_live_session_input_pipeline schema=2 status=key_flushed source={} expected={} flushed={}",
                    input_delivery_source.unwrap_or("unknown"),
                    input_events_expected,
                    input_events_flushed,
                );
                std::io::stdout().flush()?;
            }
        }};
    }

    loop {
        if !primary_child_exited && let Some(status) = child.try_wait()? {
            if status.success()
                && successful_primary_exit_ends_session(config.input_proof_requested())
            {
                break;
            }
            if !status.success() {
                return Err(
                    format!("xterm exited during live session with status {status}").into(),
                );
            }
            // The proof helper intentionally exits after displaying its
            // received text. Keep the session and secondary terminal alive so
            // the final native frame can retire and pointer evidence can run.
            primary_child_exited = true;
        }
        for (index, secondary) in secondary_children.iter_mut().enumerate() {
            if let Some(status) = secondary.try_wait()? {
                return Err(format!(
                    "secondary xterm {} exited during live session with status {status}",
                    index + 1
                )
                .into());
            }
        }
        drain_input_deliveries!();
        if !input_text_match
            && let (Some(expected), Some(result)) = (
                config
                    .inject_text
                    .as_deref()
                    .or(config.expect_physical_text.as_deref()),
                input_proof_result,
            )
            && let Some(received) = result.received()?
        {
            if received != expected.as_bytes() {
                return Err(format!(
                    "persistent live session terminal received incorrect input: expected_bytes={} received_bytes={}",
                    expected.len(),
                    received.len(),
                )
                .into());
            }
            input_text_match = true;
            println!(
                "sophia_live_session_input schema=3 status=semantic_complete source={} text_match=true bytes={}",
                if config.inject_text.is_some() {
                    "synthetic"
                } else {
                    "physical"
                },
                received.len(),
            );
            std::io::stdout().flush()?;
        }
        if let Some(post_input_deadline) = post_input_deadline
            && Instant::now() >= post_input_deadline
            && !input_text_match
        {
            return Err(
                "persistent live session timed out waiting for the terminal to receive exact text and Return"
                    .into(),
            );
        }
        if input_presented_latency.is_none()
            && let Some(post_input_deadline) = post_input_deadline
            && Instant::now() >= post_input_deadline
        {
            if !input_pixel_change {
                return Err(format!(
                    "persistent live session timed out waiting for pixels after flushed X11 input: expected={input_events_expected} flushed={input_events_flushed} authority_batches_after_input={} cpu_updates_after_input={} baseline_checksum={injection_checksum:?} final_checksum={:?} baseline_generation={input_surface_generation:?} final_generation={:?} input_surface_pixel_change={input_surface_pixel_change} native_submission_baseline={input_change_submission_baseline:?} native_submissions={} native_callbacks={}",
                    batches.saturating_sub(input_batch_baseline.unwrap_or(batches)),
                    cpu_buffer_updates.saturating_sub(input_cpu_update_baseline.unwrap_or(cpu_buffer_updates)),
                    scene.last_report.as_ref().map(|report| report.checksum),
                    input_surface.and_then(|surface| scene.surface_buffer_generation(surface)),
                    native_scanout.as_ref().map_or(0, |native| native.submissions),
                    native_scanout.as_ref().map_or(0, |native| native.callback_accepted),
                )
                .into());
            }
            return Err("persistent live session input pixels were not presented within the post-flush proof window".into());
        }
        if (post_input_deadline.is_none() || input_presented_latency.is_some())
            && deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            if config.input_proof_requested() && injection_checksum.is_none() {
                return Err(
                    "persistent live session startup budget elapsed before a focused terminal frame was ready for input proof"
                        .into(),
                );
            }
            // The global runtime budget bounds startup. Once input has been
            // injected, its delivery and pixel/semantic stages own narrower
            // explicit deadlines. Ending here can strand already-routed keys
            // without giving the frontend a chance to acknowledge them.
            if global_runtime_deadline_ends_session(config.input_proof_requested()) {
                break;
            }
        }
        if physical_input_may_route_after_primary_exit(
            primary_child_exited,
            focus.focused_surface(seat),
            input_surface,
        ) && drain_physical_input!()
        {
            break;
        }
        if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut())
            && (runtime.native_scanout_in_flight() || runtime.native_cleanup_pending())
        {
            runtime.retire_native_scanout(native_scanout)?;
        }
        if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut())
            && !runtime.queued_gpu_presentations.is_empty()
            && !runtime.native_scanout_in_flight()
        {
            let _ = runtime.drive_gpu_presentation(Some(native_scanout))?;
        }
        let input_baseline_presented_before_wait =
            scene.last_report.as_ref().is_some_and(|report| {
                report.nonzero_pixel_bytes > 0
                    && native_scanout.as_ref().is_none_or(|native| {
                        native.heads.first().is_some_and(|head| {
                            head.presented_checksum == report.checksum && head.nonzero_exports > 0
                        })
                    })
            });
        if input_presented_latency.is_none()
            && input_pixel_change
            && let Some(started) = input_proof_started_at
            && native_scanout.as_ref().is_none_or(|native| {
                input_change_submission_baseline.is_some_and(|baseline| {
                    native
                        .heads
                        .first()
                        .is_some_and(|head| head.presented_submissions > baseline)
                })
            })
        {
            input_presented_latency = Some(started.elapsed());
        }
        if require_startup_focus
            && focus.focused_surface(seat).is_none()
            && focus_deadline_started_at
                .is_some_and(|started: Instant| started.elapsed() >= Duration::from_secs(5))
        {
            return Err(
                "live-session input focus was not ready within five seconds of the first presented frame"
                    .into(),
            );
        }
        let physical_sequence_complete = physical_text_proof
            .as_ref()
            .is_none_or(PhysicalTextProof::is_complete);
        let waiting_for_keyboard_sequence =
            physical_input_ready_at.is_some() && !physical_sequence_complete;
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
        } else if waiting_for_pointer_pixels {
            let started_at = pointer_phase_started_at.expect("set above");
            if started_at.elapsed() >= Duration::from_millis(SESSION_PHYSICAL_PIXEL_TIMEOUT_MSEC) {
                return Err(format!(
                    "persistent live session timed out waiting for physical pointer pixels: pointer_observed={physical_pointer_events} pointer_routed={physical_pointer_routed} pointer_baseline={pointer_checksum:?} final_checksum={:?}",
                    scene.last_report.as_ref().map(|report| report.checksum)
                )
                .into());
            }
        } else if input_delivery_wait_started_at.is_none()
            && (input_proof_started_at.is_none() || input_presented_latency.is_some())
        {
            if config
                .max_ticks
                .is_some_and(|max_ticks| session_ticks >= max_ticks)
            {
                break;
            }
            session_ticks = session_ticks.saturating_add(1);
        }

        let authority_batch = initial_authority_batch.take().map_or_else(
            || authority_receiver.recv_timeout(Duration::from_millis(25)),
            Ok,
        );
        match authority_batch {
            Ok(batch) => {
                last_authority_update = Instant::now();
                batches = batches.saturating_add(1);
                transactions =
                    transactions.saturating_add(authority_transaction_count(&batch.transactions));
                cpu_buffer_updates =
                    cpu_buffer_updates.saturating_add(batch.cpu_buffer_updates.len());
                let removed_surfaces = batch.removed_surfaces.clone();
                let _ = layout.observe_authority_batch(&batch);
                let mut wm_update = layout.resolve_pending();
                if !resize_proof_complete
                    && let Some((transaction, surface, size)) = resize_proof
                    && layout.pending.is_none()
                    && layout.resize.committed_size(surface) == Some(size)
                {
                    println!(
                        "sophia_live_resize schema=1 status=committed transaction={} surface={} width={} height={} configure_ack=true pixels=true",
                        transaction.raw(),
                        surface.index(),
                        size.width,
                        size.height,
                    );
                    resize_proof_complete = true;
                }
                if wm_update.is_none() {
                    wm_update = layout.expire_pending(control_sender, control_ack_receiver)?;
                }
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
                if resize_proof.is_none()
                    && let Some(size) = config.inject_surface_resize
                    && layout.layers.len() >= if config.secondary_terminal { 2 } else { 1 }
                    && layout.pending.is_none()
                {
                    let surface = layout
                        .layers
                        .keys()
                        .next()
                        .copied()
                        .ok_or("surface resize proof has no target")?;
                    let transaction = TransactionId::from_raw(2_000_000);
                    let mut layers = layout.layers.values().cloned().collect::<Vec<_>>();
                    let layer = layers
                        .iter_mut()
                        .find(|layer| layer.surface == surface)
                        .ok_or("surface resize proof lost its target")?;
                    layer.geometry.width = size.width;
                    layer.geometry.height = size.height;
                    let proposal = LiveWmProposal {
                        transaction,
                        layers,
                        requested_sizes: BTreeMap::from([(surface, size)]),
                        focus: None,
                        timeout: Duration::from_secs(2),
                        update: WmTransactionUpdate {
                            commit: TransactionCommit {
                                transaction,
                                outcome: TransactionOutcome::Committed,
                                applied_surfaces: vec![surface],
                            },
                            ipc_error: None,
                        },
                        moved_surfaces: 0,
                    };
                    wm_update = layout.stage(proposal, control_sender, control_ack_receiver)?;
                    resize_proof = Some((transaction, surface, size));
                    println!(
                        "sophia_live_resize schema=1 status=requested transaction={} surface={} width={} height={}",
                        transaction.raw(),
                        surface.index(),
                        size.width,
                        size.height,
                    );
                }
                let batch = layout.projected_batch(&batch);
                scene.observe(&batch)?;
                let compose_started = Instant::now();
                let report = scene.compose()?.clone();
                max_compose = max_compose.max(compose_started.elapsed());
                if let (Some(surface), Some(before_surface)) =
                    (input_surface, input_surface_generation)
                    && scene
                        .surface_buffer_generation(surface)
                        .is_some_and(|generation| generation != before_surface)
                {
                    input_surface_pixel_change = true;
                }
                let native_frames = native_scanout
                    .as_ref()
                    .map(|_| scene.frames_for_outputs(&outputs))
                    .transpose()?;
                if let Some(before_frame) = injection_checksum
                    && report.checksum != before_frame
                    && (config.expect_physical_text.is_none()
                        || physical_sequence_completed_at.is_some())
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
                    runtime = Some(
                        PersistentBackendRuntime::new(
                            &outputs,
                            &batch.transactions,
                            native_scanout.as_mut(),
                            native_frames.clone(),
                        )?
                        .with_protocol_router(protocol_router.clone())
                        .with_m4_proof_controls(
                            config.m4_first_acquire_delay,
                            config.m4_reject_first_present,
                            config.m4_diagnose_first_mixed_export,
                        ),
                    );
                }
                let runtime = runtime
                    .as_mut()
                    .expect("persistent backend runtime was initialized above");
                let tick =
                    runtime.run_batch(&batch, native_scanout.as_mut(), native_frames, wm_update)?;
                backend_ticks = backend_ticks.saturating_add(1);
                runtime_committed = record_runtime_commits(
                    runtime_committed,
                    authority_transaction_count(&batch.transactions),
                );
                runtime_surfaces = tick.engine.runtime.runtime_state.authority_surfaces_applied;
                for surface in removed_surfaces {
                    focus.clear_surface(surface);
                }
                if focus.focused_surface(seat).is_none()
                    && let Some(surface) = runtime.committed_surfaces().first()
                {
                    if focus.focus_surface(seat, surface.surface, runtime.committed_surfaces())
                        == InputFocusDecision::Focused
                        && wm_session.is_none()
                    {
                        let client = layout
                            .client_routes
                            .client_for_surface(surface.surface)
                            .ok_or("initial X11 focus has no client route")?;
                        let transaction = TransactionId::from_raw(next_focus_control_transaction);
                        next_focus_control_transaction = next_focus_control_transaction
                            .checked_add(1)
                            .ok_or("initial X11 focus transaction exhausted")?;
                        control_sender
                            .try_send(XAuthorityClientControlCommand {
                                client,
                                command: XAuthorityControlCommand::FocusSurface {
                                    transaction,
                                    surface: surface.surface,
                                },
                            })
                            .map_err(|error| match error {
                                TrySendError::Full(_) => "initial X11 focus control queue is full",
                                TrySendError::Disconnected(_) => {
                                    "initial X11 focus control queue is disconnected"
                                }
                            })?;
                        focused_client_control = Some((transaction, surface.surface));
                        scene.raise_surface(surface.surface);
                    }
                }
                if let Some((transaction, surface)) = layout.focus_to_apply.take()
                    && focus.focus_surface(seat, surface, runtime.committed_surfaces())
                        == InputFocusDecision::Focused
                {
                    scene.raise_surface(surface);
                    println!(
                        "sophia_live_wm schema=1 status=focus_committed transaction={} target=surface",
                        transaction.raw()
                    );
                }
                if !focus_ready_reported && focus.focused_surface(seat).is_some() {
                    println!("sophia_live_session_input_pipeline schema=1 status=focus_ready");
                    std::io::stdout().flush()?;
                    focus_ready_reported = true;
                    focus_ready_at = Some(Instant::now());
                }
                if !terminal_content_ready
                    && let Some(surface) = focus.focused_surface(seat)
                    && scene.surface_has_visual_detail(surface)
                {
                    terminal_content_ready = true;
                    startup_ready_msec = Some(started.elapsed().as_millis());
                    if !terminal_content_ready_reported {
                        println!(
                            "sophia_live_session_input_pipeline schema=1 status=terminal_content_ready"
                        );
                        std::io::stdout().flush()?;
                        terminal_content_ready_reported = true;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let _ = layout.expire_pending(control_sender, control_ack_receiver)?;
                if let Some(wm_session) = wm_session.as_mut()
                    && let Some(proposal) = wm_session.poll_restart(&layout, output)?
                {
                    let _ = layout.stage(proposal, control_sender, control_ack_receiver)?;
                }
                if layout.pending.is_none()
                    && last_authority_update.elapsed()
                        >= Duration::from_millis(config.input_quiet_msec)
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
                    if runtime.native_scanout_in_flight() || runtime.native_cleanup_pending() {
                        runtime.retire_native_scanout(native_scanout)?;
                    }
                    if !runtime.queued_gpu_presentations.is_empty()
                        && !runtime.native_scanout_in_flight()
                    {
                        let _ = runtime.drive_gpu_presentation(Some(native_scanout))?;
                    }
                    if !runtime.native_scanout_in_flight()
                        && native_scanout.heads.iter().any(|head| {
                            head.exporter.pending_cpu_frame() || head.exporter.pending_mixed_frame()
                        })
                    {
                        let tick = runtime.run_native_idle(native_scanout)?;
                        backend_ticks = backend_ticks.saturating_add(1);
                        runtime_surfaces =
                            tick.engine.runtime.runtime_state.authority_surfaces_applied;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("persistent X authority transaction channel disconnected".into());
            }
        }

        if !physical_input_completion_reported
            && input_pixel_change
            && input_text_match
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

        if wm_session.is_none() {
            while let Ok(acknowledgement) = control_ack_receiver.try_recv() {
                let Some((transaction, surface)) = focused_client_control else {
                    continue;
                };
                if acknowledgement.acknowledgement.transaction != transaction
                    || acknowledgement.acknowledgement.surface != surface
                {
                    continue;
                }
                if acknowledgement.acknowledgement.outcome != XAuthorityControlOutcome::Delivered {
                    return Err(format!(
                        "initial X11 focus control was rejected: {:?}",
                        acknowledgement.acknowledgement.outcome
                    )
                    .into());
                }
                focused_client_control = None;
                focused_client_ready = true;
                println!(
                    "sophia_live_session_input_pipeline schema=1 status=focus_applied source=x11-control"
                );
                std::io::stdout().flush()?;
            }
        }

        let input_baseline_presented = input_baseline_presented_before_wait
            || scene.last_report.as_ref().is_some_and(|report| {
                report.nonzero_pixel_bytes > 0
                    && native_scanout.as_ref().is_none_or(|native| {
                        native.heads.first().is_some_and(|head| {
                            head.presented_checksum != 0 && head.nonzero_exports > 0
                        })
                    })
            });
        let input_start_stable = if config.expect_physical_text.is_some() {
            focus_ready_at.is_some_and(|ready| ready.elapsed() >= Duration::from_secs(2))
        } else {
            last_authority_update.elapsed() >= Duration::from_millis(config.input_quiet_msec)
                || wm_session.as_ref().is_some_and(|wm| {
                    wm.last_committed_at.is_some_and(|committed| {
                        committed.elapsed() >= Duration::from_millis(config.input_quiet_msec)
                    })
                })
        };
        if require_startup_focus
            && physical_input.is_some()
            && input_baseline_presented
            && focus_deadline_started_at.is_none()
        {
            focus_deadline_started_at = Some(Instant::now());
        }
        if injection_checksum.is_none()
            && config.input_proof_requested()
            && input_baseline_presented
            && input_start_stable
            && focused_client_ready
            && terminal_content_ready
        {
            injection_checksum = scene.last_report.as_ref().map(|report| report.checksum);
            input_change_submission_baseline = native_scanout
                .as_ref()
                .and_then(|native| native.heads.first())
                .map(|head| head.presented_submissions);
            input_surface = focus.focused_surface(seat);
            input_surface_generation =
                input_surface.and_then(|surface| scene.surface_buffer_generation(surface));
            if let Some(text) = config.inject_text.as_deref() {
                let events = synthetic_text_input_events(text)?;
                let expected = events.len();
                let runtime = runtime
                    .as_ref()
                    .ok_or("synthetic routed input requires an initialized runtime")?;
                let report = route_input_events(
                    events,
                    &focus,
                    runtime.committed_surfaces(),
                    &runtime.input_layers(),
                    &layout.client_routes,
                    input_sender,
                    &mut modifiers,
                    &mut emergency_chord,
                    &mut pointer,
                    false,
                    false,
                    &mut next_input_delivery,
                    None,
                )?;
                if report.keys_routed != expected {
                    return Err(format!(
                        "synthetic input did not traverse committed Engine focus: expected={expected} routed={}",
                        report.keys_routed
                    )
                    .into());
                }
                input_events_expected =
                    input_events_expected.saturating_add(report.deliveries.len());
                pending_input_deliveries.extend(report.deliveries.iter().copied());
                input_delivery_wait_started_at = Some(Instant::now());
                input_delivery_source = Some("synthetic");
                input_batch_baseline = Some(batches);
                input_cpu_update_baseline = Some(cpu_buffer_updates);
                if !key_routed_reported {
                    println!(
                        "sophia_live_session_input_pipeline schema=1 status=key_routed source=synthetic"
                    );
                    std::io::stdout().flush()?;
                    key_routed_reported = true;
                }
            } else {
                input_batch_baseline = Some(batches);
                input_cpu_update_baseline = Some(cpu_buffer_updates);
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
        {
            pointer_checksum = scene.last_report.as_ref().map(|report| report.checksum);
            println!(
                "sophia_live_session_pointer schema=1 status=ready source=physical action=select"
            );
            std::io::stdout().flush()?;
        }
        if input_presented_latency.is_none()
            && input_pixel_change
            && let Some(started) = input_proof_started_at
            && native_scanout.as_ref().is_none_or(|native| {
                input_change_submission_baseline.is_some_and(|baseline| {
                    native
                        .heads
                        .first()
                        .is_some_and(|head| head.presented_submissions > baseline)
                })
            })
        {
            input_presented_latency = Some(started.elapsed());
        }
        if (config.exit_after_input_proof || config.inject_text.is_some())
            && input_presented_latency.is_some()
            && input_text_match
            && (config.expect_physical_text.is_none() || physical_input_completion_reported)
            && (!config.expect_physical_pointer || pointer_pixel_change)
        {
            break;
        }
    }

    if let (Some(runtime), Some(native_scanout)) = (runtime.as_mut(), native_scanout.as_mut()) {
        runtime.drain_native_scanout(native_scanout, Duration::from_secs(2))?;
    }
    if let Some(runtime) = runtime.as_mut() {
        runtime.shutdown_presentations();
    }
    if input_presented_latency.is_none()
        && input_pixel_change
        && let Some(started) = input_proof_started_at
        && native_scanout.as_ref().is_none_or(|native| {
            input_change_submission_baseline.is_some_and(|baseline| {
                native
                    .heads
                    .first()
                    .is_some_and(|head| head.presented_submissions > baseline)
            })
        })
    {
        input_presented_latency = Some(started.elapsed());
    }

    let report = scene
        .last_report
        .as_ref()
        .ok_or("persistent live session received no composable X pixels")?;
    if config.input_proof_requested() && input_events_expected != input_events_flushed {
        return Err(format!(
            "persistent live session completed with unflushed X11 input: expected={input_events_expected} flushed={input_events_flushed} pending={}",
            pending_input_deliveries.len(),
        )
        .into());
    }
    if config.input_proof_requested() && input_flush_latency.is_none() {
        return Err("persistent live session input proof never observed flushed X11 input".into());
    }
    if config.input_proof_requested() && !input_pixel_change {
        return Err(format!(
            "persistent live session input did not change composed terminal pixels: baseline={injection_checksum:?} final_frame={} final_buffers={} input_surface={input_surface:?} input_surface_pixel_change={input_surface_pixel_change} batches={batches} transactions={transactions}",
            report.checksum,
            scene.buffer_checksum(),
        )
        .into());
    }
    if config.input_proof_requested() && input_presented_latency.is_none() {
        let native_heads = runtime.as_ref().map_or_else(
            || "none".to_owned(),
            PersistentBackendRuntime::native_diagnostic,
        );
        return Err(format!(
            "persistent live session input pixels were not presented: change_submission_baseline={input_change_submission_baseline:?} primary_presented_submissions={} native_submissions={} native_callbacks={} native_heads={native_heads}",
            native_scanout
                .as_ref()
                .and_then(|native| native.heads.first())
                .map_or(0, |head| head.presented_submissions),
            native_scanout.as_ref().map_or(0, |native| native.submissions),
            native_scanout
                .as_ref()
                .map_or(0, |native| native.callback_accepted),
        )
        .into());
    }
    if config.input_proof_requested() && !input_text_match {
        return Err(
            "persistent live session terminal did not receive the expected text and Return".into(),
        );
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
    if config.inject_surface_resize.is_some() && !resize_proof_complete {
        return Err(
            "persistent live session did not commit configured surface resize pixels".into(),
        );
    }
    if let Some(wm_session) = wm_session.as_ref()
        && wm_session.committed == 0
    {
        return Err("live session ended without a committed external WM layout".into());
    }
    let input_stats = physical_input
        .as_ref()
        .map_or_else(Default::default, |input| input.stats());
    let (
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_uploads,
        native_max_upload,
    ) = native_scanout.as_ref().map_or(
        (0, 0, 0, 0, Duration::ZERO),
        PersistentNativeScanout::persistent_render_metrics,
    );
    println!(
        "sophia_live_session schema=14 status=bounded_complete display={} elapsed_msec={} startup_ready_msec={} session_ticks={} authority_batches={} authority_transactions={} authority_queue_capacity={} authority_batches_dropped=0 backend_ticks={} runtime_committed={} runtime_surfaces={} cpu_layers={} cpu_nonzero_pixel_bytes={} cpu_max_nonzero_pixel_bytes={} cpu_nonzero_frames={} cpu_checksum={} cpu_max_compose_msec={} injected_input={} input_events_expected={} input_events_flushed={} input_flush_latency_msec={} input_pixel_change={} input_text_match={} input_presented_latency_msec={} input_dispatch_max_gap_msec={} input_queue_max_depth={} input_queue_dwell_max_msec={} physical_events={} physical_keys_routed={} pointer_pixel_change={} physical_pointer_events={} physical_pointer_routed={} pointer_proof={} native_presentation={} native_submissions={} native_submit_deferred={} native_submit_failures={} native_retirements={} native_retire_failures={} native_max_in_flight_ticks={} native_max_submit_to_page_flip_msec={} native_max_upload_msec={} native_target_creations={} native_target_recreations={} native_pipeline_creations={} native_frame_uploads={} native_callback_accepted={} native_callback_rejected={} native_callback_queue_saturated={} native_nonzero_exports={} native_mixed_exports={} native_export_attempts={} native_in_flight={} native_cleanup_pending={} physical_input={} wm_policy={} wm_requests={} wm_committed={} wm_restarts={} wm_degraded={} namespace_profile={} output_update={} output_notifications={} surface_resize={} present_complete_flip={} present_complete_skip={} present_idle={} present_idle_fence_triggers={} present_disconnect_sources={} present_disconnect_fences={} present_disconnect_failures={} present_live_sources={} present_live_fences={} present_live_transactions={} present_acquire_waits={} present_controlled_rejections={}",
        config.display,
        started.elapsed().as_millis(),
        startup_ready_msec.ok_or("persistent live session never reached startup readiness")?,
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
        max_compose.as_millis(),
        config.inject_text.is_some(),
        input_events_expected,
        input_events_flushed,
        input_flush_latency.map_or(0, |duration| duration.as_millis()),
        input_pixel_change,
        input_text_match,
        input_presented_latency
            .map(|latency| latency.as_millis().to_string())
            .unwrap_or_else(|| "none".to_owned()),
        input_stats.max_dispatch_gap_msec,
        input_stats.max_queue_depth,
        input_stats.max_queue_dwell_msec,
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
        native_max_upload.as_millis(),
        native_target_creations,
        native_target_recreations,
        native_pipeline_creations,
        native_uploads,
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
            .map_or(0, PersistentNativeScanout::mixed_exports),
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
        match config.namespace_profile {
            NamespaceProfile::ClassicShared => "classic_shared",
            NamespaceProfile::Confined => "confined",
        },
        if config.inject_output_size.is_some() {
            "applied"
        } else {
            "disabled"
        },
        output_notifications,
        if resize_proof_complete {
            "committed"
        } else {
            "disabled"
        },
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.present_complete_flip),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.present_complete_skip),
        runtime.as_ref().map_or(0, |runtime| runtime.present_idle),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.idle_fence_triggers),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.presentation_disconnect_sources),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.presentation_disconnect_fences),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.presentation_disconnect_failures),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.presentation_resources.source_count()),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.presentation_resources.fence_count()),
        runtime.as_ref().map_or(0, |runtime| {
            runtime.presentation_resources.presentation_count()
        }),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.present_acquire_waits),
        runtime
            .as_ref()
            .map_or(0, |runtime| runtime.present_controlled_rejections),
    );
    if let Some(runtime) = runtime.as_ref()
        && (runtime.presentation_disconnect_failures != 0
            || runtime.presentation_resources.source_count() != 0
            || runtime.presentation_resources.fence_count() != 0
            || runtime.presentation_resources.presentation_count() != 0
            || runtime.present_idle
                != runtime
                    .present_complete_flip
                    .saturating_add(runtime.present_complete_skip))
    {
        return Err("persistent Present resources did not retire exactly once".into());
    }
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

#[derive(Clone)]
struct ObservedComposedFrame {
    frame: sophia_backend_live::LiveCpuComposedFrame,
    checksum: u64,
    nonzero_pixel_bytes: usize,
}

struct PersistentBackendRuntime {
    outputs: BTreeMap<OutputId, PersistentOutputRuntime>,
    layers: BTreeMap<SurfaceId, SurfaceTransaction>,
    committed_snapshot_layers: Option<Vec<LayerSnapshot>>,
    presentation_resources: sophia_backend_live::LivePresentationResourceSession,
    queued_gpu_presentations: VecDeque<QueuedGpuPresentation>,
    submitted_gpu_presentation: Option<SubmittedGpuPresentation>,
    protocol_router: Option<XServerFrontendProtocolRouter>,
    present_complete_flip: usize,
    present_complete_skip: usize,
    present_idle: usize,
    idle_fence_triggers: usize,
    presentation_disconnect_sources: usize,
    presentation_disconnect_fences: usize,
    presentation_disconnect_failures: usize,
    first_acquire_delay: Option<Duration>,
    first_acquire_delay_applied: bool,
    reject_first_present: bool,
    present_acquire_waits: usize,
    present_controlled_rejections: usize,
    diagnose_first_mixed_export: bool,
}

struct QueuedGpuPresentation {
    submission: sophia_backend_live::LivePresentationSubmission,
    transactions: Vec<SurfaceTransaction>,
    cpu_background: Option<sophia_backend_live::LiveCpuComposedFrame>,
    target: Rect,
    deadline: Instant,
    not_before: Instant,
}

struct SubmittedGpuPresentation {
    transaction: TransactionId,
    prepared: BTreeMap<OutputId, sophia_engine::PreparedSurfaceCommit>,
}

impl PersistentBackendRuntime {
    fn new(
        outputs: &[sophia_engine::HeadlessOutput],
        first_transactions: &[SurfaceTransaction],
        native_scanout: Option<&mut PersistentNativeScanout>,
        initial_native_frames: Option<Vec<ObservedComposedFrame>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_committed_surfaces(
            outputs,
            seed_committed_surfaces(first_transactions),
            native_scanout,
            initial_native_frames,
        )
    }

    fn new_from_committed_surfaces(
        outputs: &[sophia_engine::HeadlessOutput],
        committed_surfaces: &[CommittedSurfaceState],
        native_scanout: Option<&mut PersistentNativeScanout>,
        initial_native_frames: Option<Vec<ObservedComposedFrame>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut runtime = Self::new_with_committed_surfaces(
            outputs,
            committed_surfaces.to_vec(),
            native_scanout,
            initial_native_frames,
        )?;
        runtime.committed_snapshot_layers =
            Some(layer_snapshots_from_committed(committed_surfaces));
        Ok(runtime)
    }

    fn new_with_committed_surfaces(
        outputs: &[sophia_engine::HeadlessOutput],
        committed_surfaces: Vec<CommittedSurfaceState>,
        mut native_scanout: Option<&mut PersistentNativeScanout>,
        initial_native_frames: Option<Vec<ObservedComposedFrame>>,
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
                .with_committed_surfaces(committed_surfaces.clone())
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
            committed_snapshot_layers: None,
            presentation_resources: Default::default(),
            queued_gpu_presentations: VecDeque::new(),
            submitted_gpu_presentation: None,
            protocol_router: None,
            present_complete_flip: 0,
            present_complete_skip: 0,
            present_idle: 0,
            idle_fence_triggers: 0,
            presentation_disconnect_sources: 0,
            presentation_disconnect_fences: 0,
            presentation_disconnect_failures: 0,
            first_acquire_delay: None,
            first_acquire_delay_applied: false,
            reject_first_present: false,
            present_acquire_waits: 0,
            present_controlled_rejections: 0,
            diagnose_first_mixed_export: false,
        })
    }

    fn with_protocol_router(mut self, router: XServerFrontendProtocolRouter) -> Self {
        self.protocol_router = Some(router);
        self
    }

    fn with_m4_proof_controls(
        mut self,
        first_acquire_delay: Option<Duration>,
        reject_first_present: bool,
        diagnose_first_mixed_export: bool,
    ) -> Self {
        self.first_acquire_delay = first_acquire_delay;
        self.reject_first_present = reject_first_present;
        self.diagnose_first_mixed_export = diagnose_first_mixed_export;
        self
    }

    fn run_batch(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
        mut native_scanout: Option<&mut PersistentNativeScanout>,
        native_frames: Option<Vec<ObservedComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        self.observe_presentation_resources(batch)?;
        if !batch.present_submissions.is_empty() {
            let cpu_background = native_frames
                .as_ref()
                .and_then(|frames| frames.first())
                .map(|frame| frame.frame.clone());
            for submission in &batch.present_submissions {
                let transaction = batch
                    .transactions
                    .iter()
                    .find(|transaction| transaction.surface == submission.surface)
                    .ok_or("Present submission has no matching Engine transaction")?;
                let live_submission = sophia_backend_live::LivePresentationSubmission {
                    transaction: submission.transaction,
                    buffer: submission.buffer,
                    acquire_fence: submission.acquire_fence,
                    idle_fence: submission.idle_fence,
                };
                self.presentation_resources.begin(live_submission)?;
                // The hardware proof holds the first Present at the acquire
                // gate even when Mesa omits the optional wait-fence handle.
                // When a handle is present, normal fence polling still runs
                // after this bounded scheduling delay.
                let acquire_delay =
                    if !self.first_acquire_delay_applied && self.first_acquire_delay.is_some() {
                        self.first_acquire_delay_applied = true;
                        self.first_acquire_delay.unwrap_or(Duration::ZERO)
                    } else {
                        Duration::ZERO
                    };
                let not_before = Instant::now() + acquire_delay;
                self.queued_gpu_presentations
                    .push_back(QueuedGpuPresentation {
                        submission: live_submission,
                        transactions: batch.transactions.clone(),
                        cpu_background: cpu_background.clone(),
                        target: transaction.target_geometry,
                        deadline: not_before
                            + Duration::from_millis(u64::from(
                                transaction.timeout_msec.clamp(100, 2_000),
                            )),
                        not_before,
                    });
            }
            self.layers
                .retain(|surface, _| !batch.removed_surfaces.contains(surface));
            for transaction in &batch.transactions {
                self.layers.insert(transaction.surface, transaction.clone());
            }
            return self.drive_gpu_presentation(native_scanout.as_deref_mut());
        }
        self.run_authority_transactions(
            batch.transaction,
            &batch.transactions,
            &batch.removed_surfaces,
            authority_transaction_count(&batch.transactions),
            native_scanout,
            native_frames,
            wm_update,
        )
    }

    fn observe_presentation_resources(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for registration in &batch.dma_buf_registrations {
            let plane_fds = registration
                .plane_fds
                .iter()
                .map(|fd| fd.as_ref().try_clone())
                .collect::<Result<Vec<_>, _>>()?;
            self.presentation_resources
                .register_source(registration.descriptor, plane_fds)?;
        }
        for registration in &batch.fence_registrations {
            self.presentation_resources.register_fence(
                registration.handle,
                registration.initially_triggered,
                registration.fd.as_ref().try_clone()?,
            )?;
        }
        for handle in &batch.released_dma_bufs {
            let _ = self.presentation_resources.release_source(*handle);
        }
        for handle in &batch.released_fences {
            let _ = self.presentation_resources.release_fence(*handle);
        }
        Ok(())
    }

    fn drive_gpu_presentation(
        &mut self,
        mut native_scanout: Option<&mut PersistentNativeScanout>,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        if self.submitted_gpu_presentation.is_some() {
            return self.run_observation_tick();
        }
        let Some(queued) = self.queued_gpu_presentations.front() else {
            return self.run_observation_tick();
        };
        let transaction = queued.submission.transaction;
        if Instant::now() < queued.not_before {
            self.present_acquire_waits = self.present_acquire_waits.saturating_add(1);
            return self.run_observation_tick();
        }
        if !self
            .presentation_resources
            .poll_acquire_fence(transaction)?
        {
            self.present_acquire_waits = self.present_acquire_waits.saturating_add(1);
            if Instant::now() >= queued.deadline {
                self.queued_gpu_presentations.pop_front();
                self.reject_gpu_presentation(transaction, 0, 0);
            }
            return self.run_observation_tick();
        }
        if self.reject_first_present {
            self.reject_first_present = false;
            self.present_controlled_rejections =
                self.present_controlled_rejections.saturating_add(1);
            self.queued_gpu_presentations.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
            return self.run_observation_tick();
        }
        let Some(native_scanout) = native_scanout.as_deref_mut() else {
            self.queued_gpu_presentations.pop_front();
            self.reject_gpu_presentation(transaction, 0, 0);
            return self.run_observation_tick();
        };
        if self.native_scanout_in_flight() {
            return self.run_observation_tick();
        }

        let mut prepared = BTreeMap::new();
        for (output_id, output) in &self.outputs {
            let assembly = output.runtime.assembly();
            let transactions =
                sophia_cli::presentation_transaction::rebase_full_state_present_transactions(
                    &queued.transactions,
                    assembly.committed_surfaces(),
                );
            let candidate = assembly.engine().prepare_surface_transactions(
                transaction,
                &transactions,
                assembly.committed_surfaces(),
            );
            if !candidate.is_ready() {
                self.queued_gpu_presentations.pop_front();
                self.reject_gpu_presentation(transaction, 0, 0);
                return self.run_observation_tick();
            }
            prepared.insert(*output_id, candidate);
        }
        let mixed = self.presentation_resources.build_mixed_frame(
            transaction,
            queued.cpu_background.clone(),
            queued.target,
            None,
            1.0,
        )?;
        if self.diagnose_first_mixed_export {
            self.diagnose_first_mixed_export = false;
            let (cpu_layers, dmabuf_layers) =
                mixed
                    .layers
                    .iter()
                    .fold((0usize, 0usize), |(cpu, dmabuf), layer| match layer {
                        sophia_backend_live::LiveOwnedMixedCompositionLayer::Cpu { .. } => {
                            (cpu.saturating_add(1), dmabuf)
                        }
                        sophia_backend_live::LiveOwnedMixedCompositionLayer::DmaBuf { .. } => {
                            (cpu, dmabuf.saturating_add(1))
                        }
                    });
            let (status, detail) = native_scanout.diagnose_mixed_frame(0, mixed);
            self.queued_gpu_presentations.pop_front();
            let _ = self.presentation_resources.reject(transaction);
            let _ = self.presentation_resources.disconnect();
            return Err(Box::new(NativeEglMixedSmokeComplete {
                status,
                detail,
                cpu_layers,
                dmabuf_layers,
                live_sources: self.presentation_resources.source_count(),
                live_fences: self.presentation_resources.fence_count(),
                live_transactions: self.presentation_resources.presentation_count(),
            }));
        }
        native_scanout.queue_mixed_frame(0, mixed);

        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let first_output = self
            .outputs
            .values_mut()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        let report = native_scanout.run_tick(
            0,
            &mut first_output.runtime,
            compositor_tick_input(&transactions, 0, None),
        )?;
        use sophia_backend_live::LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus as Status;
        match report
            .rendered_primary_plane_scanout_submit
            .map(|submit| submit.status)
        {
            Some(Status::SubmittedWaitingForPageFlip) => {
                // run_tick polls callbacks before it submits this frame. Any
                // feedback already queued here therefore belongs to an older
                // CPU frame and must not retire this Present transaction.
                native_scanout.discard_presentation_feedback(self.outputs.keys().next().copied());
                self.presentation_resources.mark_submitted(transaction)?;
                self.queued_gpu_presentations.pop_front();
                self.submitted_gpu_presentation = Some(SubmittedGpuPresentation {
                    transaction,
                    prepared,
                });
            }
            Some(Status::AlreadyInFlight | Status::CleanupPending) | None => {}
            Some(_) => {
                self.queued_gpu_presentations.pop_front();
                self.reject_gpu_presentation(transaction, 0, 0);
            }
        }
        Ok(report)
    }

    fn run_observation_tick(
        &mut self,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let transactions = self.layers.values().cloned().collect::<Vec<_>>();
        let output = self
            .outputs
            .values_mut()
            .next()
            .ok_or("persistent backend runtime has no outputs")?;
        Ok(output
            .runtime
            .run_tick(compositor_tick_input(&transactions, 0, None))?)
    }

    fn reject_gpu_presentation(&mut self, transaction: TransactionId, ust: u64, msc: u64) {
        if let Some(router) = self.protocol_router.as_ref() {
            let _ =
                router.route_present_complete(transaction, ust, msc, XPresentCompletionMode::Skip);
        }
        self.present_complete_skip = self.present_complete_skip.saturating_add(1);
        if let Some(retirement) = self.presentation_resources.reject(transaction)
            && retirement.idle_fence == sophia_backend_live::LiveIdleFenceStatus::Triggered
        {
            self.idle_fence_triggers = self.idle_fence_triggers.saturating_add(1);
        }
        if let Some(router) = self.protocol_router.as_ref() {
            let _ = router.route_present_idle(transaction);
        }
        self.present_idle = self.present_idle.saturating_add(1);
    }

    fn shutdown_presentations(&mut self) {
        let queued = self
            .queued_gpu_presentations
            .drain(..)
            .map(|queued| queued.submission.transaction)
            .collect::<Vec<_>>();
        for transaction in queued {
            self.reject_gpu_presentation(transaction, 0, 0);
        }
        if let Some(submitted) = self.submitted_gpu_presentation.take() {
            self.reject_gpu_presentation(submitted.transaction, 0, 0);
        }

        let report = self.presentation_resources.disconnect();
        self.idle_fence_triggers = self
            .idle_fence_triggers
            .saturating_add(report.triggered_idle_fences);
        self.presentation_disconnect_sources = self
            .presentation_disconnect_sources
            .saturating_add(report.released_sources.len());
        self.presentation_disconnect_fences = self
            .presentation_disconnect_fences
            .saturating_add(report.released_fences.len());
        self.presentation_disconnect_failures = self
            .presentation_disconnect_failures
            .saturating_add(report.failed_idle_fences);
    }

    fn run_authority_transactions(
        &mut self,
        transaction_id: TransactionId,
        transactions: &[SurfaceTransaction],
        removed_surfaces: &[SurfaceId],
        event_count: usize,
        mut native_scanout: Option<&mut PersistentNativeScanout>,
        native_frames: Option<Vec<ObservedComposedFrame>>,
        wm_update: Option<WmTransactionUpdate>,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        self.layers
            .retain(|surface, _| !removed_surfaces.contains(surface));
        for transaction in transactions {
            self.layers.insert(transaction.surface, transaction.clone());
        }
        let intake = AuthorityTransactionIntake::new(transaction_id, transactions.to_vec())
            .with_surface_removals(removed_surfaces.to_vec());
        let active_transactions = self.layers.values().cloned().collect::<Vec<_>>();

        // The runtime is initialized from the first authority batch.  A later
        // X client therefore needs a committed-state handoff before its first
        // incremental transaction can be replayed.  Without this barrier the
        // engine sees the new surface at generation zero and rejects every
        // update that raced ahead of the next compositor tick as stale.
        if transactions.iter().any(|transaction| {
            !self
                .committed_surfaces()
                .iter()
                .any(|committed| committed.surface == transaction.surface)
        }) {
            let committed_surfaces =
                seed_missing_committed_surfaces(self.committed_surfaces(), &active_transactions);
            self.prime_committed_surfaces(&committed_surfaces);
        }

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
            let input = compositor_tick_input(&active_transactions, event_count, wm_update.clone());
            let report = match native_scanout.as_deref_mut() {
                Some(native_scanout) => {
                    if let Some(frame) = native_frames.next() {
                        native_scanout.queue_frame(index, frame);
                    }
                    if output.runtime.rendered_primary_plane_scanout_in_flight() {
                        output.runtime.run_tick(input)?
                    } else {
                        native_scanout.run_tick(index, &mut output.runtime, input)?
                    }
                }
                None => output.runtime.run_tick(input)?,
            };
            if first_report.is_none() {
                first_report = Some(report);
            }
        }
        first_report.ok_or_else(|| "persistent backend runtime has no outputs".into())
    }

    /// Seeds a newly discovered authority surface at the generation immediately
    /// before its first observed update. The caller must then replay that update
    /// through the normal authority inbox so the authority generation chain
    /// remains contiguous.
    fn prime_committed_surfaces(&mut self, committed_surfaces: &[CommittedSurfaceState]) {
        self.committed_snapshot_layers = None;
        for output in self.outputs.values_mut() {
            output
                .runtime
                .assembly_mut()
                .replace_committed_surfaces(committed_surfaces.to_vec());
        }
    }

    fn run_committed_snapshot(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        mut native_scanout: Option<&mut PersistentNativeScanout>,
        native_frames: Option<Vec<ObservedComposedFrame>>,
    ) -> Result<sophia_backend_live::LiveBackendRuntimeTickReport, Box<dyn std::error::Error>> {
        let layers = layer_snapshots_from_committed(committed_surfaces);
        self.committed_snapshot_layers = Some(layers.clone());
        let mut native_frames = native_frames.unwrap_or_default().into_iter();
        let mut first_report = None;
        for (index, output) in self.outputs.values_mut().enumerate() {
            output
                .runtime
                .assembly_mut()
                .replace_committed_surfaces(committed_surfaces.to_vec());
            let input = compositor_tick_input_from_layers(layers.clone());
            let report = match native_scanout.as_deref_mut() {
                Some(native_scanout) => {
                    if let Some(frame) = native_frames.next() {
                        native_scanout.queue_frame(index, frame);
                    }
                    if output.runtime.rendered_primary_plane_scanout_in_flight() {
                        output.runtime.run_tick(input)?
                    } else {
                        native_scanout.run_tick(index, &mut output.runtime, input)?
                    }
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
                authority_local_id: None,
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
        if self.native_scanout_in_flight() {
            return Err("persistent native scanout remained in flight during teardown".into());
        }
        for (index, output) in self.outputs.values_mut().enumerate() {
            trace_live_native_lifecycle("displayed_scanout_retire_started");
            let retired = output
                .runtime
                .retire_displayed_rendered_primary_plane_scanout(native_scanout.card(index));
            if retired.cleanup_pending {
                trace_live_native_lifecycle("displayed_scanout_cleanup_retry_started");
                let cleanup = output
                    .runtime
                    .retry_tracked_rendered_primary_plane_scanout_cleanup(
                        native_scanout.card(index),
                    );
                if cleanup.cleanup_pending {
                    return Err("persistent displayed scanout cleanup remained pending".into());
                }
            }
            trace_live_native_lifecycle("displayed_scanout_owner_released");
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
                self.committed_snapshot_layers.as_ref().map_or_else(
                    || compositor_tick_input(&transactions, 0, None),
                    |layers| compositor_tick_input_from_layers(layers.clone()),
                ),
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
            if output
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
            {
                let cleanup = output
                    .runtime
                    .retry_tracked_rendered_primary_plane_scanout_cleanup(
                        native_scanout.card(index),
                    );
                if !cleanup.cleanup_pending {
                    native_scanout.retire_failures =
                        native_scanout.retire_failures.saturating_sub(1);
                }
            }
        }
        if let Some(primary) = self.outputs.keys().next().copied()
            && let Some((ust, msc)) = native_scanout.take_presentation_feedback(primary)
        {
            self.finalize_gpu_page_flip(ust, msc)?;
        }
        Ok(())
    }

    fn finalize_gpu_page_flip(
        &mut self,
        ust: u64,
        msc: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(submitted) = self.submitted_gpu_presentation.take() else {
            return Ok(());
        };
        for (output_id, prepared) in submitted.prepared {
            let output = self
                .outputs
                .get_mut(&output_id)
                .ok_or("prepared presentation referenced an unknown output")?;
            let engine = output.runtime.assembly().engine().clone();
            let mut committed = output.runtime.assembly().committed_surfaces().to_vec();
            let commit = engine.apply_prepared_surface_commit(prepared, &mut committed);
            if commit.outcome != TransactionOutcome::Committed {
                return Err("page flip could not apply its prepared Engine commit".into());
            }
            output
                .runtime
                .assembly_mut()
                .replace_committed_surfaces(committed);
        }
        if let Some(router) = self.protocol_router.as_ref() {
            let _ = router.route_present_complete(
                submitted.transaction,
                ust,
                msc,
                XPresentCompletionMode::Flip,
            );
        }
        self.present_complete_flip = self.present_complete_flip.saturating_add(1);
        if let Some(retirement) = self
            .presentation_resources
            .retire_page_flip(submitted.transaction)
            && retirement.idle_fence == sophia_backend_live::LiveIdleFenceStatus::Triggered
        {
            self.idle_fence_triggers = self.idle_fence_triggers.saturating_add(1);
        }
        if let Some(router) = self.protocol_router.as_ref() {
            let _ = router.route_present_idle(submitted.transaction);
        }
        self.present_idle = self.present_idle.saturating_add(1);
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

    fn native_diagnostic(&self) -> String {
        self.outputs
            .iter()
            .map(|(output, state)| {
                format!(
                    "{}:in_flight={}:ticks={}:cleanup={}",
                    output.raw(),
                    state.runtime.rendered_primary_plane_scanout_in_flight(),
                    state
                        .runtime
                        .rendered_primary_plane_scanout_in_flight_ticks(),
                    state
                        .runtime
                        .rendered_primary_plane_scanout_cleanup_pending(),
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub(super) struct WaylandNativeSession {
    scanout: PersistentNativeScanout,
    runtime: Option<PersistentBackendRuntime>,
    outputs: Vec<sophia_engine::HeadlessOutput>,
    pending_cpu_presentations: BTreeMap<SurfaceId, u64>,
    cursor_repaint_pending: bool,
    awaiting_presentations: BTreeMap<SurfaceId, (u64, usize)>,
    last_cpu_checksum: Option<u64>,
}

pub(super) struct WaylandCpuFrameSubmission {
    pub(super) presentations: Vec<(SurfaceId, u64)>,
    pub(super) immediate: bool,
    /// `true` when this composition pass reached KMS submission.  The
    /// corresponding client buffer remains held until the later page flip.
    pub(super) frame_scheduled: bool,
}

impl WaylandNativeSession {
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scanout = PersistentNativeScanout::new()?;
        let outputs = scanout.outputs();
        Ok(Self {
            scanout,
            runtime: None,
            outputs,
            pending_cpu_presentations: BTreeMap::new(),
            cursor_repaint_pending: false,
            awaiting_presentations: BTreeMap::new(),
            last_cpu_checksum: None,
        })
    }

    pub(super) fn primary_size(&self) -> Size {
        self.outputs[0].size
    }

    pub(super) fn dmabuf_main_device(&self) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(self.scanout.card(0).try_clone_file()?.metadata()?.rdev())
    }

    pub(super) fn enqueue_cpu_presentation(&mut self, surface: SurfaceId, generation: u64) {
        self.pending_cpu_presentations.insert(surface, generation);
    }

    /// A compositor-owned cursor has changed. Unlike a client commit, this
    /// repaint must not create presentation feedback for any client surface.
    pub(super) fn request_cpu_cursor_repaint(&mut self) {
        self.cursor_repaint_pending = true;
    }

    pub(super) fn should_compose_cpu_frame(&self) -> bool {
        let (in_flight, cleanup_pending) =
            self.runtime.as_ref().map_or((false, false), |runtime| {
                (
                    runtime.native_scanout_in_flight(),
                    runtime.native_cleanup_pending(),
                )
            });
        cpu_frame_submission_ready(
            !self.pending_cpu_presentations.is_empty() || self.cursor_repaint_pending,
            in_flight,
            cleanup_pending,
            self.scanout
                .heads
                .iter()
                .enumerate()
                .any(|(index, _)| self.scanout.pending_frame(index)),
        )
    }

    pub(super) fn submit_cpu_frame(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        report: &sophia_backend_live::LiveCpuCompositionReport,
    ) -> Result<WaylandCpuFrameSubmission, Box<dyn std::error::Error>> {
        let presentations = self
            .pending_cpu_presentations
            .iter()
            .map(|(surface, generation)| (*surface, *generation))
            .collect::<Vec<_>>();
        if presentations.is_empty() && !self.cursor_repaint_pending {
            return Err(
                "native CPU composition had no queued presentation or cursor repaint".into(),
            );
        }
        if cpu_frame_matches_visible_output(
            self.outputs.len(),
            self.runtime.is_some(),
            self.last_cpu_checksum,
            report.checksum,
        ) {
            self.pending_cpu_presentations.clear();
            self.cursor_repaint_pending = false;
            return Ok(WaylandCpuFrameSubmission {
                presentations,
                immediate: true,
                frame_scheduled: false,
            });
        }
        let frames = self
            .outputs
            .iter()
            .map(|output| native_frame_for_output(report, output.size))
            .collect::<Vec<_>>();
        if self.runtime.is_none() {
            self.runtime = Some(PersistentBackendRuntime::new_from_committed_surfaces(
                &self.outputs,
                committed_surfaces,
                Some(&mut self.scanout),
                Some(frames),
            )?);
            self.pending_cpu_presentations.clear();
            self.cursor_repaint_pending = false;
            self.last_cpu_checksum = Some(report.checksum);
            return Ok(WaylandCpuFrameSubmission {
                presentations,
                immediate: true,
                frame_scheduled: false,
            });
        }
        let runtime = self.runtime.as_mut().expect("checked above");
        let submissions_before = self.scanout.heads[0].submissions;
        let _ = runtime.run_committed_snapshot(
            committed_surfaces,
            Some(&mut self.scanout),
            Some(frames),
        )?;
        let frame_scheduled = self.scanout.heads[0].submissions > submissions_before;
        let required_submission = self.required_presentation_submission(submissions_before)?;
        self.pending_cpu_presentations.clear();
        self.cursor_repaint_pending = false;
        for (surface, generation) in &presentations {
            retain_latest_wayland_presentation(
                &mut self.awaiting_presentations,
                *surface,
                *generation,
                required_submission,
            );
        }
        self.last_cpu_checksum = Some(report.checksum);
        Ok(WaylandCpuFrameSubmission {
            presentations,
            immediate: false,
            frame_scheduled,
        })
    }

    pub(super) fn present_dmabuf(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        transaction: &SurfaceTransaction,
        generation: u64,
        frame: &sophia_backend_live::LiveOwnedDmaBufFrame,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Direct client scanout replaces the visible CPU-composed output, so a
        // later SHM frame cannot be elided against an older CPU checksum.
        self.last_cpu_checksum = None;
        // CPU cursor composition cannot safely overlay a directly scanned-out
        // DMA-BUF. The next CPU-backed commit will re-enable cursor repaints.
        self.cursor_repaint_pending = false;
        if self.runtime.is_none() {
            let blank = sophia_backend_live::compose_live_cpu_frame(self.primary_size(), &[])
                .map_err(|error| format!("native DMA-BUF bootstrap failed: {error:?}"))?;
            let frames = self
                .outputs
                .iter()
                .map(|output| native_frame_for_output(&blank, output.size))
                .collect::<Vec<_>>();
            self.runtime = Some(PersistentBackendRuntime::new_from_committed_surfaces(
                &self.outputs,
                committed_surfaces,
                Some(&mut self.scanout),
                Some(frames),
            )?);
        }
        for head in &mut self.scanout.heads {
            head.exporter.set_pending_dmabuf_frame(frame.try_clone()?);
        }
        trace_native_dmabuf_lifecycle("client_frame_retained");
        let runtime = self.runtime.as_mut().expect("initialized above");
        let submissions_before = self.scanout.heads[0].submissions;
        let _ =
            runtime.run_committed_snapshot(committed_surfaces, Some(&mut self.scanout), None)?;
        let head = &self.scanout.heads[0];
        if head.submissions > submissions_before {
            trace_native_dmabuf_lifecycle("kms_submitted");
        } else if head.exporter.pending_dmabuf_frame() {
            trace_native_dmabuf_lifecycle("kms_submission_deferred");
        } else {
            trace_native_dmabuf_lifecycle("kms_submission_failed");
        }
        self.queue_presentation(transaction.surface, generation, submissions_before)?;
        Ok(false)
    }

    pub(super) fn service(&mut self) -> Result<Vec<(SurfaceId, u64)>, Box<dyn std::error::Error>> {
        let Some(runtime) = self.runtime.as_mut() else {
            return Ok(Vec::new());
        };
        if runtime.native_scanout_in_flight() || runtime.native_cleanup_pending() {
            let callbacks_before = self.scanout.callback_accepted;
            let retirements_before = self.scanout.retirements;
            runtime.retire_native_scanout(&mut self.scanout)?;
            if self.scanout.callback_accepted > callbacks_before {
                trace_native_dmabuf_lifecycle("page_flip_observed");
            }
            if self.scanout.retirements > retirements_before {
                trace_native_dmabuf_lifecycle("scanout_retired");
            }
        }
        if !runtime.native_scanout_in_flight()
            && self
                .scanout
                .heads
                .iter()
                .enumerate()
                .any(|(index, _)| self.scanout.pending_frame(index))
        {
            let _ = runtime.run_native_idle(&mut self.scanout)?;
        }
        let presented_submissions = self
            .scanout
            .heads
            .first()
            .map_or(0, |head| head.presented_submissions);
        let ready = self
            .awaiting_presentations
            .iter()
            .filter(|(_, (_, required_submission))| *required_submission <= presented_submissions)
            .map(|(surface, (generation, _))| (*surface, *generation))
            .collect::<Vec<_>>();
        self.awaiting_presentations
            .retain(|_, (_, required_submission)| *required_submission > presented_submissions);
        Ok(ready)
    }

    fn queue_presentation(
        &mut self,
        surface: SurfaceId,
        generation: u64,
        submissions_before: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let required_submission = self.required_presentation_submission(submissions_before)?;
        retain_latest_wayland_presentation(
            &mut self.awaiting_presentations,
            surface,
            generation,
            required_submission,
        );
        Ok(())
    }

    fn required_presentation_submission(
        &self,
        submissions_before: usize,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let head = &self.scanout.heads[0];
        let required_submission = required_wayland_presentation_submission(
            submissions_before,
            head.submissions,
            head.exporter.pending_cpu_frame() || head.exporter.pending_dmabuf_frame(),
        )?;
        Ok(required_submission)
    }

    pub(super) fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.drain_native_scanout(&mut self.scanout, Duration::from_secs(2))?;
        }
        Ok(())
    }

    pub(super) fn completion_evidence(&self) -> String {
        let dmabuf_import_attempts = self
            .scanout
            .heads
            .iter()
            .map(|head| head.exporter.dmabuf_frame_export_attempts())
            .sum::<usize>();
        let dmabuf_imports = self
            .scanout
            .heads
            .iter()
            .map(|head| head.exporter.dmabuf_frame_exports())
            .sum::<usize>();
        let (
            native_target_creations,
            native_target_recreations,
            native_pipeline_creations,
            native_uploads,
            native_max_upload,
        ) = self.scanout.persistent_render_metrics();
        let in_flight = self
            .runtime
            .as_ref()
            .is_some_and(PersistentBackendRuntime::native_scanout_in_flight);
        let cleanup_pending = self
            .runtime
            .as_ref()
            .is_some_and(PersistentBackendRuntime::native_cleanup_pending);
        format!(
            "sophia_wayland_native schema=1 status=complete outputs={} submissions={} retirements={} callbacks={} submit_failures={} retire_failures={} callback_rejected={} dmabuf_import_attempts={} dmabuf_imports={} max_submit_to_page_flip_msec={} native_max_upload_msec={} native_target_creations={} native_target_recreations={} native_pipeline_creations={} native_frame_uploads={} in_flight={} cleanup_pending={}",
            self.scanout.heads.len(),
            self.scanout.submissions,
            self.scanout.retirements,
            self.scanout.callback_accepted,
            self.scanout.submit_failures,
            self.scanout.retire_failures,
            self.scanout.callback_rejected,
            dmabuf_import_attempts,
            dmabuf_imports,
            self.scanout.max_submit_to_page_flip.as_millis(),
            native_max_upload.as_millis(),
            native_target_creations,
            native_target_recreations,
            native_pipeline_creations,
            native_uploads,
            in_flight,
            cleanup_pending,
        )
    }

    pub(super) fn cancel_surface(&mut self, surface: SurfaceId) {
        self.pending_cpu_presentations.remove(&surface);
        self.awaiting_presentations.remove(&surface);
    }
}

fn retain_latest_wayland_presentation(
    pending: &mut BTreeMap<SurfaceId, (u64, usize)>,
    surface: SurfaceId,
    generation: u64,
    required_submission: usize,
) {
    pending.insert(surface, (generation, required_submission));
}

fn cpu_frame_submission_ready(
    has_pending_cpu_presentation: bool,
    native_scanout_in_flight: bool,
    native_cleanup_pending: bool,
    native_frame_pending: bool,
) -> bool {
    has_pending_cpu_presentation
        && !native_scanout_in_flight
        && !native_cleanup_pending
        && !native_frame_pending
}

fn cpu_frame_matches_visible_output(
    output_count: usize,
    runtime_exists: bool,
    last_cpu_checksum: Option<u64>,
    candidate_checksum: u64,
) -> bool {
    output_count == 1 && runtime_exists && last_cpu_checksum == Some(candidate_checksum)
}

fn required_wayland_presentation_submission(
    submissions_before: usize,
    submissions_after: usize,
    frame_pending: bool,
) -> Result<usize, &'static str> {
    if submissions_after > submissions_before {
        Ok(submissions_after)
    } else if frame_pending {
        Ok(submissions_after.saturating_add(1))
    } else {
        Err("native frame was neither submitted nor retained for a later submit")
    }
}

fn trace_native_dmabuf_lifecycle(stage: &str) {
    if std::env::var_os("SOPHIA_WAYLAND_DMABUF_DIAGNOSTIC").is_some() {
        eprintln!(
            "sophia_dmabuf_lifecycle schema=1 pid={} stage={stage}",
            std::process::id()
        );
    }
}

fn native_frame_for_output(
    report: &sophia_backend_live::LiveCpuCompositionReport,
    output_size: Size,
) -> ObservedComposedFrame {
    if report.frame.size == output_size {
        return ObservedComposedFrame {
            frame: report.frame.clone(),
            checksum: report.checksum,
            nonzero_pixel_bytes: report.nonzero_pixel_bytes,
        };
    }
    let width = usize::try_from(output_size.width).unwrap_or(0);
    let height = usize::try_from(output_size.height).unwrap_or(0);
    let stride = width.saturating_mul(4);
    let mut bytes = vec![0; stride.saturating_mul(height)];
    let source_width = usize::try_from(report.frame.size.width).unwrap_or(0);
    let source_height = usize::try_from(report.frame.size.height).unwrap_or(0);
    let source_stride = usize::try_from(report.frame.stride).unwrap_or(0);
    let copy_width = width.min(source_width);
    let copy_height = height.min(source_height);
    for row in 0..copy_height {
        let source = row.saturating_mul(source_stride);
        let target = row.saturating_mul(stride);
        let count = copy_width.saturating_mul(4);
        if let (Some(source), Some(target)) = (
            report.frame.bytes.get(source..source.saturating_add(count)),
            bytes.get_mut(target..target.saturating_add(count)),
        ) {
            target.copy_from_slice(source);
        }
    }
    let (nonzero_pixel_bytes, checksum) = bytes.iter().fold(
        (0usize, 0xcbf2_9ce4_8422_2325u64),
        |(nonzero, hash), byte| {
            (
                nonzero.saturating_add(usize::from(*byte != 0)),
                (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3),
            )
        },
    );
    ObservedComposedFrame {
        frame: sophia_backend_live::LiveCpuComposedFrame {
            size: output_size,
            stride: u32::try_from(stride).unwrap_or(u32::MAX),
            format: sophia_backend_live::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
            bytes,
        },
        checksum,
        nonzero_pixel_bytes,
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

fn compositor_tick_input_from_layers(
    layer_templates: Vec<LayerSnapshot>,
) -> CompositorBackendTickInput {
    CompositorBackendTickInput {
        x_event_count: 0,
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layer_templates,
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    }
}

fn layer_snapshots_from_committed(
    committed_surfaces: &[CommittedSurfaceState],
) -> Vec<LayerSnapshot> {
    committed_surfaces
        .iter()
        .enumerate()
        .map(|(stack_rank, surface)| LayerSnapshot {
            surface: surface.surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: u32::try_from(stack_rank).unwrap_or(u32::MAX),
            geometry: surface.geometry,
            source: surface.buffer,
            damage: surface.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: surface.committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        })
        .collect()
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
    presentation_feedback: VecDeque<(OutputId, u64, u64)>,
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
    submitted_sequence: Option<usize>,
    presented_checksum: u64,
    presented_submissions: usize,
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
                    submitted_sequence: None,
                    presented_checksum: 0,
                    presented_submissions: 0,
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
            presentation_feedback: VecDeque::new(),
        })
    }

    fn clone_render_device_file(&self) -> std::io::Result<std::fs::File> {
        self.groups
            .first()
            .ok_or_else(|| std::io::Error::other("native scanout has no DRM device group"))?
            .session
            .card()
            .try_clone_file()
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
                    trace_live_native_lifecycle("kms_submit_accepted");
                    self.submissions = self.submissions.saturating_add(1);
                    self.heads[index].submissions = self.heads[index].submissions.saturating_add(1);
                    self.heads[index].submitted_at = Some(Instant::now());
                    self.heads[index].submitted_checksum = Some(self.heads[index].last_checksum);
                    self.heads[index].submitted_sequence = Some(self.heads[index].submissions);
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
                trace_live_native_lifecycle("kms_buffer_retired");
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
            trace_live_native_lifecycle("page_flip_callback_accepted");
            if let Some(checksum) = self.heads[index].submitted_checksum.take() {
                self.heads[index].presented_checksum = checksum;
            }
            if let Some(submission) = self.heads[index].submitted_sequence.take() {
                self.heads[index].presented_submissions = submission;
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
                let ust = u64::try_from(self.presentation_started.elapsed().as_micros())
                    .unwrap_or(u64::MAX);
                self.presentation_feedback
                    .push_back((output, ust, kernel_sequence));
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
        frame: ObservedComposedFrame,
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
        trace_live_native_lifecycle("initial_modeset_complete");
        head.submissions = head.submissions.saturating_add(1);
        head.presented_checksum = head.last_checksum;
        head.presented_submissions = head.submissions;
        Ok(())
    }

    fn queue_frame(&mut self, index: usize, frame: ObservedComposedFrame) {
        let head = &mut self.heads[index];
        head.pending_nonzero_pixel_bytes = frame.nonzero_pixel_bytes;
        head.last_checksum = frame.checksum;
        head.exporter
            .set_pending_cpu_frame_with_checksum(frame.frame, frame.checksum);
    }

    fn queue_mixed_frame(
        &mut self,
        index: usize,
        frame: sophia_backend_live::LiveOwnedMixedCompositionFrame,
    ) {
        self.heads[index].exporter.set_pending_mixed_frame(frame);
    }

    fn diagnose_mixed_frame(
        &mut self,
        index: usize,
        frame: sophia_backend_live::LiveOwnedMixedCompositionFrame,
    ) -> (
        sophia_backend_live::LiveRendererScanoutBufferExportStatus,
        sophia_backend_live::LiveRendererScanoutBufferExportDetail,
    ) {
        use sophia_backend_live::LiveRenderedScanoutBufferExporter as _;

        let head = &mut self.heads[index];
        head.exporter.set_pending_mixed_frame(frame);
        let export = head.exporter.export_rendered_scanout_buffer(
            sophia_backend_live::LiveGbmEglFrameTargetRecord::new(head.output.size),
        );
        let status = export.status;
        let detail = export.detail;
        drop(export);
        (status, detail)
    }

    fn take_presentation_feedback(&mut self, output: OutputId) -> Option<(u64, u64)> {
        let index = self
            .presentation_feedback
            .iter()
            .position(|(candidate, _, _)| *candidate == output)?;
        let (_, ust, msc) = self.presentation_feedback.remove(index)?;
        Some((ust, msc))
    }

    fn discard_presentation_feedback(&mut self, output: Option<OutputId>) {
        match output {
            Some(output) => self
                .presentation_feedback
                .retain(|(candidate, _, _)| *candidate != output),
            None => self.presentation_feedback.clear(),
        }
    }

    fn pending_frame(&self, index: usize) -> bool {
        self.heads[index].exporter.pending_cpu_frame()
            || self.heads[index].exporter.pending_dmabuf_frame()
            || self.heads[index].exporter.pending_mixed_frame()
    }

    fn export_attempts(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.exporter.cpu_frame_export_attempts())
            .chain(
                self.heads
                    .iter()
                    .map(|head| head.exporter.mixed_frame_export_attempts()),
            )
            .sum()
    }

    fn mixed_exports(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.exporter.mixed_frame_exports())
            .sum()
    }

    fn persistent_render_metrics(&self) -> (usize, usize, usize, usize, Duration) {
        self.heads.iter().fold(
            (0, 0, 0, 0, Duration::ZERO),
            |(targets, recreations, pipelines, uploads, max_upload), head| {
                let stats = head.exporter.persistent_render_stats();
                (
                    targets.saturating_add(stats.target_creations),
                    recreations.saturating_add(stats.target_recreations),
                    pipelines.saturating_add(stats.gl_pipeline_creations),
                    uploads.saturating_add(stats.frame_uploads),
                    max_upload.max(stats.max_upload),
                )
            },
        )
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
    seed_missing_committed_surfaces(&[], transactions)
}

fn seed_missing_committed_surfaces(
    existing: &[CommittedSurfaceState],
    transactions: &[SurfaceTransaction],
) -> Vec<CommittedSurfaceState> {
    let mut surfaces = existing
        .iter()
        .cloned()
        .map(|surface| (surface.surface, surface))
        .collect::<BTreeMap<_, _>>();
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
    raised_surface: Option<SurfaceId>,
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
            raised_surface: None,
            last_report: None,
            max_nonzero_pixel_bytes: 0,
            nonzero_frames: 0,
        }
    }

    fn observe(
        &mut self,
        batch: &XAuthorityObservedTransactionBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for surface in &batch.removed_surfaces {
            self.surfaces.remove(surface);
            if self.raised_surface == Some(*surface) {
                self.raised_surface = None;
            }
        }
        self.buffers.retain(|handle, _| {
            self.surfaces
                .values()
                .any(|(_, surface_handle)| surface_handle == handle)
        });
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

    fn raise_surface(&mut self, surface: SurfaceId) {
        if self.surfaces.contains_key(&surface) {
            self.raised_surface = Some(surface);
        }
    }

    fn compose(
        &mut self,
    ) -> Result<&sophia_backend_live::LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        let mut surface_order = self
            .surfaces
            .keys()
            .copied()
            .filter(|surface| Some(*surface) != self.raised_surface)
            .collect::<Vec<_>>();
        if let Some(surface) = self.raised_surface
            && self.surfaces.contains_key(&surface)
        {
            surface_order.push(surface);
        }
        let layers = surface_order
            .iter()
            .filter_map(|surface| {
                let (geometry, handle) = self.surfaces.get(surface)?;
                let buffer = self.buffers.get(handle)?;
                Some(sophia_backend_live::LiveCpuCompositionLayerRef {
                    geometry: *geometry,
                    buffer: sophia_backend_live::LiveCpuBufferSourceRef {
                        handle: buffer.handle,
                        size: buffer.size,
                        stride: buffer.stride,
                        format: buffer.format,
                        generation: buffer.generation,
                        bytes: &buffer.bytes,
                    },
                })
            })
            .collect::<Vec<_>>();
        self.last_report = Some(
            sophia_backend_live::compose_live_cpu_frame_ref(self.output_size, &layers)
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

    fn surface_buffer_generation(&self, surface: SurfaceId) -> Option<u64> {
        let (_, handle) = self.surfaces.get(&surface)?;
        let buffer = self.buffers.get(handle)?;
        Some(buffer.generation)
    }

    /// Returns true only when the focused surface contains at least two
    /// visible XRGB pixel values. A newly mapped xterm initially publishes a
    /// uniform background buffer; its prompt or cursor introduces visual
    /// detail once the terminal side is ready for input. Inspecting the
    /// focused surface avoids treating another client's draw as readiness.
    fn surface_has_visual_detail(&self, surface: SurfaceId) -> bool {
        let Some((_, handle)) = self.surfaces.get(&surface) else {
            return false;
        };
        let Some(buffer) = self.buffers.get(handle) else {
            return false;
        };
        let Ok(width) = usize::try_from(buffer.size.width) else {
            return false;
        };
        let Ok(height) = usize::try_from(buffer.size.height) else {
            return false;
        };
        let Ok(stride) = usize::try_from(buffer.stride) else {
            return false;
        };
        let Some(row_bytes) = width.checked_mul(4) else {
            return false;
        };
        if width == 0 || height == 0 || stride < row_bytes || buffer.bytes.len() < 4 {
            return false;
        }
        let first = &buffer.bytes[..4];
        (0..height).any(|row| {
            let Some(start) = row.checked_mul(stride) else {
                return false;
            };
            let Some(end) = start.checked_add(row_bytes) else {
                return false;
            };
            buffer
                .bytes
                .get(start..end)
                .is_some_and(|bytes| bytes.chunks_exact(4).any(|pixel| pixel != first))
        })
    }

    fn frames_for_outputs(
        &self,
        outputs: &[sophia_engine::HeadlessOutput],
    ) -> Result<Vec<ObservedComposedFrame>, Box<dyn std::error::Error>> {
        let primary = self
            .last_report
            .as_ref()
            .ok_or("persistent CPU scene has no composed primary frame")?;
        let mut frames = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            if index == 0 && output.size == primary.frame.size {
                frames.push(ObservedComposedFrame {
                    frame: primary.frame.clone(),
                    checksum: primary.checksum,
                    nonzero_pixel_bytes: primary.nonzero_pixel_bytes,
                });
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
            let report = sophia_backend_live::compose_live_cpu_frame(output.size, &[marker])
                .map_err(|error| format!("secondary output composition failed: {error:?}"))?;
            frames.push(ObservedComposedFrame {
                frame: report.frame,
                checksum: report.checksum,
                nonzero_pixel_bytes: report.nonzero_pixel_bytes,
            });
        }
        Ok(frames)
    }
}

fn trace_live_native_lifecycle(stage: &str) {
    if std::env::var_os("SOPHIA_LIVE_SESSION_DIAGNOSTIC").is_some() {
        eprintln!("sophia_live_native_lifecycle schema=1 stage={stage}");
    }
}

fn synthetic_text_input_events(
    text: &str,
) -> Result<Vec<sophia_protocol::InputEventPacket>, Box<dyn std::error::Error>> {
    let mut serial = 1u64;
    let mut events = Vec::with_capacity((text.len() + 1).saturating_mul(2));
    for x_keycode in text
        .bytes()
        .map(super::x_authority::x11_keycode_for_ascii)
        .chain(std::iter::once(Some(36)))
    {
        let x_keycode = x_keycode.ok_or("test input has no core X keycode")?;
        let keycode = u32::from(
            x_keycode
                .checked_sub(8)
                .ok_or("test input has no evdev keycode")?,
        );
        for pressed in [true, false] {
            events.push(sophia_protocol::InputEventPacket {
                serial,
                seat: SeatId::from_raw(SESSION_SEAT_RAW),
                device: DeviceId::from_raw(SESSION_KEYBOARD_DEVICE_RAW),
                time_msec: serial,
                kind: sophia_protocol::InputEventKind::Key { keycode, pressed },
                global_position: None,
                target_surface: None,
                local_position: None,
            });
            serial = serial.saturating_add(1);
        }
    }
    Ok(events)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PhysicalInputRouteReport {
    events: usize,
    keys_observed: usize,
    keys_routed: usize,
    pointer_events: usize,
    pointer_routed: usize,
    deliveries: Vec<XAuthorityInputDeliveryId>,
    emergency_exit: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SessionPointerPlacement {
    offset: Option<Point>,
}

fn pointer_offset_for_geometry(raw: Point, geometry: Rect) -> Point {
    Point {
        x: f64::from(geometry.x) + f64::from(geometry.width) / 2.0 - raw.x,
        y: f64::from(geometry.y) + f64::from(geometry.height) / 2.0 - raw.y,
    }
}

impl SessionPointerPlacement {
    fn place(
        &mut self,
        raw: Point,
        focused_surface: Option<SurfaceId>,
        input_layers: &[LayerSnapshot],
    ) -> Point {
        let offset = *self.offset.get_or_insert_with(|| {
            let Some(geometry) = focused_surface.and_then(|surface| {
                input_layers
                    .iter()
                    .find(|layer| layer.surface == surface)
                    .map(|layer| layer.geometry)
            }) else {
                return Point::default();
            };
            pointer_offset_for_geometry(raw, geometry)
        });
        Point {
            x: raw.x + offset.x,
            y: raw.y + offset.y,
        }
    }
}

fn place_pointer_event_for_routing(
    event: &mut sophia_protocol::InputEventPacket,
    focused_surface: Option<SurfaceId>,
    input_layers: &[LayerSnapshot],
    pointer: &mut SessionPointerPlacement,
    buttons_only: bool,
) -> bool {
    if let Some(raw) = event.global_position {
        event.global_position = Some(pointer.place(raw, focused_surface, input_layers));
    }
    !(buttons_only && matches!(event.kind, sophia_protocol::InputEventKind::PointerMotion))
}

fn route_physical_input<P: NonBlockingInputPoller>(
    poller: &mut P,
    focus: &InputFocusState,
    committed_surfaces: &[CommittedSurfaceState],
    input_layers: &[LayerSnapshot],
    client_routes: &XAuthorityClientSurfaceRoutes,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    modifiers: &mut XCoreKeyboardMapper,
    emergency_chord: &mut EmergencyChordState,
    pointer: &mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_buttons_only: bool,
    next_input_delivery: &mut u64,
    physical_text_proof: Option<&mut PhysicalTextProof>,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let events = poller.poll_ready()?;
    route_input_events(
        events,
        focus,
        committed_surfaces,
        input_layers,
        client_routes,
        input_sender,
        modifiers,
        emergency_chord,
        pointer,
        pointer_routing_enabled,
        pointer_buttons_only,
        next_input_delivery,
        physical_text_proof,
    )
}

#[allow(clippy::too_many_arguments)]
fn route_input_events(
    events: Vec<sophia_protocol::InputEventPacket>,
    focus: &InputFocusState,
    committed_surfaces: &[CommittedSurfaceState],
    input_layers: &[LayerSnapshot],
    _client_routes: &XAuthorityClientSurfaceRoutes,
    input_sender: &SyncSender<XAuthorityRoutedInput>,
    modifiers: &mut XCoreKeyboardMapper,
    emergency_chord: &mut EmergencyChordState,
    pointer: &mut SessionPointerPlacement,
    pointer_routing_enabled: bool,
    pointer_buttons_only: bool,
    next_input_delivery: &mut u64,
    mut physical_text_proof: Option<&mut PhysicalTextProof>,
) -> Result<PhysicalInputRouteReport, Box<dyn std::error::Error>> {
    let mut report = PhysicalInputRouteReport {
        events: events.len(),
        keys_observed: 0,
        keys_routed: 0,
        pointer_events: 0,
        pointer_routed: 0,
        deliveries: Vec::new(),
        emergency_exit: false,
    };
    for mut event in events {
        match event.kind {
            sophia_protocol::InputEventKind::Key { keycode, pressed } => {
                report.keys_observed = report.keys_observed.saturating_add(1);
                if emergency_chord.observe(keycode, pressed) == EmergencyChordAction::Triggered {
                    report.emergency_exit = true;
                    continue;
                }
                let FocusedInputRoute::Routed(event) =
                    focus.route_keyboard_event(event, committed_surfaces)
                else {
                    continue;
                };
                let Some(target_surface) = event.target_surface else {
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
                let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
                *next_input_delivery = next_input_delivery
                    .checked_add(1)
                    .ok_or("live-session input delivery ID exhausted")?;
                input_sender.try_send(XAuthorityRoutedInput {
                    request: sophia_protocol::RoutedInputRequest {
                        serial: event.serial,
                        seat: event.seat,
                        device: event.device,
                        time_msec: event.time_msec,
                        target_surface,
                        global_position: Point::default(),
                        local_position: Point::default(),
                        kind: event.kind,
                    },
                    delivery: Some(delivery),
                })?;
                report.keys_routed = report.keys_routed.saturating_add(1);
                report.deliveries.push(delivery);
            }
            kind @ (sophia_protocol::InputEventKind::PointerMotion
            | sophia_protocol::InputEventKind::PointerButton { .. }) => {
                report.pointer_events = report.pointer_events.saturating_add(1);
                if !pointer_routing_enabled {
                    continue;
                }
                let focused_surface = focus.focused_surface(event.seat);
                if !place_pointer_event_for_routing(
                    &mut event,
                    focused_surface,
                    input_layers,
                    pointer,
                    pointer_buttons_only,
                ) {
                    continue;
                }
                let route = sophia_engine::hit_test_scene_surface_for_input(&event, input_layers);
                let (Some(global), Some(local)) = (event.global_position, route.local_position)
                else {
                    continue;
                };
                let Some(surface) = route.target_surface else {
                    continue;
                };
                let delivery = XAuthorityInputDeliveryId::from_raw(*next_input_delivery);
                *next_input_delivery = next_input_delivery
                    .checked_add(1)
                    .ok_or("live-session input delivery ID exhausted")?;
                input_sender.try_send(XAuthorityRoutedInput {
                    request: sophia_protocol::RoutedInputRequest {
                        serial: event.serial,
                        seat: event.seat,
                        device: event.device,
                        time_msec: event.time_msec,
                        target_surface: surface,
                        global_position: global,
                        local_position: local,
                        kind,
                    },
                    delivery: Some(delivery),
                })?;
                report.pointer_routed = report.pointer_routed.saturating_add(1);
                report.deliveries.push(delivery);
            }
        }
    }
    Ok(report)
}

struct SessionProcessGuard {
    child: Option<Child>,
    secondary_children: Vec<Child>,
    socket_path: Option<std::path::PathBuf>,
}

impl SessionProcessGuard {
    fn new(child: Child, secondary_children: Vec<Child>, socket_path: std::path::PathBuf) -> Self {
        Self {
            child: Some(child),
            secondary_children,
            socket_path: Some(socket_path),
        }
    }

    fn children_mut(&mut self) -> Result<(&mut Child, &mut [Child]), Box<dyn std::error::Error>> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| -> Box<dyn std::error::Error> { "xterm child missing".into() })?;
        Ok((child, self.secondary_children.as_mut_slice()))
    }

    fn add_secondary_child(&mut self, child: Child) {
        self.secondary_children.push(child);
    }

    fn terminate(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        for mut child in self.secondary_children.drain(..) {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
        }
        if let Some(socket_path) = self.socket_path.as_ref() {
            match std::fs::remove_file(socket_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

impl Drop for SessionProcessGuard {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BufferSource, CommittedSurfaceState, LiveXAuthorityFile, PRIMARY_INPUT_PROOF_SCRIPT,
        PersistentBackendRuntime, PersistentCpuScene, PersistentXtermSessionConfig, Rect, Region,
        SECONDARY_POINTER_WITNESS_SCRIPT, SessionPointerPlacement, Size,
        authority_transaction_count, center_geometry_without_scaling,
        cpu_frame_matches_visible_output, cpu_frame_submission_ready,
        global_runtime_deadline_ends_session, layer_snapshots_from_committed,
        physical_input_may_route_after_primary_exit, physical_input_pixels_already_changed,
        place_pointer_event_for_routing, pointer_offset_for_geometry, record_runtime_commits,
        required_wayland_presentation_submission, retain_latest_wayland_presentation,
        seed_missing_committed_surfaces, successful_primary_exit_ends_session,
    };
    use sophia_protocol::{
        AuthorityKind, DeviceId, InputEventKind, InputEventPacket, NamespaceCapabilities,
        NamespaceProfile, Point, SeatId, SurfaceId, SurfaceTransaction,
        SurfaceTransactionReadiness,
    };
    use sophia_x_authority::{
        X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888, XAuthorityCpuBufferSnapshot, XResourceId,
    };
    use std::collections::BTreeMap;

    #[test]
    fn successful_primary_exit_keeps_requested_input_proof_alive() {
        assert!(successful_primary_exit_ends_session(false));
        assert!(!successful_primary_exit_ends_session(true));
    }

    #[test]
    fn global_runtime_deadline_does_not_strand_an_active_input_proof() {
        assert!(global_runtime_deadline_ends_session(false));
        assert!(!global_runtime_deadline_ends_session(true));
    }

    #[test]
    fn physical_input_waits_for_focus_to_leave_exited_proof_surface() {
        let proof = SurfaceId::new(1, 1);
        let survivor = SurfaceId::new(2, 1);
        assert!(physical_input_may_route_after_primary_exit(
            false,
            Some(proof),
            Some(proof)
        ));
        assert!(!physical_input_may_route_after_primary_exit(
            true,
            Some(proof),
            Some(proof)
        ));
        assert!(physical_input_may_route_after_primary_exit(
            true,
            Some(survivor),
            Some(proof)
        ));
    }

    #[test]
    fn authority_transaction_accounting_excludes_surface_removals() {
        assert_eq!(authority_transaction_count(&[]), 0);
    }

    #[test]
    fn runtime_commit_accounting_records_only_accepted_batches() {
        assert_eq!(record_runtime_commits(166, 1), 167);
        assert_eq!(record_runtime_commits(167, 0), 167);
    }

    #[test]
    fn completed_physical_input_reconciles_pixels_that_arrived_before_return() {
        assert!(physical_input_pixels_already_changed(
            Some(10),
            Some(20),
            true
        ));
        assert!(!physical_input_pixels_already_changed(
            Some(10),
            Some(20),
            false
        ));
        assert!(!physical_input_pixels_already_changed(
            Some(10),
            Some(10),
            true
        ));
    }

    #[test]
    fn physical_pointer_starts_at_focused_surface_center() {
        let raw = Point { x: -4.0, y: 6.0 };
        let offset = pointer_offset_for_geometry(
            raw,
            Rect {
                x: 80,
                y: 60,
                width: 960,
                height: 640,
            },
        );
        assert_eq!(raw.x + offset.x, 560.0);
        assert_eq!(raw.y + offset.y, 380.0);
    }

    #[test]
    fn button_only_pointer_proof_tracks_motion_without_routing_it() {
        let mut pointer = SessionPointerPlacement {
            offset: Some(Point { x: 10.0, y: 20.0 }),
        };
        let mut motion = InputEventPacket {
            serial: 1,
            seat: SeatId::from_raw(1),
            device: DeviceId::from_raw(2),
            time_msec: 1,
            kind: InputEventKind::PointerMotion,
            global_position: Some(Point { x: 30.0, y: 40.0 }),
            target_surface: None,
            local_position: None,
        };

        assert!(!place_pointer_event_for_routing(
            &mut motion,
            None,
            &[],
            &mut pointer,
            true,
        ));
        assert_eq!(motion.global_position, Some(Point { x: 40.0, y: 60.0 }));
    }

    #[test]
    fn secondary_terminal_is_a_pointer_witness_without_a_text_prompt() {
        assert!(SECONDARY_POINTER_WITNESS_SCRIPT.contains("?1000h"));
        assert!(SECONDARY_POINTER_WITNESS_SCRIPT.contains("stty raw -echo"));
        assert!(SECONDARY_POINTER_WITNESS_SCRIPT.contains("Pointer input received"));
        assert!(!SECONDARY_POINTER_WITNESS_SCRIPT.contains("read -r line"));
        assert!(!SECONDARY_POINTER_WITNESS_SCRIPT.contains('\0'));
    }

    #[test]
    fn primary_input_proof_remains_visible_until_session_completion() {
        assert!(PRIMARY_INPUT_PROOF_SCRIPT.contains("sleep 300"));
        assert!(!PRIMARY_INPUT_PROOF_SCRIPT.contains("sleep 5"));
    }

    #[test]
    fn live_x_session_profiles_are_explicit_and_fail_closed() {
        let classic = PersistentXtermSessionConfig::from_args(&[]).unwrap();
        assert_eq!(classic.namespace_profile, NamespaceProfile::ClassicShared);
        assert_eq!(classic.namespace_capabilities, NamespaceCapabilities::NONE);

        let confined =
            PersistentXtermSessionConfig::from_args(&["--namespace-profile=confined".to_owned()])
                .unwrap();
        assert_eq!(confined.namespace_profile, NamespaceProfile::Confined);
        assert_eq!(confined.namespace_capabilities, NamespaceCapabilities::NONE);

        assert!(
            PersistentXtermSessionConfig::from_args(&["--namespace-profile=unknown".to_owned()])
                .unwrap_err()
                .to_string()
                .contains("expected classic or confined")
        );
    }

    #[test]
    fn live_x_output_injection_is_bounded_and_explicit() {
        let config = PersistentXtermSessionConfig::from_args(&[
            "--inject-output-size=1600x900".to_owned(),
            "--inject-surface-resize=960x640".to_owned(),
        ])
        .unwrap();
        assert_eq!(
            config.inject_output_size,
            Some(Size {
                width: 1600,
                height: 900
            })
        );
        assert_eq!(
            config.inject_surface_resize,
            Some(Size {
                width: 960,
                height: 640
            })
        );
        assert!(
            PersistentXtermSessionConfig::from_args(&["--inject-output-size=0x900".to_owned(),])
                .is_err()
        );
        assert!(
            PersistentXtermSessionConfig::from_args(&["--inject-output-size=wide".to_owned(),])
                .is_err()
        );
    }

    #[test]
    fn live_xauthority_file_is_owner_only_valid_and_removed_on_drop() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn field<'a>(record: &'a [u8], offset: &mut usize) -> &'a [u8] {
            let len = usize::from(u16::from_be_bytes([record[*offset], record[*offset + 1]]));
            *offset += 2;
            let value = &record[*offset..*offset + len];
            *offset += len;
            value
        }

        let directory = std::env::temp_dir().join(format!(
            "sophia-live-xauthority-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let (authority, cookie) = LiveXAuthorityFile::create_in(&directory, 77).unwrap();
        let path = authority.path().to_owned();
        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

        let record = std::fs::read(&path).unwrap();
        assert_eq!(u16::from_be_bytes([record[0], record[1]]), 256);
        let mut offset = 2;
        assert_eq!(
            field(&record, &mut offset),
            rustix::system::uname().nodename().to_bytes()
        );
        assert_eq!(field(&record, &mut offset), b"77");
        assert_eq!(field(&record, &mut offset), b"MIT-MAGIC-COOKIE-1");
        assert_eq!(field(&record, &mut offset), cookie);
        assert_eq!(offset, record.len());

        drop(authority);
        assert!(!path.exists());
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn compatibility_surface_is_centered_without_resizing() {
        let geometry = center_geometry_without_scaling(
            Rect {
                x: 19,
                y: 27,
                width: 800,
                height: 600,
            },
            Size {
                width: 1280,
                height: 720,
            },
        );
        assert_eq!(geometry.x, 240);
        assert_eq!(geometry.y, 60);
        assert_eq!(geometry.width, 800);
        assert_eq!(geometry.height, 600);
    }

    #[test]
    fn oversized_compatibility_surface_keeps_size_and_anchors_at_origin() {
        let geometry = center_geometry_without_scaling(
            Rect {
                x: 19,
                y: 27,
                width: 1920,
                height: 1080,
            },
            Size {
                width: 1280,
                height: 720,
            },
        );
        assert_eq!(geometry.x, 0);
        assert_eq!(geometry.y, 0);
        assert_eq!(geometry.width, 1920);
        assert_eq!(geometry.height, 1080);
    }

    #[test]
    fn wayland_presentation_tracks_immediate_or_deferred_native_submission() {
        assert_eq!(required_wayland_presentation_submission(3, 4, false), Ok(4));
        assert_eq!(required_wayland_presentation_submission(4, 4, true), Ok(5));
        assert!(required_wayland_presentation_submission(4, 4, false).is_err());
    }

    #[test]
    fn wayland_presentation_retains_only_the_latest_generation_per_surface() {
        let first = sophia_protocol::SurfaceId::new(1, 1);
        let second = sophia_protocol::SurfaceId::new(2, 1);
        let mut pending = BTreeMap::new();

        retain_latest_wayland_presentation(&mut pending, first, 3, 8);
        retain_latest_wayland_presentation(&mut pending, second, 4, 8);
        retain_latest_wayland_presentation(&mut pending, first, 5, 9);

        assert_eq!(pending.len(), 2);
        assert_eq!(pending.get(&first), Some(&(5, 9)));
        assert_eq!(pending.get(&second), Some(&(4, 8)));
    }

    #[test]
    fn wayland_cpu_composition_waits_for_a_free_scanout_slot() {
        assert!(cpu_frame_submission_ready(true, false, false, false));
        assert!(!cpu_frame_submission_ready(false, false, false, false));
        assert!(!cpu_frame_submission_ready(true, true, false, false));
        assert!(!cpu_frame_submission_ready(true, false, true, false));
        assert!(!cpu_frame_submission_ready(true, false, false, true));
    }

    #[test]
    fn unchanged_cpu_frame_can_complete_without_another_scanout_submission() {
        assert!(cpu_frame_matches_visible_output(1, true, Some(7), 7));
        assert!(!cpu_frame_matches_visible_output(2, true, Some(7), 7));
        assert!(!cpu_frame_matches_visible_output(1, false, Some(7), 7));
        assert!(!cpu_frame_matches_visible_output(1, true, Some(7), 8));
        assert!(!cpu_frame_matches_visible_output(1, true, None, 7));
    }

    #[test]
    fn terminal_readiness_is_scoped_to_the_focused_surface() {
        let focused = SurfaceId::new(21, 1);
        let secondary = SurfaceId::new(22, 1);
        let mut scene = PersistentCpuScene::new(Size {
            width: 4,
            height: 1,
        });
        scene.surfaces.insert(
            focused,
            (
                Rect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                },
                1,
            ),
        );
        scene.surfaces.insert(
            secondary,
            (
                Rect {
                    x: 2,
                    y: 0,
                    width: 2,
                    height: 1,
                },
                2,
            ),
        );
        scene.buffers.insert(1, test_cpu_buffer(1, [0xff; 8]));
        scene.buffers.insert(
            2,
            test_cpu_buffer(2, [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff]),
        );

        assert!(!scene.surface_has_visual_detail(focused));
        assert!(scene.surface_has_visual_detail(secondary));

        scene.buffers.insert(
            1,
            test_cpu_buffer(1, [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0xff]),
        );
        assert!(scene.surface_has_visual_detail(focused));
    }

    #[test]
    fn focused_surface_is_composed_above_an_overlapping_client() {
        let focused = SurfaceId::new(31, 1);
        let secondary = SurfaceId::new(32, 1);
        let geometry = Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        };
        let mut scene = PersistentCpuScene::new(Size {
            width: 2,
            height: 1,
        });
        scene.surfaces.insert(focused, (geometry, 1));
        scene.surfaces.insert(secondary, (geometry, 2));
        let focused_pixels = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        let secondary_pixels = [0, 0, 0, 0xff, 0, 0, 0, 0xff];
        scene.buffers.insert(1, test_cpu_buffer(1, focused_pixels));
        scene
            .buffers
            .insert(2, test_cpu_buffer(2, secondary_pixels));

        assert_eq!(
            scene.compose().unwrap().frame.bytes,
            secondary_pixels.to_vec()
        );
        scene.raise_surface(focused);
        assert_eq!(
            scene.compose().unwrap().frame.bytes,
            focused_pixels.to_vec()
        );
    }

    fn test_cpu_buffer(handle: u64, bytes: [u8; 8]) -> XAuthorityCpuBufferSnapshot {
        XAuthorityCpuBufferSnapshot {
            handle,
            drawable: XResourceId::new(handle, 1),
            size: Size {
                width: 2,
                height: 1,
            },
            stride: 8,
            format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
            generation: 1,
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn committed_wayland_snapshot_preserves_surface_generation_in_render_layers() {
        let layers = layer_snapshots_from_committed(&[CommittedSurfaceState {
            surface: sophia_protocol::SurfaceId::new(9, 1),
            committed_generation: 4,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 300,
                height: 200,
            },
            buffer: BufferSource::CpuBuffer { handle: 99 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 300,
                height: 200,
            }),
        }]);

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].generation, 4);
        assert_eq!(layers[0].source, BufferSource::CpuBuffer { handle: 99 });
    }

    #[test]
    fn newly_observed_surface_seed_preserves_existing_generations() {
        let primary = sophia_protocol::SurfaceId::new(11, 1);
        let secondary = sophia_protocol::SurfaceId::new(12, 1);
        let existing = vec![CommittedSurfaceState {
            surface: primary,
            committed_generation: 7,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            buffer: BufferSource::CpuBuffer { handle: 11 },
            damage: Region::empty(),
        }];
        let new_surface_transaction = SurfaceTransaction {
            transaction: sophia_protocol::TransactionId::from_raw(29),
            authority: AuthorityKind::SophiaX,
            surface: secondary,
            namespace: None,
            target_geometry: Rect {
                x: 20,
                y: 30,
                width: 320,
                height: 200,
            },
            target_buffer: BufferSource::CpuBuffer { handle: 12 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 320,
                height: 200,
            }),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 3,
        };

        let seeded = seed_missing_committed_surfaces(&existing, &[new_surface_transaction]);

        assert_eq!(seeded.len(), 2);
        assert_eq!(seeded[0].surface, primary);
        assert_eq!(seeded[0].committed_generation, 7);
        assert_eq!(seeded[1].surface, secondary);
        assert_eq!(seeded[1].committed_generation, 3);
    }

    #[test]
    fn committed_snapshot_runtime_does_not_replay_authority_transactions() {
        let output = sophia_engine::HeadlessOutput {
            id: sophia_protocol::OutputId::from_raw(17),
            size: Size {
                width: 640,
                height: 480,
            },
            scale: 1,
        };
        let committed = vec![CommittedSurfaceState {
            surface: sophia_protocol::SurfaceId::new(17, 1),
            committed_generation: 5,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            buffer: BufferSource::CpuBuffer { handle: 17 },
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            }),
        }];
        let mut runtime = PersistentBackendRuntime::new_from_committed_surfaces(
            &[output],
            &committed,
            None,
            None,
        )
        .unwrap();

        let report = runtime
            .run_committed_snapshot(&committed, None, None)
            .unwrap();

        assert_eq!(runtime.committed_surfaces(), committed);
        assert_eq!(
            report
                .engine
                .runtime
                .runtime_state
                .authority_transactions_committed,
            0
        );
    }
}

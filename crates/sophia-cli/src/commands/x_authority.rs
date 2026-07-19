use super::prelude::*;
use sophia_x_authority::{
    XAuthorityClientInputEvent, XAuthorityClientSurfaceRoutes, XAuthorityInputDeliveryId,
    XAuthorityInputDeliveryOutcome, XServerFrontendConfig, XServerFrontendRenderDeviceError,
    XServerFrontendRenderDeviceProvider, XServerFrontendRouteBroker, XServerFrontendServiceCommand,
    run_x_server_frontend_routed_until_stopped,
    run_x11_core_socket_server_once_config_traced_with_idle_timeout,
};
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::sync::Arc;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args
        .iter()
        .any(|arg| arg == "x-authority-zenity-render-smoke")
    {
        let report = run_x_authority_zenity_render_smoke()?;
        print_external_probe_smoke_report("x-authority-zenity-render-smoke", &report);
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-authority-vkcube-smoke") {
        let report = run_x_authority_vkcube_smoke()?;
        print_external_probe_smoke_report("x-authority-vkcube-smoke", &report);
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-authority-xterm-two-client-smoke")
    {
        let report = run_x_authority_xterm_two_client_smoke()?;
        println!(
            "x-authority-xterm-two-client-smoke display={} clients={} routed_keys={} initial_generation={} final_generation={} initial_checksum={} final_checksum={} pixel_change={}",
            report.display,
            report.clients,
            report.routed_keys,
            report.initial_generation,
            report.final_generation,
            report.initial_checksum,
            report.final_checksum,
            report.initial_checksum != report.final_checksum,
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-authority-xterm-input-smoke")
    {
        let report = run_x_authority_xterm_input_smoke()?;
        println!(
            "x-authority-xterm-input-smoke display={} keys={} initial_generation={} final_generation={} initial_checksum={} final_checksum={} pixel_change={} text_match={}",
            report.display,
            report.keys,
            report.initial_generation,
            report.final_generation,
            report.initial_checksum,
            report.final_checksum,
            report.initial_checksum != report.final_checksum,
            report.text_match,
        );
        return Ok(true);
    }

    if let Some(spec) = EXTERNAL_PROBE_SMOKES
        .iter()
        .find(|spec| args.iter().any(|arg| arg == spec.command_name))
    {
        let report = run_x_authority_external_probe_smoke_spec(spec)?;
        print_external_probe_smoke_report(spec.command_name, &report);
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-authority-present-pixmap-smoke")
    {
        let report = run_x_authority_present_pixmap_smoke()?;
        println!(
            "x-authority-present-pixmap-smoke display={} extension_opcode={} transactions={} runtime_committed={} runtime_surfaces={}",
            report.display,
            report.extension_opcode,
            report.transactions,
            report.runtime_committed,
            report.runtime_surfaces
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-authority-xlib-put-image-smoke")
    {
        let report = run_x_authority_xlib_put_image_smoke()?;
        println!(
            "x-authority-xlib-put-image-smoke display={} status={} stdout_bytes={} stderr_bytes={} image_ops={} transactions={} runtime_committed={} runtime_surfaces={}",
            report.display,
            report.status,
            report.stdout_bytes,
            report.stderr_bytes,
            report.image_ops,
            report.transactions,
            report.runtime_committed,
            report.runtime_surfaces
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-authority-xlib-drawing-smoke")
    {
        let report = run_x_authority_xlib_drawing_smoke()?;
        println!(
            "x-authority-xlib-drawing-smoke display={} status={} stdout_bytes={} stderr_bytes={} draw_ops={} transactions={} runtime_committed={} runtime_surfaces={}",
            report.display,
            report.status,
            report.stdout_bytes,
            report.stderr_bytes,
            report.draw_ops,
            report.transactions,
            report.runtime_committed,
            report.runtime_surfaces
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-authority-xlib-smoke") {
        let report = run_x_authority_xlib_smoke()?;
        println!(
            "x-authority-xlib-smoke display={} status={} stdout_bytes={} stderr_bytes={} title_bytes={} title_match={}",
            report.display,
            report.status,
            report.stdout_bytes,
            report.stderr_bytes,
            report.title_bytes,
            report.title_match
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-authority-xdpyinfo-smoke") {
        let report = run_x_authority_xdpyinfo_smoke()?;
        println!(
            "x-authority-xdpyinfo-smoke display={} status={} stdout_bytes={} stderr_bytes={} mentions_sophia={} mentions_root={}",
            report.display,
            report.status,
            report.stdout_bytes,
            report.stderr_bytes,
            report.mentions_sophia,
            report.mentions_root
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-authority-x11rb-smoke") {
        let report = run_x_authority_x11rb_smoke()?;
        println!(
            "x-authority-x11rb-smoke display={} window={:#x} title_bytes={} configure_notify={} map_notify={} errors={}",
            report.display,
            report.window,
            report.title_bytes,
            report.configure_notify,
            report.map_notify,
            report.errors
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-authority-x11-smoke") {
        let report = run_x_authority_x11_smoke()?;
        println!(
            "x-authority-x11-smoke setup=ok configure_notify={} map_notify={} property_bytes={} errors={}",
            report.configure_notify, report.map_notify, report.property_bytes, report.errors
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-authority-runtime-smoke") {
        let report = run_x_authority_runtime_smoke()?;
        println!(
            "x-authority-runtime-smoke socket={} surfaces={} transactions={} portal_prompts={} selection_artifacts={}",
            report.socket_path.display(),
            report.surfaces,
            report.transactions,
            report.portal_prompts,
            report.selection_artifacts
        );
        return Ok(true);
    }

    Ok(false)
}

#[derive(Clone, Debug)]
struct XAuthorityX11SmokeReport {
    configure_notify: usize,
    map_notify: usize,
    property_bytes: usize,
    errors: usize,
}

#[derive(Clone, Debug)]
struct XAuthorityX11rbSmokeReport {
    display: String,
    window: u32,
    title_bytes: usize,
    configure_notify: usize,
    map_notify: usize,
    errors: usize,
}

#[derive(Clone, Debug)]
struct XAuthorityXdpyinfoSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    mentions_sophia: bool,
    mentions_root: bool,
}

#[derive(Clone, Debug)]
struct XAuthorityXlibSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    title_bytes: usize,
    title_match: bool,
}

#[derive(Clone, Debug)]
struct XAuthorityXlibDrawingSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    draw_ops: usize,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
}

#[derive(Clone, Debug)]
struct XAuthorityXlibPutImageSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    image_ops: usize,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
}

#[derive(Clone, Debug)]
struct XAuthorityExternalProbeSmokeReport {
    display: String,
    outcome: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    requests: usize,
    opcode_count: usize,
    opcodes: String,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
    cpu_buffers: usize,
    cpu_buffer_bytes: usize,
    nonzero_pixel_bytes: usize,
    ascii_marker_match: bool,
    first_error: Option<String>,
    #[cfg_attr(not(feature = "atomic-scanout-live"), allow(dead_code))]
    observed_transactions: Vec<SurfaceTransaction>,
    #[cfg_attr(not(feature = "atomic-scanout-live"), allow(dead_code))]
    observed_cpu_buffers: Vec<XAuthorityCpuBufferSnapshot>,
}

#[cfg(feature = "atomic-scanout-live")]
#[derive(Clone, Debug)]
pub(crate) struct XAuthorityTerminalRenderProof {
    pub display: String,
    pub requests: usize,
    pub transactions: usize,
    pub runtime_committed: u64,
    pub runtime_surfaces: u64,
    pub cpu_buffers: Vec<XAuthorityCpuBufferSnapshot>,
    pub authority_batches: Vec<AuthorityTransactionIntake>,
}

#[derive(Clone, Debug)]
struct XAuthorityPresentPixmapSmokeReport {
    display: String,
    extension_opcode: u8,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
}

#[derive(Clone, Debug)]
struct XAuthorityRuntimeSmokeReport {
    socket_path: std::path::PathBuf,
    surfaces: usize,
    transactions: usize,
    portal_prompts: usize,
    selection_artifacts: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct XAuthorityXtermInputSmokeReport {
    pub display: String,
    pub keys: usize,
    pub initial_generation: u64,
    pub final_generation: u64,
    pub initial_checksum: u64,
    pub final_checksum: u64,
    pub text_match: bool,
}

struct XtermInputResultFile {
    path: std::path::PathBuf,
}

impl Drop for XtermInputResultFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct XAuthorityXtermTwoClientSmokeReport {
    pub display: String,
    pub clients: usize,
    pub routed_keys: usize,
    pub initial_generation: u64,
    pub final_generation: u64,
    pub initial_checksum: u64,
    pub final_checksum: u64,
}

#[derive(Clone, Copy, Debug)]
enum ExternalProbeDisplayMode {
    Argument(&'static str),
    Environment,
}

#[derive(Clone, Copy, Debug)]
enum ExternalProbePixelProof {
    None,
    Nonzero,
    Ascii(&'static [u8]),
}

#[derive(Clone, Copy, Debug)]
struct ExternalProbeSmokeSpec {
    command_name: &'static str,
    label: &'static str,
    binary: &'static str,
    display_mode: ExternalProbeDisplayMode,
    args: &'static [&'static str],
    display_base: u32,
    namespace: u64,
    require_transactions: bool,
    pixel_proof: ExternalProbePixelProof,
    allow_proof_kill_without_transactions: bool,
    allow_client_failure_without_x_error: bool,
}

const EXTERNAL_PROBE_SMOKES: &[ExternalProbeSmokeSpec] = &[
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xclock-smoke",
        label: "xclock",
        binary: "xclock",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-analog", "-norender", "-update", "1"],
        display_base: 6600,
        namespace: 48,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xeyes-smoke",
        label: "xeyes",
        binary: "xeyes",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[],
        display_base: 6800,
        namespace: 49,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xwininfo-root-smoke",
        label: "xwininfo",
        binary: "xwininfo",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-root"],
        display_base: 6900,
        namespace: 50,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xprop-root-smoke",
        label: "xprop",
        binary: "xprop",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-root"],
        display_base: 7000,
        namespace: 51,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xsetroot-name-smoke",
        label: "xsetroot",
        binary: "xsetroot",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-name", "Sophia Root"],
        display_base: 7100,
        namespace: 52,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xlogo-smoke",
        label: "xlogo",
        binary: "xlogo",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[],
        display_base: 7200,
        namespace: 53,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xmessage-smoke",
        label: "xmessage",
        binary: "xmessage",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["Sophia"],
        display_base: 7300,
        namespace: 54,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xrandr-query-smoke",
        label: "xrandr",
        binary: "xrandr",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["--query"],
        display_base: 7400,
        namespace: 55,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xcalc-smoke",
        label: "xcalc",
        binary: "xcalc",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[],
        display_base: 7500,
        namespace: 56,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xterm-smoke",
        label: "xterm",
        binary: "xterm",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-geometry", "80x24", "-title", "Sophia xterm", "-e", "true"],
        display_base: 7600,
        namespace: 57,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: true,
        allow_client_failure_without_x_error: true,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xterm-render-smoke",
        label: "xterm_render",
        binary: "xterm",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[
            "-geometry",
            "80x24",
            "-title",
            "Sophia xterm",
            "-cm",
            "-dc",
            "-xrm",
            "*numColorRegisters: 2",
            "-tn",
            "vt100",
            "-hold",
            "-e",
            "sh",
            "-c",
            "printf 'Sophia terminal proof\\n'; sleep 5",
        ],
        display_base: 7650,
        namespace: 59,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Ascii(b"Sophia"),
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-zenity-smoke",
        label: "zenity",
        binary: "zenity",
        display_mode: ExternalProbeDisplayMode::Environment,
        args: &[
            "--entry",
            "--title",
            "Sophia zenity",
            "--text",
            "Sophia GTK probe",
        ],
        display_base: 7700,
        namespace: 58,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Nonzero,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-firefox-smoke",
        label: "firefox",
        binary: "firefox",
        display_mode: ExternalProbeDisplayMode::Environment,
        args: &["--new-instance", "--no-remote", "about:blank"],
        display_base: 7800,
        namespace: 61,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Nonzero,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
    },
];

fn run_x_authority_x11_smoke() -> Result<XAuthorityX11SmokeReport, Box<dyn std::error::Error>> {
    use std::io::Write;

    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-authority-x11-{}-{}.sock",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(41))
    });

    wait_for_socket_path(&socket_path)?;
    let mut stream = UnixStream::connect(&socket_path)?;
    stream.write_all(&x11_setup_request(XByteOrder::LittleEndian))?;
    read_x11_setup_success(&mut stream, XByteOrder::LittleEndian)?;

    stream.write_all(&x11_intern_atom_request(
        XByteOrder::LittleEndian,
        false,
        "_NET_WM_NAME",
    ))?;
    let net_wm_name = read_x11_record(&mut stream)?;
    let net_wm_name = read_x11_u32(XByteOrder::LittleEndian, &net_wm_name[8..12]);

    stream.write_all(&x11_intern_atom_request(
        XByteOrder::LittleEndian,
        false,
        "UTF8_STRING",
    ))?;
    let utf8 = read_x11_record(&mut stream)?;
    let utf8 = read_x11_u32(XByteOrder::LittleEndian, &utf8[8..12]);

    stream.write_all(&x11_create_window_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        20,
        30,
        640,
        480,
    ))?;
    let configure = read_x11_record(&mut stream)?;

    stream.write_all(&x11_resource_request(
        XByteOrder::LittleEndian,
        8,
        0x0020_0001,
    ))?;
    let map = read_x11_record(&mut stream)?;

    stream.write_all(&x11_change_property_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        net_wm_name,
        utf8,
        b"Sophia Socket",
    ))?;
    let property_notify = read_x11_record(&mut stream)?;

    stream.write_all(&x11_get_property_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        net_wm_name,
        0,
        0,
        64,
    ))?;
    let property = read_x11_reply(&mut stream, XByteOrder::LittleEndian)?;

    let records = [configure, map, property_notify];
    let configure_notify = records.iter().filter(|record| record[0] == 22).count();
    let map_notify = records.iter().filter(|record| record[0] == 19).count();
    let errors = records.iter().filter(|record| record[0] == 0).count();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    Ok(XAuthorityX11SmokeReport {
        configure_notify,
        map_notify,
        property_bytes: usize::try_from(read_x11_u32(XByteOrder::LittleEndian, &property[16..20]))?,
        errors,
    })
}

fn run_x_authority_x11rb_smoke() -> Result<XAuthorityX11rbSmokeReport, Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        AtomEnum, ConnectionExt, CreateWindowAux, PropMode, WindowClass,
    };
    use x11rb::wrapper::ConnectionExt as _;

    let display_number = 600 + (std::process::id() % 1000);
    let display = format!(":{display_number}");
    let socket_path = std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    std::fs::create_dir_all("/tmp/.X11-unix")?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(42))
    });

    wait_for_socket_path(&socket_path)?;
    let (connection, screen_index) = x11rb::connect(Some(&display))?;
    let screen = &connection.setup().roots[screen_index];
    let net_wm_name = connection
        .intern_atom(false, b"_NET_WM_NAME")?
        .reply()?
        .atom;
    let utf8 = connection.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
    let window = connection.generate_id()?;
    connection.create_window(
        screen.root_depth,
        window,
        screen.root,
        20,
        30,
        320,
        200,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new(),
    )?;
    let title = b"Sophia x11rb";
    connection.change_property8(PropMode::REPLACE, window, net_wm_name, utf8, title)?;
    let property = connection
        .get_property(false, window, net_wm_name, AtomEnum::ANY, 0, 64)?
        .reply()?;
    connection.map_window(window)?;
    connection.flush()?;

    let mut configure_notify = 0usize;
    let mut map_notify = 0usize;
    let mut errors = 0usize;
    for _ in 0..8 {
        match connection.poll_for_event()? {
            Some(Event::ConfigureNotify(_)) => configure_notify += 1,
            Some(Event::MapNotify(_)) => map_notify += 1,
            Some(Event::Error(_)) => errors += 1,
            Some(_) => {}
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }

    drop(connection);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    Ok(XAuthorityX11rbSmokeReport {
        display,
        window,
        title_bytes: property.value.len(),
        configure_notify,
        map_notify,
        errors,
    })
}

fn run_x_authority_xdpyinfo_smoke()
-> Result<XAuthorityXdpyinfoSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(1600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(43))
    });

    wait_for_socket_path(&socket_path)?;
    let output = std::process::Command::new("xdpyinfo")
        .arg("-display")
        .arg(&display)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let report = XAuthorityXdpyinfoSmokeReport {
        display: display.clone(),
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        mentions_sophia: stdout.contains("Sophia") || stderr.contains("Sophia"),
        mentions_root: stdout.contains("root window id") || stderr.contains("root window id"),
    };

    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    if !output.status.success() {
        return Err(format!(
            "xdpyinfo failed for {display}: status={status} stderr={}",
            stderr.trim()
        )
        .into());
    }

    Ok(report)
}

fn run_x_authority_xlib_smoke() -> Result<XAuthorityXlibSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(2600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(44))
    });
    wait_for_socket_path(&socket_path)?;
    let output = run_compiled_xlib_probe(&display, "xlib", XLIB_SMOKE_SOURCE)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let title_bytes = xlib_smoke_title_bytes(&stdout).unwrap_or(0);
    let title_match = stdout.contains("title_match=1");

    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    if !output.status.success() {
        return Err(format!(
            "Xlib smoke failed for {display}: status={status} stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        )
        .into());
    }

    Ok(XAuthorityXlibSmokeReport {
        display,
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        title_bytes,
        title_match,
    })
}

fn run_x_authority_xlib_drawing_smoke()
-> Result<XAuthorityXlibDrawingSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(3600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || -> Result<Vec<SurfaceTransaction>, String> {
        let mut transactions = Vec::new();
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(45),
            |result| {
                if let Some(response) = &result.response {
                    transactions.extend(response.transactions.iter().cloned());
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(transactions)
    });
    wait_for_socket_path(&socket_path)?;
    let output = run_compiled_xlib_probe(&display, "xlib-drawing", XLIB_DRAWING_SMOKE_SOURCE)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let draw_ops = xlib_smoke_field(&stdout, "draw_ops").unwrap_or(0);

    let _ = std::fs::remove_file(&socket_path);
    let transactions = server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")?
        .map_err(|error| format!("X authority X11 socket server failed: {error}"))?;
    let runtime_state = runtime_state_from_observed_transactions(&transactions)?;

    if !output.status.success() {
        return Err(format!(
            "Xlib drawing smoke failed for {display}: status={status} stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        )
        .into());
    }

    Ok(XAuthorityXlibDrawingSmokeReport {
        display,
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        draw_ops,
        transactions: transactions.len(),
        runtime_committed: runtime_state.authority_transactions_committed,
        runtime_surfaces: runtime_state.authority_surfaces_applied,
    })
}

fn run_x_authority_xlib_put_image_smoke()
-> Result<XAuthorityXlibPutImageSmokeReport, Box<dyn std::error::Error>> {
    let (display, socket_path) = temp_xauthority_display(4600)?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || -> Result<Vec<SurfaceTransaction>, String> {
        let mut transactions = Vec::new();
        run_x11_core_socket_server_once_observed(
            &server_path,
            NamespaceId::from_raw(46),
            |result| {
                if let Some(response) = &result.response {
                    transactions.extend(response.transactions.iter().cloned());
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(transactions)
    });
    wait_for_socket_path(&socket_path)?;
    let output = run_compiled_xlib_probe(&display, "xlib-put-image", XLIB_PUT_IMAGE_SMOKE_SOURCE)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or(-1);
    let image_ops = xlib_smoke_field(&stdout, "image_ops").unwrap_or(0);

    let _ = std::fs::remove_file(&socket_path);
    let transactions = server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")?
        .map_err(|error| format!("X authority X11 socket server failed: {error}"))?;
    let runtime_state = runtime_state_from_observed_transactions(&transactions)?;

    if !output.status.success() {
        return Err(format!(
            "Xlib PutImage smoke failed for {display}: status={status} stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        )
        .into());
    }

    Ok(XAuthorityXlibPutImageSmokeReport {
        display,
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        image_ops,
        transactions: transactions.len(),
        runtime_committed: runtime_state.authority_transactions_committed,
        runtime_surfaces: runtime_state.authority_surfaces_applied,
    })
}

fn run_x_authority_external_probe_smoke_spec(
    spec: &ExternalProbeSmokeSpec,
) -> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary(spec.label, spec.binary)?;
    let (display, socket_path) = temp_xauthority_display(spec.display_base)?;
    run_x_authority_external_probe_smoke(
        spec.label,
        &command,
        spec.display_mode,
        spec.args,
        display,
        socket_path,
        NamespaceId::from_raw(spec.namespace),
        spec.require_transactions,
        spec.pixel_proof,
        spec.allow_proof_kill_without_transactions,
        spec.allow_client_failure_without_x_error,
        None,
    )
}

struct ExternalProbeRenderDeviceProvider {
    device: std::fs::File,
}

impl XServerFrontendRenderDeviceProvider for ExternalProbeRenderDeviceProvider {
    fn open_render_device_fd(
        &self,
    ) -> Result<std::os::fd::OwnedFd, XServerFrontendRenderDeviceError> {
        use std::os::fd::AsRawFd as _;

        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/self/fd/{}", self.device.as_raw_fd()))
            .map(std::os::fd::OwnedFd::from)
            .map_err(|_| XServerFrontendRenderDeviceError::OpenFailed)
    }
}

fn run_x_authority_zenity_render_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("zenity_render", "zenity")?;
    let provider = Arc::new(ExternalProbeRenderDeviceProvider {
        device: first_openable_render_node()?,
    });
    let (display, socket_path) = temp_xauthority_display(7760)?;
    run_x_authority_external_probe_smoke(
        "zenity_render",
        &command,
        ExternalProbeDisplayMode::Environment,
        &[
            "--entry",
            "--title",
            "Sophia zenity render",
            "--text",
            "Sophia GTK render-provider probe",
        ],
        display,
        socket_path,
        NamespaceId::from_raw(60),
        true,
        ExternalProbePixelProof::Nonzero,
        false,
        false,
        Some(provider),
    )
}

fn run_x_authority_vkcube_smoke()
-> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("vkcube", "vkcube")?;
    let render_node = first_openable_render_node()?;
    let provider = Arc::new(ExternalProbeRenderDeviceProvider {
        device: render_node,
    });
    let (display, socket_path) = temp_xauthority_display(6680)?;
    run_x_authority_external_probe_smoke(
        "vkcube",
        &command,
        ExternalProbeDisplayMode::Environment,
        &["--wsi", "xcb", "--c", "2", "--suppress_popups"],
        display,
        socket_path,
        NamespaceId::from_raw(58),
        false,
        ExternalProbePixelProof::None,
        true,
        false,
        Some(provider),
    )
}

fn first_openable_render_node() -> Result<std::fs::File, Box<dyn std::error::Error>> {
    let mut candidates = std::fs::read_dir("/dev/dri")?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    for path in candidates {
        if let Ok(device) = std::fs::File::options().read(true).write(true).open(&path) {
            return Ok(device);
        }
    }
    Err("vkcube probe found no openable DRM render node".into())
}

#[cfg(feature = "atomic-scanout-live")]
pub(crate) fn collect_x_authority_xterm_render_authority_batches(
    terminal: &str,
) -> Result<XAuthorityTerminalRenderProof, Box<dyn std::error::Error>> {
    let spec = EXTERNAL_PROBE_SMOKES
        .iter()
        .find(|spec| spec.command_name == "x-authority-xterm-render-smoke")
        .ok_or("xterm render smoke spec is missing")?;
    let command = resolve_external_probe_binary(spec.label, terminal)?;
    let (display, socket_path) = temp_xauthority_display(spec.display_base)?;
    let report = run_x_authority_external_probe_smoke(
        spec.label,
        &command,
        spec.display_mode,
        spec.args,
        display,
        socket_path,
        NamespaceId::from_raw(spec.namespace),
        spec.require_transactions,
        spec.pixel_proof,
        spec.allow_proof_kill_without_transactions,
        spec.allow_client_failure_without_x_error,
        None,
    )?;
    let authority_batches =
        authority_intakes_from_observed_transactions(&report.observed_transactions);
    Ok(XAuthorityTerminalRenderProof {
        display: report.display,
        requests: report.requests,
        transactions: report.transactions,
        runtime_committed: report.runtime_committed,
        runtime_surfaces: report.runtime_surfaces,
        cpu_buffers: report.observed_cpu_buffers,
        authority_batches,
    })
}

pub(crate) fn resolve_external_probe_binary(
    label: &str,
    binary: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let env_name = format!("SOPHIA_XAUTHORITY_{}", label.to_ascii_uppercase());
    if let Ok(override_path) = std::env::var(&env_name) {
        if override_path.is_empty() {
            return Err(format!("{env_name} is set but empty").into());
        }
        return Ok(std::path::PathBuf::from(override_path));
    }

    if binary.contains('/') {
        let path = std::path::PathBuf::from(binary);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{label} probe binary {binary:?} was not found; set {env_name} to override"
        )
        .into());
    }

    let Some(path_var) = std::env::var_os("PATH") else {
        return Err(format!("{label} probe binary {binary:?} needs PATH or {env_name}").into());
    };
    for directory in std::env::split_paths(&path_var) {
        let candidate = directory.join(binary);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "{label} probe binary {binary:?} was not found in PATH; set {env_name} to override"
    )
    .into())
}

pub(crate) fn run_x_authority_xterm_input_smoke()
-> Result<XAuthorityXtermInputSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("xterm", "xterm")?;
    let (display, socket_path) = temp_xauthority_display(150)?;
    let input_result = XtermInputResultFile {
        path: std::env::temp_dir().join(format!(
            "sophia-xterm-input-{}-{}",
            std::process::id(),
            display.trim_start_matches(':')
        )),
    };
    let _result_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&input_result.path)?;
    let server_path = socket_path.clone();
    let (transaction_sender, transaction_receiver) = sync_channel(256);
    let (key_sender, key_receiver) = sync_channel(32);
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_channels(
            &server_path,
            NamespaceId::from_raw(49),
            transaction_sender,
            key_receiver,
        )
    });
    wait_for_socket_path(&socket_path)?;

    let mut child = std::process::Command::new(command)
        .env("DISPLAY", &display)
        .args([
            "-cm",
            "-dc",
            "-geometry",
            "40x8",
            "-e",
            "sh",
            "-c",
            "printf 'type sophia then Return: '; read line; umask 077; printf '%s' \"$line\" > \"$1\"; printf 'received:%s\\n' \"$line\"; sleep 3",
            "sophia-xterm-input",
        ])
        .arg(&input_result.path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut cpu_buffers = std::collections::BTreeMap::new();
    let initial = wait_for_xterm_cpu_state(
        &transaction_receiver,
        &mut child,
        std::time::Instant::now() + Duration::from_secs(6),
        None,
        &mut cpu_buffers,
    )?;
    let mut time_msec = 1u32;
    for keycode in b"sophia"
        .iter()
        .copied()
        .map(x11_keycode_for_ascii)
        .chain(std::iter::once(Some(36)))
    {
        let keycode = keycode.ok_or("input smoke character has no X keycode")?;
        for pressed in [true, false] {
            key_sender.send(
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
    let final_state = wait_for_xterm_cpu_state(
        &transaction_receiver,
        &mut child,
        std::time::Instant::now() + Duration::from_secs(4),
        Some(initial),
        &mut cpu_buffers,
    );

    if child.try_wait()?.is_none() {
        let _ = child.kill();
    }
    let output = child.wait_with_output()?;
    drop(key_sender);
    let server_result = server
        .join()
        .map_err(|_| "X authority xterm input server thread panicked")?;
    let _ = std::fs::remove_file(&socket_path);
    server_result.map_err(|error| format!("X authority xterm input server failed: {error}"))?;
    let final_state = final_state.map_err(|error| {
        format!(
            "{error}; xterm_status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
    })?;
    let received = std::fs::read(&input_result.path)?;
    if received != b"sophia" {
        return Err(format!(
            "xterm input smoke received incorrect terminal bytes: expected=6 received={}",
            received.len(),
        )
        .into());
    }

    Ok(XAuthorityXtermInputSmokeReport {
        display,
        keys: 7,
        initial_generation: initial.0,
        final_generation: final_state.0,
        initial_checksum: initial.1,
        final_checksum: final_state.1,
        text_match: true,
    })
}

/// Launches two independent real xterms against the bounded routed frontend.
///
/// Each terminal receives different Engine-addressed keystrokes only after the
/// authority has observed both clients' surface routes. This is intentionally a
/// compatibility proof, not the normal persistent-session launcher.
pub(crate) fn run_x_authority_xterm_two_client_smoke()
-> Result<XAuthorityXtermTwoClientSmokeReport, Box<dyn std::error::Error>> {
    let command = resolve_external_probe_binary("xterm", "xterm")?;
    let (display, socket_path) = temp_xauthority_display(152)?;
    let server_path = socket_path.clone();
    let (transaction_sender, transaction_receiver) = sync_channel(256);
    let (control_ack_sender, _control_ack_receiver) = sync_channel(64);
    let (input_delivery_sender, input_delivery_receiver) = sync_channel(64);
    let broker = XServerFrontendRouteBroker::with_control_and_input_delivery_senders(
        NonZeroUsize::new(64).unwrap(),
        control_ack_sender,
        input_delivery_sender,
    );
    let input_sender = broker.input_sender();
    let (service_command_sender, service_command_receiver) = sync_channel(1);
    let config = XServerFrontendConfig::new(&server_path, NamespaceId::from_raw(53))?
        .with_max_concurrent_clients(NonZeroUsize::new(2).unwrap());
    let server = std::thread::spawn(move || {
        run_x_server_frontend_routed_until_stopped(
            config,
            transaction_sender,
            broker,
            service_command_receiver,
        )
    });
    wait_for_socket_path(&socket_path)?;

    let mut first = spawn_xterm_two_client_probe(&command, &display, "first", "40x8+40+40")?;
    let mut second = spawn_xterm_two_client_probe(&command, &display, "second", "40x8+420+40")?;
    let mut state = XtermTwoClientState::default();
    let result = (|| {
        wait_for_two_xterm_routes(
            &transaction_receiver,
            &mut first,
            &mut second,
            std::time::Instant::now() + Duration::from_secs(8),
            &mut state,
        )?;
        let initial = state.fingerprint();
        let clients = state.clients.iter().copied().collect::<Vec<_>>();
        if clients.len() != 2 {
            return Err(format!(
                "two-client xterm smoke observed {} routed clients",
                clients.len()
            )
            .into());
        }

        let mut time_msec = 1u32;
        let mut next_delivery = 1u64;
        let first_deliveries = send_xterm_text_to_client(
            &input_sender,
            clients[0],
            b"alpha",
            &mut time_msec,
            &mut next_delivery,
        )?;
        wait_for_xterm_input_deliveries(&input_delivery_receiver, &first_deliveries)?;
        let after_first = wait_for_two_xterm_change(
            &transaction_receiver,
            &mut first,
            &mut second,
            std::time::Instant::now() + Duration::from_secs(5),
            initial,
            &mut state,
        )?;
        let second_deliveries = send_xterm_text_to_client(
            &input_sender,
            clients[1],
            b"bravo",
            &mut time_msec,
            &mut next_delivery,
        )?;
        wait_for_xterm_input_deliveries(&input_delivery_receiver, &second_deliveries)?;
        let final_state = wait_for_two_xterm_change(
            &transaction_receiver,
            &mut first,
            &mut second,
            std::time::Instant::now() + Duration::from_secs(5),
            after_first,
            &mut state,
        )?;

        Ok(XAuthorityXtermTwoClientSmokeReport {
            display: display.clone(),
            clients: clients.len(),
            routed_keys: first_deliveries.len() + second_deliveries.len(),
            initial_generation: initial.0,
            final_generation: final_state.0,
            initial_checksum: initial.1,
            final_checksum: final_state.1,
        })
    })();

    let first_output = stop_xterm_two_client_probe(&mut first)?;
    let second_output = stop_xterm_two_client_probe(&mut second)?;
    let _ = service_command_sender.send(XServerFrontendServiceCommand::StopAccepting);
    drop(service_command_sender);
    drop(input_sender);
    let server_result = server
        .join()
        .map_err(|_| "two-client xterm authority server thread panicked")?;
    let _ = std::fs::remove_file(&socket_path);
    server_result.map_err(|error| format!("two-client X authority server failed: {error}"))?;
    result.map_err(|error: Box<dyn std::error::Error>| {
        format!("{error}; first_status={first_output} second_status={second_output}",).into()
    })
}

#[derive(Default)]
struct XtermTwoClientState {
    routes: XAuthorityClientSurfaceRoutes,
    clients: BTreeSet<sophia_x_authority::XServerFrontendClientId>,
    buffers: BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
}

impl XtermTwoClientState {
    fn observe(
        &mut self,
        batch: XAuthorityObservedTransactionBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(client) = batch.client
            && (!batch.transactions.is_empty() || !batch.cpu_buffer_updates.is_empty())
        {
            self.clients.insert(client);
        }
        self.routes.observe(&batch);
        for update in batch.cpu_buffer_updates {
            update.apply_to(&mut self.buffers)?;
        }
        Ok(())
    }

    fn has_two_live_cpu_routes(&self) -> bool {
        self.clients.len() == 2
            && self.routes.len() >= 2
            && self
                .buffers
                .values()
                .filter(|buffer| buffer.bytes.iter().any(|byte| *byte != 0))
                .count()
                >= 2
    }

    fn fingerprint(&self) -> (u64, u64) {
        let generation = self
            .buffers
            .values()
            .map(|buffer| buffer.generation)
            .max()
            .unwrap_or(0);
        let checksum = self
            .buffers
            .values()
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, buffer| {
                buffer.bytes.iter().fold(hash, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                })
            });
        (generation, checksum)
    }
}

fn spawn_xterm_two_client_probe(
    command: &std::path::Path,
    display: &str,
    label: &str,
    geometry: &str,
) -> Result<std::process::Child, Box<dyn std::error::Error>> {
    let script = format!(
        "printf '{label}: '; IFS= read -r line; printf '{label}-received:%s\\n' \"$line\"; sleep 3"
    );
    Ok(std::process::Command::new(command)
        .env("DISPLAY", display)
        .args(["-cm", "-dc", "-geometry", geometry, "-e", "sh", "-c"])
        .arg(script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?)
}

fn stop_xterm_two_client_probe(
    child: &mut std::process::Child,
) -> Result<std::process::ExitStatus, Box<dyn std::error::Error>> {
    if child.try_wait()?.is_none() {
        let _ = child.kill();
    }
    Ok(child.wait()?)
}

fn wait_for_two_xterm_routes(
    receiver: &std::sync::mpsc::Receiver<XAuthorityObservedTransactionBatch>,
    first: &mut std::process::Child,
    second: &mut std::process::Child,
    deadline: std::time::Instant,
    state: &mut XtermTwoClientState,
) -> Result<(), Box<dyn std::error::Error>> {
    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(batch) => {
                state.observe(batch)?;
                if state.has_two_live_cpu_routes() {
                    return Ok(());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("two-client xterm transaction channel disconnected".into());
            }
        }
        ensure_xterm_two_client_alive(first, "first")?;
        ensure_xterm_two_client_alive(second, "second")?;
    }
    Err(format!(
        "timed out waiting for two routed xterm CPU surfaces: clients={} routes={} buffers={}",
        state.clients.len(),
        state.routes.len(),
        state.buffers.len(),
    )
    .into())
}

fn wait_for_two_xterm_change(
    receiver: &std::sync::mpsc::Receiver<XAuthorityObservedTransactionBatch>,
    first: &mut std::process::Child,
    second: &mut std::process::Child,
    deadline: std::time::Instant,
    previous: (u64, u64),
    state: &mut XtermTwoClientState,
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(batch) => {
                state.observe(batch)?;
                let current = state.fingerprint();
                if current.0 > previous.0 && current.1 != previous.1 {
                    return Ok(current);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("two-client xterm transaction channel disconnected".into());
            }
        }
        ensure_xterm_two_client_alive(first, "first")?;
        ensure_xterm_two_client_alive(second, "second")?;
    }
    Err("timed out waiting for targeted xterm input to change CPU pixels".into())
}

fn ensure_xterm_two_client_alive(
    child: &mut std::process::Child,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(status) = child.try_wait()? {
        return Err(format!("{label} xterm exited before two-client pixel proof: {status}").into());
    }
    Ok(())
}

fn send_xterm_text_to_client(
    sender: &std::sync::mpsc::SyncSender<XAuthorityClientInputEvent>,
    client: sophia_x_authority::XServerFrontendClientId,
    text: &[u8],
    time_msec: &mut u32,
    next_delivery: &mut u64,
) -> Result<Vec<XAuthorityInputDeliveryId>, Box<dyn std::error::Error>> {
    let mut deliveries = Vec::new();
    for keycode in text
        .iter()
        .copied()
        .map(x11_keycode_for_ascii)
        .chain(std::iter::once(Some(36)))
    {
        let keycode = keycode.ok_or("two-client input smoke character has no X keycode")?;
        for pressed in [true, false] {
            let delivery = XAuthorityInputDeliveryId::from_raw(*next_delivery);
            *next_delivery = next_delivery
                .checked_add(1)
                .ok_or("two-client xterm smoke exhausted input delivery IDs")?;
            sender.send(XAuthorityClientInputEvent {
                client,
                event: XAuthorityKeyEvent {
                    keycode,
                    pressed,
                    state: 0,
                    time_msec: *time_msec,
                }
                .into(),
                target_window: None,
                xi_event_type: None,
                xi_transition_mask: 0,
                delivery: Some(delivery),
            })?;
            *time_msec = time_msec.saturating_add(1);
            deliveries.push(delivery);
        }
    }
    Ok(deliveries)
}

fn wait_for_xterm_input_deliveries(
    receiver: &std::sync::mpsc::Receiver<sophia_x_authority::XAuthorityClientInputDelivery>,
    deliveries: &[XAuthorityInputDeliveryId],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pending = deliveries.iter().copied().collect::<BTreeSet<_>>();
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !pending.is_empty() && std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(delivery) if pending.remove(&delivery.delivery) => {
                if delivery.outcome != XAuthorityInputDeliveryOutcome::Flushed {
                    return Err(format!(
                        "two-client xterm input delivery failed for client {}: {:?}",
                        delivery.client.raw(),
                        delivery.outcome,
                    )
                    .into());
                }
            }
            Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("two-client xterm input delivery channel disconnected".into());
            }
        }
    }
    if pending.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "two-client xterm input delivery timed out with {} pending events",
            pending.len(),
        )
        .into())
    }
}

fn wait_for_xterm_cpu_state(
    receiver: &std::sync::mpsc::Receiver<XAuthorityObservedTransactionBatch>,
    child: &mut std::process::Child,
    deadline: std::time::Instant,
    previous: Option<(u64, u64)>,
    latest: &mut std::collections::BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    let mut candidate = None;
    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(batch) => {
                for update in batch.cpu_buffer_updates {
                    update.apply_to(latest)?;
                }
                let generation = latest
                    .values()
                    .map(|buffer| buffer.generation)
                    .max()
                    .unwrap_or(0);
                let checksum = latest
                    .values()
                    .fold(0xcbf2_9ce4_8422_2325u64, |hash, buffer| {
                        buffer.bytes.iter().fold(hash, |hash, byte| {
                            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                        })
                    });
                let has_pixels = latest
                    .values()
                    .any(|buffer| buffer.bytes.iter().any(|byte| *byte != 0));
                if has_pixels {
                    candidate = Some((generation, checksum));
                    if previous.is_some_and(|(old_generation, old_checksum)| {
                        generation > old_generation && checksum != old_checksum
                    }) {
                        return Ok((generation, checksum));
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if previous.is_none()
                    && let Some(candidate) = candidate
                {
                    return Ok(candidate);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("X authority xterm input transaction channel disconnected".into());
            }
        }
        if let Some(status) = child.try_wait()? {
            return Err(format!("xterm input client exited before pixel proof: {status}").into());
        }
    }
    Err("timed out waiting for xterm input to change CPU pixels".into())
}

pub(crate) fn x11_keycode_for_ascii(byte: u8) -> Option<u8> {
    b"qwertyuiop"
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|index| 24 + index as u8)
        .or_else(|| {
            b"asdfghjkl"
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|index| 38 + index as u8)
        })
        .or_else(|| {
            b"zxcvbnm"
                .iter()
                .position(|candidate| *candidate == byte)
                .map(|index| 52 + index as u8)
        })
}

fn print_external_probe_smoke_report(
    command_name: &str,
    report: &XAuthorityExternalProbeSmokeReport,
) {
    println!(
        "{} display={} outcome={} status={} stdout_bytes={} stderr_bytes={} requests={} opcode_count={} opcodes={} transactions={} runtime_committed={} runtime_surfaces={} cpu_buffers={} cpu_buffer_bytes={} nonzero_pixel_bytes={} ascii_marker_match={} first_error={}",
        command_name,
        report.display,
        report.outcome,
        report.status,
        report.stdout_bytes,
        report.stderr_bytes,
        report.requests,
        report.opcode_count,
        report.opcodes,
        report.transactions,
        report.runtime_committed,
        report.runtime_surfaces,
        report.cpu_buffers,
        report.cpu_buffer_bytes,
        report.nonzero_pixel_bytes,
        report.ascii_marker_match,
        report.first_error.as_deref().unwrap_or("none")
    );
}

fn run_x_authority_external_probe_smoke(
    label: &str,
    command: &std::path::Path,
    display_mode: ExternalProbeDisplayMode,
    command_args: &[&str],
    display: String,
    socket_path: std::path::PathBuf,
    namespace: NamespaceId,
    require_transactions: bool,
    pixel_proof: ExternalProbePixelProof,
    allow_proof_kill_without_transactions: bool,
    allow_client_failure_without_x_error: bool,
    render_device_provider: Option<Arc<dyn XServerFrontendRenderDeviceProvider>>,
) -> Result<XAuthorityExternalProbeSmokeReport, Box<dyn std::error::Error>> {
    let server_path = socket_path.clone();
    // One X request can produce an opcode, detail, transaction, and buffer
    // update. Keep the diagnostic channel large enough that a replacement
    // update cannot be dropped while a later patch is retained.
    let (sender, receiver) = sync_channel(4_096);
    let mut server_config = XServerFrontendConfig::new(&server_path, namespace)?;
    if let Some(provider) = render_device_provider {
        server_config = server_config.with_render_device_provider(provider);
    }
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_config_traced_with_idle_timeout(
            server_config,
            Duration::from_secs(8),
            |trace| {
                let _ = sender.try_send(ExternalProbeObservation::Opcode(trace.major_opcode));
                if let Some(detail) = &trace.request_detail {
                    let _ = sender.try_send(ExternalProbeObservation::Detail(detail.clone()));
                }
                if let Some(error) = &trace.parse_error {
                    let _ = sender.try_send(ExternalProbeObservation::Error(format!(
                        "parse_error:major={}:{}",
                        trace.major_opcode, error
                    )));
                }
                for output in &trace.result.outputs {
                    if let XClientOutput::Error(error) = output {
                        if error.code == sophia_x_authority::XErrorCode::BadWindow
                            && error.resource_id == 0
                            && error.minor_code == 0
                            && matches!(error.major_code, 3 | 14)
                        {
                            continue;
                        }
                        let _ = sender.try_send(ExternalProbeObservation::Error(format!(
                            "{:?}:major={}:resource={:#x}",
                            error.code, error.major_code, error.resource_id
                        )));
                    }
                }
                if let Some(response) = &trace.result.response {
                    if !response.transactions.is_empty() {
                        let _ = sender.try_send(ExternalProbeObservation::Transactions(
                            response.transactions.clone(),
                        ));
                    }
                }
                if let Some(buffer) = trace.cpu_buffer_update {
                    let _ =
                        sender.try_send(ExternalProbeObservation::CpuBufferUpdate(buffer.clone()));
                }
                Ok(())
            },
        )
    });
    wait_for_socket_path(&socket_path)?;

    let mut command = std::process::Command::new(command);
    let firefox_profile = (label == "firefox").then(|| {
        std::env::temp_dir().join(format!(
            "sophia-firefox-profile-{}-{}",
            std::process::id(),
            namespace.raw()
        ))
    });
    if let Some(profile) = firefox_profile.as_ref() {
        std::fs::create_dir(profile)?;
        command.arg("--profile").arg(profile);
    }
    match display_mode {
        ExternalProbeDisplayMode::Argument(display_arg) => {
            command.arg(display_arg).arg(&display);
        }
        ExternalProbeDisplayMode::Environment => {
            command
                .env("DISPLAY", &display)
                .env("GDK_BACKEND", "x11")
                .env("GTK_USE_PORTAL", "0")
                .env("MOZ_ENABLE_WAYLAND", "0")
                .env_remove("WAYLAND_DISPLAY");
        }
    }
    command
        .args(command_args)
        .process_group(0)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn()?;

    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    let mut transactions = Vec::new();
    let mut cpu_buffers = std::collections::BTreeMap::new();
    let mut cpu_buffer_updates = 0usize;
    let mut first_error = None;
    let mut opcodes = std::collections::BTreeSet::new();
    let mut details = std::collections::BTreeSet::new();
    let mut requests = 0usize;

    while std::time::Instant::now() < deadline {
        while let Ok(observation) = receiver.try_recv() {
            match observation {
                ExternalProbeObservation::Opcode(opcode) => {
                    requests = requests.saturating_add(1);
                    opcodes.insert(opcode);
                }
                ExternalProbeObservation::Transactions(batch) => transactions.extend(batch),
                ExternalProbeObservation::CpuBufferUpdate(update) => {
                    cpu_buffer_updates = cpu_buffer_updates.saturating_add(1);
                    update.apply_to(&mut cpu_buffers)?;
                }
                ExternalProbeObservation::Detail(detail) => {
                    details.insert(detail);
                }
                ExternalProbeObservation::Error(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        let pixel_proof_ready = match pixel_proof {
            ExternalProbePixelProof::None => true,
            ExternalProbePixelProof::Nonzero => cpu_buffers
                .values()
                .any(|buffer| buffer.bytes.iter().any(|byte| *byte != 0)),
            ExternalProbePixelProof::Ascii(marker) => {
                cpu_buffers_contain_fixed_text(&cpu_buffers, marker)
            }
        };
        if !transactions.is_empty() && pixel_proof_ready {
            break;
        }
        if child.try_wait()?.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut proof_window_killed = false;
    let output = if child.try_wait()?.is_none() {
        if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::TERM);
            std::thread::sleep(Duration::from_millis(25));
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
        proof_window_killed = true;
        let status = child.wait()?;
        std::process::Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    } else {
        child.wait_with_output()?
    };
    let status = output.status.code().unwrap_or(-1);

    let _ = std::fs::remove_file(&socket_path);
    if let Some(profile) = firefox_profile.as_ref() {
        let _ = std::fs::remove_dir_all(profile);
    }
    if !allow_proof_kill_without_transactions || !proof_window_killed {
        server
            .join()
            .map_err(|_| format!("X authority {label} socket server thread panicked"))?
            .map_err(|error| format!("X authority {label} socket server failed: {error}"))?;
    }

    while let Ok(observation) = receiver.try_recv() {
        match observation {
            ExternalProbeObservation::Opcode(opcode) => {
                requests = requests.saturating_add(1);
                opcodes.insert(opcode);
            }
            ExternalProbeObservation::Transactions(batch) => transactions.extend(batch),
            ExternalProbeObservation::CpuBufferUpdate(update) => {
                cpu_buffer_updates = cpu_buffer_updates.saturating_add(1);
                update.apply_to(&mut cpu_buffers)?;
            }
            ExternalProbeObservation::Detail(detail) => {
                details.insert(detail);
            }
            ExternalProbeObservation::Error(error) => {
                first_error.get_or_insert(error);
            }
        }
    }

    let opcode_count = opcodes.len();
    let opcodes = opcodes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let details = details.into_iter().collect::<Vec<_>>().join(",");

    if let Some(error) = &first_error {
        return Err(format!(
            "{label} produced an X protocol error for {display}: status={status} requests={requests} opcode_count={opcode_count} opcodes={opcodes} details={details} first_error={error} stderr={}",
            String::from_utf8_lossy(&output.stderr).trim(),
        )
        .into());
    }

    if require_transactions && transactions.is_empty() {
        return Err(format!(
            "{label} did not produce an authority transaction for {display}: status={status} requests={requests} opcode_count={opcode_count} opcodes={opcodes} details={details} stderr={} first_error={}",
            String::from_utf8_lossy(&output.stderr).trim(),
            first_error.as_deref().unwrap_or("none")
        )
        .into());
    }

    if !require_transactions
        && !output.status.success()
        && !(allow_proof_kill_without_transactions && proof_window_killed)
        && !(allow_client_failure_without_x_error && requests > 0)
    {
        return Err(format!(
            "{label} probe failed for {display}: status={status} requests={requests} opcode_count={opcode_count} opcodes={opcodes} details={details} stderr={} first_error={}",
            String::from_utf8_lossy(&output.stderr).trim(),
            first_error.as_deref().unwrap_or("none")
        )
        .into());
    }

    let runtime_state = if transactions.is_empty() {
        None
    } else {
        Some(runtime_state_from_observed_transactions(&transactions)?)
    };
    let runtime_committed = runtime_state
        .as_ref()
        .map(|state| state.authority_transactions_committed)
        .unwrap_or(0);
    let runtime_surfaces = runtime_state
        .as_ref()
        .map(|state| state.authority_surfaces_applied)
        .unwrap_or(0);
    let cpu_buffer_bytes = cpu_buffers.values().map(|buffer| buffer.bytes.len()).sum();
    let nonzero_pixel_bytes = cpu_buffers
        .values()
        .flat_map(|buffer| buffer.bytes.iter())
        .filter(|byte| **byte != 0)
        .count();
    let ascii_marker_match = cpu_buffers_contain_fixed_text(&cpu_buffers, b"Sophia");
    let pixel_proof_passed = match pixel_proof {
        ExternalProbePixelProof::None => true,
        ExternalProbePixelProof::Nonzero => nonzero_pixel_bytes != 0,
        ExternalProbePixelProof::Ascii(marker) => {
            cpu_buffers_contain_fixed_text(&cpu_buffers, marker)
        }
    };
    if !pixel_proof_passed {
        return Err(format!(
            "{label} did not satisfy its pixel proof for {display}: requests={requests} opcodes={opcodes} details={details}"
        )
        .into());
    }
    if require_transactions && (runtime_committed == 0 || runtime_surfaces == 0) {
        return Err(format!(
            "{label} transactions did not commit through runtime for {display}: transactions={} committed={} surfaces={}",
            transactions.len(),
            runtime_committed,
            runtime_surfaces
        )
        .into());
    }

    let outcome = if proof_window_killed {
        "proof_window_killed"
    } else if output.status.success() {
        "client_exited_success"
    } else {
        "client_exited_failure"
    };

    Ok(XAuthorityExternalProbeSmokeReport {
        display,
        outcome: outcome.to_owned(),
        status,
        stdout_bytes: output.stdout.len(),
        stderr_bytes: output.stderr.len(),
        requests,
        opcode_count,
        opcodes,
        transactions: transactions.len(),
        runtime_committed,
        runtime_surfaces,
        cpu_buffers: cpu_buffer_updates,
        cpu_buffer_bytes,
        nonzero_pixel_bytes,
        ascii_marker_match,
        first_error,
        observed_transactions: transactions,
        observed_cpu_buffers: cpu_buffers.into_values().collect(),
    })
}

fn cpu_buffers_contain_fixed_text(
    buffers: &std::collections::BTreeMap<u64, XAuthorityCpuBufferSnapshot>,
    text: &[u8],
) -> bool {
    buffers.values().any(|buffer| {
        let Ok(width) = usize::try_from(buffer.size.width) else {
            return false;
        };
        let Ok(height) = usize::try_from(buffer.size.height) else {
            return false;
        };
        let Some(text_width) = text.len().checked_mul(8) else {
            return false;
        };
        if width < text_width || height < 12 {
            return false;
        }
        (0..=height - 12).any(|top| {
            (0..=width - text_width).any(|left| fixed_text_matches_at(buffer, left, top, text))
        })
    })
}

fn fixed_text_matches_at(
    buffer: &XAuthorityCpuBufferSnapshot,
    left: usize,
    top: usize,
    text: &[u8],
) -> bool {
    let Some(background) = xrgb_pixel(buffer, left, top) else {
        return false;
    };
    let first_rows = x_fixed_glyph_rows(text[0]);
    let Some((first_row, first_column)) = first_rows.iter().enumerate().find_map(|(row, bits)| {
        (0..5)
            .find(|column| bits & (1 << (4 - column)) != 0)
            .map(|column| (row, column))
    }) else {
        return false;
    };
    let Some(foreground) = xrgb_pixel(
        buffer,
        left.saturating_add(first_column + 1),
        top.saturating_add(first_row + 2),
    ) else {
        return false;
    };
    if foreground == background {
        return false;
    }
    for (index, byte) in text.iter().copied().enumerate() {
        let rows = x_fixed_glyph_rows(byte);
        let cell_left = left.saturating_add(index.saturating_mul(8));
        for (row, bits) in rows.into_iter().enumerate() {
            for column in 0..5 {
                let expected = if bits & (1 << (4 - column)) != 0 {
                    foreground
                } else {
                    background
                };
                if xrgb_pixel(
                    buffer,
                    cell_left.saturating_add(column + 1),
                    top.saturating_add(row + 2),
                ) != Some(expected)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn xrgb_pixel(buffer: &XAuthorityCpuBufferSnapshot, x: usize, y: usize) -> Option<u32> {
    let stride = usize::try_from(buffer.stride).ok()?;
    let offset = y.checked_mul(stride)?.checked_add(x.checked_mul(4)?)?;
    Some(u32::from_le_bytes(
        buffer
            .bytes
            .get(offset..offset.checked_add(4)?)?
            .try_into()
            .ok()?,
    ))
}

#[derive(Clone, Debug)]
enum ExternalProbeObservation {
    Opcode(u8),
    Transactions(Vec<SurfaceTransaction>),
    CpuBufferUpdate(XAuthorityCpuBufferUpdate),
    Detail(String),
    Error(String),
}

fn run_x_authority_present_pixmap_smoke()
-> Result<XAuthorityPresentPixmapSmokeReport, Box<dyn std::error::Error>> {
    let artifacts = run_x_authority_present_pixmap_smoke_artifacts()?;
    let runtime_state = runtime_state_from_observed_batches(&artifacts.batches)?;

    Ok(XAuthorityPresentPixmapSmokeReport {
        display: artifacts.display,
        extension_opcode: artifacts.extension_opcode,
        transactions: artifacts
            .batches
            .iter()
            .map(|batch| batch.transactions.len())
            .sum(),
        runtime_committed: runtime_state.authority_transactions_committed,
        runtime_surfaces: runtime_state.authority_surfaces_applied,
    })
}

#[cfg(feature = "atomic-scanout-live")]
pub(crate) fn collect_x_authority_present_pixmap_authority_batches()
-> Result<Vec<AuthorityTransactionIntake>, Box<dyn std::error::Error>> {
    let artifacts = run_x_authority_present_pixmap_smoke_artifacts()?;
    Ok(authority_intakes_from_observed_batches(&artifacts.batches))
}

#[derive(Clone, Debug)]
struct XAuthorityPresentPixmapSmokeArtifacts {
    display: String,
    extension_opcode: u8,
    batches: Vec<XAuthorityObservedTransactionBatch>,
}

fn run_x_authority_present_pixmap_smoke_artifacts()
-> Result<XAuthorityPresentPixmapSmokeArtifacts, Box<dyn std::error::Error>> {
    use std::io::Write;

    let (display, socket_path) = temp_xauthority_display(5600)?;
    let server_path = socket_path.clone();
    let (sender, receiver) = sync_channel(8);
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_channel(&server_path, NamespaceId::from_raw(47), sender)
    });

    wait_for_socket_path(&socket_path)?;
    let mut stream = UnixStream::connect(&socket_path)?;
    stream.write_all(&x11_setup_request(XByteOrder::LittleEndian))?;
    read_x11_setup_success(&mut stream, XByteOrder::LittleEndian)?;

    stream.write_all(&x11_query_extension_request(
        XByteOrder::LittleEndian,
        X_SOPHIA_PRESENT_EXTENSION_NAME,
    ))?;
    let extension = read_x11_record(&mut stream)?;
    if extension[8] != 1 || extension[9] != X_SOPHIA_PRESENT_MAJOR_OPCODE {
        return Err(format!(
            "SOPHIA-PRESENT query returned present={} opcode={}",
            extension[8], extension[9]
        )
        .into());
    }

    stream.write_all(&x11_create_window_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        20,
        30,
        640,
        480,
    ))?;
    let configure = read_x11_record(&mut stream)?;
    if configure[0] != 22 {
        return Err(format!("expected ConfigureNotify, got record {}", configure[0]).into());
    }

    stream.write_all(&x11_sophia_present_pixmap_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        0x0000_0990,
        (0, 0, 640, 480),
        1,
        250,
    ))?;

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")?
        .map_err(|error| format!("X authority X11 socket server failed: {error}"))?;
    let batches = receiver.try_iter().collect::<Vec<_>>();

    Ok(XAuthorityPresentPixmapSmokeArtifacts {
        display,
        extension_opcode: extension[9],
        batches,
    })
}

fn runtime_state_from_observed_batches(
    batches: &[XAuthorityObservedTransactionBatch],
) -> Result<sophia_runtime::SessionRuntimeState, Box<dyn std::error::Error>> {
    let transactions = batches
        .iter()
        .flat_map(|batch| batch.transactions.iter().cloned())
        .collect::<Vec<_>>();
    let engine = HeadlessEngine::default();
    let committed = seed_committed_states_for_transactions(&transactions);
    let (sender, receiver) = sync_channel(batches.len().max(1));
    for batch in authority_intakes_from_observed_batches(batches) {
        sender.try_send(batch)?;
    }
    let inbox = AuthorityTransactionInbox::new(receiver, batches.len().max(1));
    let mut assembly = HeadlessCompositorBackendAssembly::new(engine.output())
        .with_committed_surfaces(committed)
        .with_authority_inbox(inbox);
    let report = assembly.run_tick(CompositorBackendTickInput {
        x_event_count: u32::try_from(transactions.len()).unwrap_or(u32::MAX),
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layer_templates: layer_templates_from_surface_transactions(&transactions),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    })?;
    Ok(report.runtime.runtime_state)
}

fn authority_intakes_from_observed_batches(
    batches: &[XAuthorityObservedTransactionBatch],
) -> Vec<AuthorityTransactionIntake> {
    batches
        .iter()
        .map(|batch| {
            AuthorityTransactionIntake::new(batch.transaction, batch.transactions.clone())
                .with_surface_removals(batch.removed_surfaces.clone())
        })
        .collect()
}

#[cfg(feature = "atomic-scanout-live")]
fn authority_intakes_from_observed_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<AuthorityTransactionIntake> {
    transactions
        .iter()
        .map(|transaction| {
            AuthorityTransactionIntake::new(transaction.transaction, vec![transaction.clone()])
        })
        .collect()
}

fn runtime_state_from_observed_transactions(
    transactions: &[SurfaceTransaction],
) -> Result<sophia_runtime::SessionRuntimeState, Box<dyn std::error::Error>> {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut committed = seed_committed_states_for_transactions(transactions);
    let mut commits = Vec::new();

    for transaction in transactions {
        commits.push(engine.commit_surface_transactions(
            transaction.transaction,
            std::slice::from_ref(transaction),
            &mut committed,
        ));
    }

    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: u32::try_from(transactions.len()).unwrap_or(u32::MAX),
        authority_commits: commits,
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layers: layer_templates_from_surface_transactions(transactions),
        committed_surfaces: committed,
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    });
    let report = driver.run_with_adapter(output.id, 1, &mut adapter)?;
    Ok(report.runtime_state)
}

fn seed_committed_states_for_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<CommittedSurfaceState> {
    let mut surfaces = std::collections::BTreeMap::new();
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

pub(crate) fn layer_templates_from_surface_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<LayerSnapshot> {
    transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| LayerSnapshot {
            surface: transaction.surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
            geometry: transaction.target_geometry,
            source: BufferSource::None,
            damage: transaction.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: transaction.previous_committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        })
        .collect()
}

fn run_x_authority_runtime_smoke()
-> Result<XAuthorityRuntimeSmokeReport, Box<dyn std::error::Error>> {
    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-authority-runtime-{}-{}.sock",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || run_x_authority_socket_server_once(&server_path));

    wait_for_socket_path(&socket_path)?;
    let mut stream = UnixStream::connect(&socket_path)?;
    let trusted = NamespaceId::from_raw(31);
    let untrusted = NamespaceId::from_raw(32);

    let create_source = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(301),
            namespace: trusted,
            kind: XAuthorityRequestKind::CreateWindow {
                window: XResourceId::new(0xd0, 1),
                surface: SurfaceId::new(301, 1),
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            },
        },
    )?;
    let create_target = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(302),
            namespace: untrusted,
            kind: XAuthorityRequestKind::CreateWindow {
                window: XResourceId::new(0xd1, 1),
                surface: SurfaceId::new(302, 1),
                geometry: Rect {
                    x: 700,
                    y: 20,
                    width: 480,
                    height: 360,
                },
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            },
        },
    )?;
    let present = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(303),
            namespace: trusted,
            kind: XAuthorityRequestKind::PresentPixmap {
                window: XResourceId::new(0xd0, 1),
                pixmap: 0x990,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                }),
                previous_committed_generation: 1,
                timeout_msec: 250,
            },
        },
    )?;
    let _selection_owner = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(304),
            namespace: trusted,
            kind: XAuthorityRequestKind::SetSelectionOwner {
                selection: 1,
                owner: Some(XResourceId::new(0xd0, 1)),
                timestamp: 10,
                selection_timestamp: 10,
                kind: XAuthoritySelectionChangeKind::SetOwner,
            },
        },
    )?;
    let selection = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(305),
            namespace: untrusted,
            kind: XAuthorityRequestKind::RequestSelection {
                requestor: XResourceId::new(0xd1, 1),
                selection: 1,
                target: 2,
                target_name: "UTF8_STRING".to_owned(),
                property: 3,
                time: 11,
                transfer: PortalTransferId::from_raw(401),
            },
        },
    )?;

    let surfaces = create_source.surfaces.len() + create_target.surfaces.len();
    let transactions = present.transactions.len();
    let portal_prompts = selection.portal_commands.len();
    let selection_artifacts = selection.selection_artifacts.len();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority socket server thread panicked")??;

    Ok(XAuthorityRuntimeSmokeReport {
        socket_path,
        surfaces,
        transactions,
        portal_prompts,
        selection_artifacts,
    })
}

fn x11_setup_request(byte_order: XByteOrder) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(byte_order.marker());
    out.push(0);
    push_x11_u16(&mut out, byte_order, 11);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 0);
    out
}

fn x11_create_window_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![1, 24];
    push_x11_u16(&mut out, byte_order, 8);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, 0x20);
    push_x11_i16(&mut out, byte_order, x);
    push_x11_i16(&mut out, byte_order, y);
    push_x11_u16(&mut out, byte_order, width);
    push_x11_u16(&mut out, byte_order, height);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 1);
    push_x11_u32(&mut out, byte_order, 0);
    push_x11_u32(&mut out, byte_order, 0);
    out
}

fn x11_resource_request(byte_order: XByteOrder, opcode: u8, id: u32) -> Vec<u8> {
    let mut out = vec![opcode, 0];
    push_x11_u16(&mut out, byte_order, 2);
    push_x11_u32(&mut out, byte_order, id);
    out
}

fn x11_intern_atom_request(byte_order: XByteOrder, only_if_exists: bool, name: &str) -> Vec<u8> {
    let mut out = vec![16, u8::from(only_if_exists)];
    let len_units = (8 + padded_x11_len(name.len())) / 4;
    push_x11_u16(&mut out, byte_order, len_units as u16);
    push_x11_u16(&mut out, byte_order, name.len() as u16);
    push_x11_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_x11(&mut out);
    out
}

fn x11_query_extension_request(byte_order: XByteOrder, name: &str) -> Vec<u8> {
    let mut out = vec![98, 0];
    let len_units = (8 + padded_x11_len(name.len())) / 4;
    push_x11_u16(&mut out, byte_order, len_units as u16);
    push_x11_u16(&mut out, byte_order, name.len() as u16);
    push_x11_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_x11(&mut out);
    out
}

fn x11_sophia_present_pixmap_request(
    byte_order: XByteOrder,
    window: u32,
    pixmap: u32,
    damage: (i16, i16, u16, u16),
    previous_committed_generation: u64,
    timeout_msec: u32,
) -> Vec<u8> {
    let mut out = vec![
        X_SOPHIA_PRESENT_MAJOR_OPCODE,
        X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE,
    ];
    push_x11_u16(&mut out, byte_order, 8);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, pixmap);
    push_x11_i16(&mut out, byte_order, damage.0);
    push_x11_i16(&mut out, byte_order, damage.1);
    push_x11_u16(&mut out, byte_order, damage.2);
    push_x11_u16(&mut out, byte_order, damage.3);
    push_x11_u64(&mut out, byte_order, previous_committed_generation);
    push_x11_u32(&mut out, byte_order, timeout_msec);
    out
}

fn x11_change_property_request(
    byte_order: XByteOrder,
    window: u32,
    property: u32,
    property_type: u32,
    bytes: &[u8],
) -> Vec<u8> {
    let mut out = vec![18, 0];
    let len_units = (24 + padded_x11_len(bytes.len())) / 4;
    push_x11_u16(&mut out, byte_order, len_units as u16);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, property);
    push_x11_u32(&mut out, byte_order, property_type);
    out.push(8);
    out.extend_from_slice(&[0, 0, 0]);
    push_x11_u32(&mut out, byte_order, bytes.len() as u32);
    out.extend_from_slice(bytes);
    pad_x11(&mut out);
    out
}

fn x11_get_property_request(
    byte_order: XByteOrder,
    window: u32,
    property: u32,
    property_type: u32,
    long_offset: u32,
    long_length: u32,
) -> Vec<u8> {
    let mut out = vec![20, 0];
    push_x11_u16(&mut out, byte_order, 6);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, property);
    push_x11_u32(&mut out, byte_order, property_type);
    push_x11_u32(&mut out, byte_order, long_offset);
    push_x11_u32(&mut out, byte_order, long_length);
    out
}

fn read_x11_setup_success(
    stream: &mut UnixStream,
    byte_order: XByteOrder,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut prefix = [0; 8];
    stream.read_exact(&mut prefix)?;
    if prefix[0] != 1 {
        return Err(format!("X11 setup failed with status {}", prefix[0]).into());
    }
    let body_len = usize::from(read_x11_u16(byte_order, &prefix[6..8])) * 4;
    let mut body = vec![0; body_len];
    stream.read_exact(&mut body)?;
    Ok(())
}

fn read_x11_record(stream: &mut UnixStream) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut record = [0; 32];
    stream.read_exact(&mut record)?;
    Ok(record)
}

fn read_x11_reply(
    stream: &mut UnixStream,
    byte_order: XByteOrder,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut prefix = [0; 32];
    stream.read_exact(&mut prefix)?;
    let body_len = usize::try_from(read_x11_u32(byte_order, &prefix[4..8]))? * 4;
    let mut reply = prefix.to_vec();
    reply.resize(32 + body_len, 0);
    stream.read_exact(&mut reply[32..])?;
    Ok(reply)
}

fn push_x11_u16(out: &mut Vec<u8>, byte_order: XByteOrder, value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_x11_i16(out: &mut Vec<u8>, byte_order: XByteOrder, value: i16) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_x11_u32(out: &mut Vec<u8>, byte_order: XByteOrder, value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_x11_u64(out: &mut Vec<u8>, byte_order: XByteOrder, value: u64) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn read_x11_u16(byte_order: XByteOrder, bytes: &[u8]) -> u16 {
    match byte_order {
        XByteOrder::LittleEndian => u16::from_le_bytes(bytes.try_into().expect("u16 bytes")),
        XByteOrder::BigEndian => u16::from_be_bytes(bytes.try_into().expect("u16 bytes")),
    }
}

fn read_x11_u32(byte_order: XByteOrder, bytes: &[u8]) -> u32 {
    match byte_order {
        XByteOrder::LittleEndian => u32::from_le_bytes(bytes.try_into().expect("u32 bytes")),
        XByteOrder::BigEndian => u32::from_be_bytes(bytes.try_into().expect("u32 bytes")),
    }
}

fn pad_x11(out: &mut Vec<u8>) {
    out.resize(padded_x11_len(out.len()), 0);
}

const fn padded_x11_len(len: usize) -> usize {
    (len + 3) & !3
}

fn send_request(
    stream: &mut UnixStream,
    request: XAuthorityRequestPacket,
) -> Result<sophia_x_authority::XAuthorityResponsePacket, Box<dyn std::error::Error>> {
    write_x_authority_request(stream, &request)?;
    Ok(read_x_authority_response(stream)?)
}

fn temp_xauthority_display(
    base: u32,
) -> Result<(String, std::path::PathBuf), Box<dyn std::error::Error>> {
    let display_number = base + (std::process::id() % 1000);
    let display = format!(":{display_number}");
    let socket_path = std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    std::fs::create_dir_all("/tmp/.X11-unix")?;
    Ok((display, socket_path))
}

fn run_compiled_xlib_probe(
    display: &str,
    name: &str,
    source: &str,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let source_path = std::env::temp_dir().join(format!(
        "sophia-xauthority-{name}-{}-{}.c",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let binary_path = source_path.with_extension("bin");
    std::fs::write(&source_path, source)?;
    let compile = std::process::Command::new("gcc")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .arg("-lX11")
        .output()?;
    if !compile.status.success() {
        let _ = std::fs::remove_file(&source_path);
        return Err(format!(
            "failed to compile {name} smoke: {}",
            String::from_utf8_lossy(&compile.stderr).trim()
        )
        .into());
    }
    let output = std::process::Command::new(&binary_path)
        .env("DISPLAY", display)
        .output()?;
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&binary_path);
    Ok(output)
}

fn xlib_smoke_title_bytes(stdout: &str) -> Option<usize> {
    xlib_smoke_field(stdout, "title_bytes")
}

fn xlib_smoke_field(stdout: &str, name: &str) -> Option<usize> {
    let prefix = format!("{name}=");
    stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&prefix))
        .and_then(|value| value.parse().ok())
}

pub(crate) fn wait_for_socket_path(
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    Err(format!(
        "timed out waiting for X authority socket {}",
        path.display()
    )
    .into())
}

const XLIB_SMOKE_SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "open_display=0\n");
        return 2;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 20, 240, 160, 0, 0, 0);
    Atom net_wm_name = XInternAtom(display, "_NET_WM_NAME", False);
    Atom utf8 = XInternAtom(display, "UTF8_STRING", False);
    const char *title = "Sophia Xlib";
    XStoreName(display, window, title);
    XChangeProperty(display, window, net_wm_name, utf8, 8, PropModeReplace,
                    (const unsigned char *)title, (int)strlen(title));

    Atom actual_type = None;
    int actual_format = 0;
    unsigned long nitems = 0;
    unsigned long bytes_after = 0;
    unsigned char *value = NULL;
    int property_status = XGetWindowProperty(display, window, net_wm_name, 0, 64, False,
                                             AnyPropertyType, &actual_type, &actual_format,
                                             &nitems, &bytes_after, &value);
    if (property_status != Success) {
        fprintf(stderr, "get_property=%d\n", property_status);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 3;
    }

    int title_match = value != NULL && nitems == strlen(title) &&
        memcmp(value, title, strlen(title)) == 0;
    if (value) {
        XFree(value);
    }

    XMapWindow(display, window);
    XSync(display, False);
    printf("window=0x%lx title_bytes=%lu title_match=%d\n", window, nitems, title_match);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return title_match ? 0 : 4;
}
"#;

const XLIB_DRAWING_SMOKE_SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <stdio.h>

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "open_display=0\n");
        return 2;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 20, 240, 160, 0, 0, 0);
    GC gc = XCreateGC(display, window, 0, NULL);
    XMapWindow(display, window);
    XFillRectangle(display, window, gc, 5, 6, 40, 30);
    XSync(display, False);
    printf("window=0x%lx draw_ops=1\n", window);
    XFreeGC(display, gc);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
"#;

const XLIB_PUT_IMAGE_SMOKE_SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "open_display=0\n");
        return 2;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 20, 240, 160, 0, 0, 0);
    GC gc = XCreateGC(display, window, 0, NULL);
    XMapWindow(display, window);

    const int width = 8;
    const int height = 4;
    char *data = calloc((size_t)width * (size_t)height, 4);
    if (!data) {
        fprintf(stderr, "alloc=0\n");
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 3;
    }
    for (int i = 0; i < width * height * 4; ++i) {
        data[i] = (char)(i * 3);
    }

    XImage *image = XCreateImage(display, DefaultVisual(display, screen),
                                 DefaultDepth(display, screen), ZPixmap, 0,
                                 data, width, height, 32, 0);
    if (!image) {
        fprintf(stderr, "create_image=0\n");
        free(data);
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 4;
    }

    XPutImage(display, window, gc, image, 0, 0, 3, 5, width, height);
    XSync(display, False);
    printf("window=0x%lx image_ops=1\n", window);

    XDestroyImage(image);
    XFreeGC(display, gc);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
"#;

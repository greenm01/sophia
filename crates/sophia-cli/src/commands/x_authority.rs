use super::prelude::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
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
struct XAuthorityRuntimeSmokeReport {
    socket_path: std::path::PathBuf,
    surfaces: usize,
    transactions: usize,
    portal_prompts: usize,
    selection_artifacts: usize,
}

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
    let display_number = 1600 + (std::process::id() % 1000);
    let display = format!(":{display_number}");
    let socket_path = std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    std::fs::create_dir_all("/tmp/.X11-unix")?;
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

fn wait_for_socket_path(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
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

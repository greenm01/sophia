use super::prelude::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
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
struct XAuthorityRuntimeSmokeReport {
    socket_path: std::path::PathBuf,
    surfaces: usize,
    transactions: usize,
    portal_prompts: usize,
    selection_artifacts: usize,
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

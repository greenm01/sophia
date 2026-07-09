use crate::prelude::*;
use crate::state::*;

pub fn probe_display(
    display_name: Option<&str>,
    namespaces: StaticNamespaceConfig,
) -> Result<XConnectionProbe, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let required_extensions = query_required_extensions(&connection)?;

    Ok(XConnectionProbe {
        display_name: display_name.map(str::to_owned),
        screen_num,
        required_extensions,
        namespaces,
    })
}

pub fn import_root_window_tree(
    display_name: Option<&str>,
    namespaces: StaticNamespaceConfig,
) -> Result<XRootImport, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let required_extensions = query_required_extensions(&connection)?;
    let mut mirror = import_root_window_tree_from_connection(&connection, screen_num)?;
    let atoms = intern_client_hint_atoms(&connection)?;
    let hints = detect_client_hints(&connection, screen_num, &mirror, atoms)?;
    mirror.apply_client_hints(&hints);

    Ok(XRootImport {
        probe: XConnectionProbe {
            display_name: display_name.map(str::to_owned),
            screen_num,
            required_extensions,
            namespaces,
        },
        mirror,
    })
}

pub fn run_test_client_window(config: TestClientConfig) -> Result<TestClientWindow, XBridgeError> {
    let width = u16::try_from(config.size.width.max(1)).unwrap_or(u16::MAX);
    let height = u16::try_from(config.size.height.max(1)).unwrap_or(u16::MAX);
    let (connection, screen_num) =
        x11rb::connect(config.display_name.as_deref()).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let screen = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?;
    let window = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let gc = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let window_aux = CreateWindowAux::new()
        .background_pixel(screen.white_pixel)
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY);

    connection
        .create_window(
            screen.root_depth,
            window,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &window_aux,
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .create_gc(
            gc,
            window,
            &CreateGCAux::new()
                .foreground(screen.black_pixel)
                .background(screen.white_pixel),
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .map_window(window)
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .poly_fill_rectangle(
            window,
            gc,
            &[Rectangle {
                x: 24,
                y: 24,
                width: width.saturating_sub(48),
                height: height.saturating_sub(48),
            }],
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;

    thread::sleep(Duration::from_millis(config.hold_millis));

    Ok(TestClientWindow {
        window: wrap_xid(window),
        size: Size {
            width: i32::from(width),
            height: i32::from(height),
        },
    })
}

pub fn smoke_readback_display(
    display_name: Option<&str>,
) -> Result<SmokeReadbackReport, XBridgeError> {
    capture_readback_display(display_name).map(|capture| capture.report)
}

pub fn capture_readback_display(
    display_name: Option<&str>,
) -> Result<SmokeReadbackCapture, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let mut mirror = import_root_window_tree_from_connection(&connection, screen_num)?;
    let atoms = intern_client_hint_atoms(&connection)?;
    let hints = detect_client_hints(&connection, screen_num, &mirror, atoms)?;
    mirror.apply_client_hints(&hints);
    mirror.apply_unmanaged_client_fallback();

    let targets = mirror.composite_redirect_targets();
    redirect_composite_targets(&connection, &targets)?;

    let mut pixmaps = CompositePixmapMap::default();
    name_composite_pixmaps(&connection, &targets, &mut pixmaps)?;

    let mut surface_ids = SurfaceIdMap::default();
    let mut surfaces = mirror.emit_surfaces(&mut surface_ids, &pixmaps);
    let mut buffers = CpuBufferStore::default();
    let readbacks = readback_surface_pixmaps(&connection, &mut surfaces, &mut buffers)?;
    let layers = layers_from_surfaces(&surfaces);
    let total_bytes = readbacks
        .iter()
        .map(|readback| readback.bytes.len())
        .sum::<usize>();

    Ok(SmokeReadbackCapture {
        report: SmokeReadbackReport {
            display_name: display_name.map(str::to_owned),
            mirrored_windows: mirror.windows().len(),
            surfaces: surfaces.len(),
            renderable_layers: layers.len(),
            redirect_targets: targets.len(),
            readbacks: readbacks.len(),
            total_bytes,
        },
        surfaces,
        layers,
        readbacks,
    })
}

pub fn redirect_composite_targets<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    connection
        .composite_query_version(0, 4)
        .map_err(|error| XBridgeError::CompositeVersion {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::CompositeVersion {
            message: error.to_string(),
        })?;

    for target in targets {
        connection
            .composite_redirect_window(target.window.xid(), target.update.to_x11())
            .map_err(|error| XBridgeError::CompositeRedirect {
                window: target.window.xid(),
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::CompositeRedirect {
                window: target.window.xid(),
                message: error.to_string(),
            })?;
    }

    Ok(())
}

pub fn name_composite_pixmaps<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
    pixmaps: &mut CompositePixmapMap,
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    for target in targets {
        if pixmaps.pixmap_for_window(target.window).is_some() {
            continue;
        }

        let pixmap = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;

        connection
            .composite_name_window_pixmap(target.window.xid(), pixmap)
            .map_err(|error| XBridgeError::CompositeNamePixmap {
                window: target.window.xid(),
                pixmap,
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::CompositeNamePixmap {
                window: target.window.xid(),
                pixmap,
                message: error.to_string(),
            })?;

        pixmaps.insert_named_pixmap(target.window, pixmap);
    }

    Ok(())
}

pub fn create_damage_trackers<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
    tracker: &mut DamageTracker,
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    connection
        .damage_query_version(1, 1)
        .map_err(|error| XBridgeError::DamageVersion {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::DamageVersion {
            message: error.to_string(),
        })?;

    for target in targets {
        if tracker.damage_for_window(target.window).is_some() {
            continue;
        }

        let damage = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;

        connection
            .damage_create(damage, target.window.xid(), ReportLevel::BOUNDING_BOX)
            .map_err(|error| XBridgeError::DamageCreate {
                window: target.window.xid(),
                damage,
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::DamageCreate {
                window: target.window.xid(),
                damage,
                message: error.to_string(),
            })?;

        tracker.insert_damage(target.window, damage);
    }

    Ok(())
}

pub fn emit_damage_frame(
    tracker: &mut DamageTracker,
    output: OutputId,
    frame_serial: u64,
    buffer_age: u32,
    root_generation: u64,
    surfaces: &[SurfaceSnapshot],
) -> DamageFrame {
    let mut affected_surfaces = Vec::new();
    let mut seen_surfaces = BTreeSet::new();
    let mut damage = Region::empty();

    for surface in surfaces {
        let Some(client) = surface.client else {
            continue;
        };

        let local_damage = tracker.drain_damage(client);
        if local_damage.is_empty() || !surface.mapped {
            continue;
        }

        let translated = translate_region(&local_damage, surface.geometry.x, surface.geometry.y);
        if translated.is_empty() {
            continue;
        }

        if seen_surfaces.insert(surface.surface) {
            affected_surfaces.push(surface.surface);
        }
        damage.extend(&translated);
    }

    DamageFrame {
        output,
        frame_serial,
        buffer_age,
        root_generation,
        affected_surfaces,
        damage,
    }
}

pub fn readback_composite_pixmap<C>(
    connection: &C,
    pixmap: u32,
    buffers: &mut CpuBufferStore,
) -> Result<CpuBufferSnapshot, XBridgeError>
where
    C: Connection,
{
    let geometry = connection
        .get_geometry(pixmap)
        .map_err(|error| XBridgeError::PixmapGeometry {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapGeometry {
            pixmap,
            message: error.to_string(),
        })?;
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            pixmap,
            0,
            0,
            geometry.width,
            geometry.height,
            u32::MAX,
        )
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?;

    Ok(buffers.upsert_pixmap(
        pixmap,
        Size {
            width: i32::from(geometry.width),
            height: i32::from(geometry.height),
        },
        image.depth,
        image.visual,
        image.data,
    ))
}

pub fn readback_surface_pixmaps<C>(
    connection: &C,
    surfaces: &mut [SurfaceSnapshot],
    buffers: &mut CpuBufferStore,
) -> Result<Vec<CpuBufferSnapshot>, XBridgeError>
where
    C: Connection,
{
    let mut readbacks = Vec::new();

    for surface in surfaces {
        let BufferSource::XPixmap { pixmap } = surface.source else {
            continue;
        };
        let readback = readback_composite_pixmap(connection, pixmap, buffers)?;
        surface.source = BufferSource::CpuBuffer {
            handle: readback.handle,
        };
        readbacks.push(readback);
    }

    Ok(readbacks)
}

pub fn layers_from_surfaces(surfaces: &[SurfaceSnapshot]) -> Vec<LayerSnapshot> {
    surfaces
        .iter()
        .filter(|surface| surface.mapped && !surface.geometry.is_empty())
        .map(|surface| LayerSnapshot {
            surface: surface.surface,
            window: Some(surface.window),
            namespace: surface.namespace,
            stack_rank: surface.stack_rank,
            geometry: surface.geometry,
            source: surface.source,
            damage: surface.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: surface.generation,
            resize_sync: surface.resize_sync,
        })
        .collect()
}

fn translate_region(region: &Region, dx: i32, dy: i32) -> Region {
    let mut translated = Region::empty();
    for rect in &region.rects {
        translated.push(Rect {
            x: rect.x.saturating_add(dx),
            y: rect.y.saturating_add(dy),
            width: rect.width,
            height: rect.height,
        });
    }
    translated
}

fn query_required_extensions<C>(connection: &C) -> Result<Vec<ExtensionStatus>, XBridgeError>
where
    C: Connection,
{
    let mut required_extensions = Vec::with_capacity(RequiredExtension::ALL.len());

    for extension in RequiredExtension::ALL {
        let reply = connection
            .query_extension(extension.name().as_bytes())
            .map_err(|error| XBridgeError::QueryExtension {
                extension,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::QueryExtension {
                extension,
                message: error.to_string(),
            })?;

        required_extensions.push(ExtensionStatus {
            extension,
            present: reply.present,
            major_opcode: reply.present.then_some(reply.major_opcode),
            first_event: reply.present.then_some(reply.first_event),
            first_error: reply.present.then_some(reply.first_error),
        });
    }

    Ok(required_extensions)
}

fn intern_client_hint_atoms<C>(connection: &C) -> Result<XAtoms, XBridgeError>
where
    C: Connection,
{
    Ok(XAtoms {
        wm_state: intern_atom(connection, "WM_STATE")?,
        net_client_list: intern_atom(connection, "_NET_CLIENT_LIST")?,
        wm_protocols: intern_atom(connection, "WM_PROTOCOLS")?,
        wm_delete_window: intern_atom(connection, "WM_DELETE_WINDOW")?,
    })
}

pub fn intern_selection_atoms<C>(connection: &C) -> Result<XSelectionAtoms, XBridgeError>
where
    C: Connection,
{
    Ok(XSelectionAtoms {
        primary: intern_atom(connection, "PRIMARY")?,
        secondary: intern_atom(connection, "SECONDARY")?,
        clipboard: intern_atom(connection, "CLIPBOARD")?,
    })
}

pub fn select_selection_owner_events<C>(
    connection: &C,
    window: Window,
    selections: &[Atom],
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    let mask = SelectionEventMask::SET_SELECTION_OWNER
        | SelectionEventMask::SELECTION_WINDOW_DESTROY
        | SelectionEventMask::SELECTION_CLIENT_CLOSE;

    for selection in selections {
        connection
            .xfixes_select_selection_input(window, *selection, mask)
            .map_err(|error| XBridgeError::SelectionMonitor {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::SelectionMonitor {
                message: error.to_string(),
            })?;
    }

    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;

    Ok(())
}

pub(crate) fn intern_atom<C>(connection: &C, name: &str) -> Result<Atom, XBridgeError>
where
    C: Connection,
{
    connection
        .intern_atom(false, name.as_bytes())
        .map_err(|error| XBridgeError::InternAtom {
            atom: name.to_owned(),
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| XBridgeError::InternAtom {
            atom: name.to_owned(),
            message: error.to_string(),
        })
}

fn detect_client_hints<C>(
    connection: &C,
    screen_num: usize,
    mirror: &XMirrorState,
    atoms: XAtoms,
) -> Result<XClientHints, XBridgeError>
where
    C: Connection,
{
    let root = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?
        .root;
    let ewmh_clients = read_window_list_property(connection, root, atoms.net_client_list)?
        .into_iter()
        .map(wrap_xid)
        .collect();
    let mut icccm_clients = Vec::new();

    for mirror in mirror.windows() {
        if has_property(connection, mirror.window.xid(), atoms.wm_state)? {
            icccm_clients.push(mirror.window);
        }
    }

    Ok(XClientHints {
        ewmh_clients,
        icccm_clients,
    })
}

fn read_window_list_property<C>(
    connection: &C,
    window: Window,
    property: Atom,
) -> Result<Vec<Window>, XBridgeError>
where
    C: Connection,
{
    let reply = connection
        .get_property(false, window, property, AtomEnum::WINDOW, 0, u32::MAX / 4)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?;

    Ok(reply
        .value32()
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default())
}

fn has_property<C>(connection: &C, window: Window, property: Atom) -> Result<bool, XBridgeError>
where
    C: Connection,
{
    connection
        .get_property(false, window, property, AtomEnum::ANY, 0, 0)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.type_ != 0)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })
}

pub fn polite_close_surface<C>(
    connection: &C,
    mirror: &XMirrorState,
    surfaces: &SurfaceIdMap,
    atoms: XAtoms,
    surface: SurfaceId,
    timestamp: u32,
) -> Result<PoliteCloseOutcome, XBridgeError>
where
    C: Connection,
{
    let target = close_target_for_surface(mirror, surfaces, surface).ok_or_else(|| {
        XBridgeError::PoliteClose {
            window: 0,
            message: format!("surface {:?} has no X close target", surface),
        }
    })?;

    polite_close_window(connection, target, atoms, timestamp)
}

pub fn polite_close_window<C>(
    connection: &C,
    window: XWindowId,
    atoms: XAtoms,
    timestamp: u32,
) -> Result<PoliteCloseOutcome, XBridgeError>
where
    C: Connection,
{
    if !window_supports_wm_delete(connection, window, atoms)? {
        return Ok(PoliteCloseOutcome::UnsupportedProtocol { window });
    }

    let event = build_wm_delete_client_message(window, atoms, timestamp);
    connection
        .send_event(false, window.xid(), EventMask::NO_EVENT, event)
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?;

    Ok(PoliteCloseOutcome::SentDeleteWindow { window })
}

pub fn build_wm_delete_client_message(
    window: XWindowId,
    atoms: XAtoms,
    timestamp: u32,
) -> ClientMessageEvent {
    ClientMessageEvent::new(
        32,
        window.xid(),
        atoms.wm_protocols,
        ClientMessageData::from([atoms.wm_delete_window, timestamp, 0, 0, 0]),
    )
}

fn window_supports_wm_delete<C>(
    connection: &C,
    window: XWindowId,
    atoms: XAtoms,
) -> Result<bool, XBridgeError>
where
    C: Connection,
{
    Ok(
        read_atom_list_property(connection, window.xid(), atoms.wm_protocols)?
            .into_iter()
            .any(|atom| atom == atoms.wm_delete_window),
    )
}

fn read_atom_list_property<C>(
    connection: &C,
    window: Window,
    property: Atom,
) -> Result<Vec<Atom>, XBridgeError>
where
    C: Connection,
{
    let reply = connection
        .get_property(false, window, property, AtomEnum::ATOM, 0, u32::MAX / 4)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?;

    Ok(reply
        .value32()
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default())
}

fn import_root_window_tree_from_connection<C>(
    connection: &C,
    screen_num: usize,
) -> Result<XMirrorState, XBridgeError>
where
    C: Connection,
{
    let root = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?
        .root;
    let mut queue = VecDeque::from([(root, None, 0)]);
    let mut visited = BTreeSet::new();
    let mut mirror = XMirrorState::default();

    while let Some((window, parent, stack_rank)) = queue.pop_front() {
        if !visited.insert(window) {
            continue;
        }

        let tree = connection
            .query_tree(window)
            .map_err(|error| XBridgeError::QueryTree {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::QueryTree {
                window,
                message: error.to_string(),
            })?;
        let attributes = connection
            .get_window_attributes(window)
            .map_err(|error| XBridgeError::WindowAttributes {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::WindowAttributes {
                window,
                message: error.to_string(),
            })?;
        let geometry = connection
            .get_geometry(window)
            .map_err(|error| XBridgeError::WindowGeometry {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::WindowGeometry {
                window,
                message: error.to_string(),
            })?;

        for (rank, child) in tree.children.iter().copied().enumerate() {
            let rank = u32::try_from(rank).expect("X child stack rank overflow");
            queue.push_back((child, Some(window), rank));
        }

        mirror.ingest_window(XWindowMirror {
            window: wrap_xid(window),
            parent: parent.map(wrap_xid),
            children: tree.children.iter().copied().map(wrap_xid).collect(),
            toplevel: None,
            client: None,
            mapped: u8::from(attributes.map_state) == u8::from(MapState::VIEWABLE),
            stack_rank,
            geometry: Rect {
                x: i32::from(geometry.x),
                y: i32::from(geometry.y),
                width: i32::from(geometry.width),
                height: i32::from(geometry.height),
            },
            namespace: None,
            stale_metadata: 0,
        });
    }

    Ok(mirror)
}

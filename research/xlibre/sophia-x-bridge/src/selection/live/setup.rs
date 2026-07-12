use super::*;

pub(super) struct LiveClipboardPortalSmokeSetup {
    pub connection: x11rb::rust_connection::RustConnection,
    pub requestor: u32,
    pub owner_window: XWindowId,
    pub requestor_window: XWindowId,
    pub selection: Atom,
    pub target: Atom,
    pub denied_property: Atom,
    pub approved_property: Atom,
    pub mirror: XMirrorState,
    pub monitor: XSelectionMonitor,
    pub selection_generation: u64,
}

pub(super) fn create_live_clipboard_portal_smoke_setup(
    display_name: Option<&str>,
) -> Result<LiveClipboardPortalSmokeSetup, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let screen = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?;
    let root = screen.root;
    let root_depth = screen.root_depth;
    let root_visual = screen.root_visual;
    let owner = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let requestor = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let selection = intern_atom(&connection, "CLIPBOARD")?;
    let target = intern_atom(&connection, "UTF8_STRING")?;
    let denied_property = intern_atom(&connection, "SOPHIA_CLIPBOARD_DENIED")?;
    let approved_property = intern_atom(&connection, "SOPHIA_CLIPBOARD_APPROVED")?;

    create_clipboard_smoke_window(&connection, root, root_depth, root_visual, owner)?;
    create_clipboard_smoke_window(&connection, root, root_depth, root_visual, requestor)?;
    connection
        .set_selection_owner(owner, selection, 0u32)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    let current_owner = connection
        .get_selection_owner(selection)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .owner;
    if current_owner != owner {
        return Err(XBridgeError::SelectionMonitor {
            message: format!("selection owner is {current_owner:#x}, expected {owner:#x}"),
        });
    }
    drain_pending_events(&connection)?;

    let source_namespace = NamespaceId::from_raw(10);
    let target_namespace = NamespaceId::from_raw(20);
    let owner_window = XWindowId::new(owner, 1);
    let requestor_window = XWindowId::new(requestor, 1);
    let mut mirror = XMirrorState::default();
    mirror.ingest_window(clipboard_smoke_mirror(owner_window, source_namespace));
    mirror.ingest_window(clipboard_smoke_mirror(requestor_window, target_namespace));
    let mut monitor = XSelectionMonitor::new();
    let update = monitor.apply_event(
        XSelectionEvent {
            selection,
            owner: Some(owner_window),
            timestamp: 1,
            selection_timestamp: 1,
            kind: XSelectionChangeKind::SetOwner,
        },
        &mirror,
    );

    Ok(LiveClipboardPortalSmokeSetup {
        connection,
        requestor,
        owner_window,
        requestor_window,
        selection,
        target,
        denied_property,
        approved_property,
        mirror,
        monitor,
        selection_generation: update.current.generation,
    })
}

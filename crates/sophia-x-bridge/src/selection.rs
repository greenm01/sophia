use crate::live::intern_atom;
use crate::prelude::*;
use crate::routed_input::drain_pending_events;
use crate::state::*;

fn create_clipboard_smoke_window<C>(
    connection: &C,
    root: Window,
    depth: u8,
    visual: u32,
    window: Window,
) -> Result<(), XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    connection
        .create_window(
            depth,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })
}

fn wait_for_selection_request<C>(
    connection: &C,
    requestor: Window,
    selection: Atom,
    property: Atom,
    timeout: Duration,
) -> Result<SelectionRequestEvent, XBridgeError>
where
    C: Connection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(event) =
            connection
                .poll_for_event()
                .map_err(|error| XBridgeError::SelectionMonitor {
                    message: error.to_string(),
                })?
        {
            if let Event::SelectionRequest(event) = event {
                if event.requestor == requestor
                    && event.selection == selection
                    && event.property == property
                {
                    return Ok(event);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::SelectionMonitor {
        message: format!(
            "timed out waiting for SelectionRequest requestor={requestor:#x} selection={selection:#x} property={property:#x}"
        ),
    })
}

fn wait_for_selection_notify<C>(
    connection: &C,
    requestor: Window,
    selection: Atom,
    target: Atom,
    timeout: Duration,
) -> Result<SelectionNotifyEvent, XBridgeError>
where
    C: Connection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(event) =
            connection
                .poll_for_event()
                .map_err(|error| XBridgeError::SelectionMonitor {
                    message: error.to_string(),
                })?
        {
            if let Event::SelectionNotify(event) = event {
                if event.requestor == requestor
                    && event.selection == selection
                    && event.target == target
                {
                    return Ok(event);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::SelectionMonitor {
        message: format!(
            "timed out waiting for SelectionNotify requestor={requestor:#x} selection={selection:#x} target={target:#x}"
        ),
    })
}

fn clipboard_smoke_mirror(window: XWindowId, namespace: NamespaceId) -> XWindowMirror {
    XWindowMirror {
        window,
        parent: None,
        children: Vec::new(),
        toplevel: Some(window),
        client: Some(window),
        mapped: true,
        stack_rank: 0,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        namespace: Some(namespace),
        stale_metadata: 0,
    }
}
pub fn dispatch_clipboard_selection_request_event(
    event: &Event,
    target_name: impl Into<String>,
    monitor: &XSelectionMonitor,
    mirror: &XMirrorState,
    transfer: PortalTransferId,
    portal: &mut ClipboardPortal,
) -> Result<ClipboardSelectionDispatch, ClipboardSelectionDispatchError> {
    let Event::SelectionRequest(event) = event else {
        return Err(ClipboardSelectionDispatchError::NotSelectionRequest);
    };
    let portal_request = clipboard_portal_request_from_selection_request(
        event,
        target_name,
        monitor,
        mirror,
        transfer,
    )
    .map_err(ClipboardSelectionDispatchError::Request)?;
    let command = portal
        .request_import(portal_request.request.clone())
        .map_err(ClipboardSelectionDispatchError::Portal)?;

    Ok(ClipboardSelectionDispatch {
        portal_request,
        command,
    })
}

pub fn clipboard_portal_request_from_selection_request(
    event: &SelectionRequestEvent,
    target_name: impl Into<String>,
    monitor: &XSelectionMonitor,
    mirror: &XMirrorState,
    transfer: PortalTransferId,
) -> Result<ClipboardSelectionPortalRequest, ClipboardSelectionRequestError> {
    let target_namespace = mirror
        .namespace_for_window(wrap_xid(event.requestor))
        .ok_or(ClipboardSelectionRequestError::UnknownRequestorNamespace)?;
    let source_owner = monitor
        .current_owner_for_selection(event.selection)
        .ok_or(ClipboardSelectionRequestError::UnknownSourceOwner)?;
    let source_namespace = source_owner
        .namespace
        .ok_or(ClipboardSelectionRequestError::MissingSourceNamespace)?;

    if source_namespace == target_namespace {
        return Err(ClipboardSelectionRequestError::SameNamespace);
    }

    Ok(ClipboardSelectionPortalRequest {
        request: ClipboardTransferRequest {
            transfer,
            source_namespace,
            target_namespace,
            target: ClipboardTarget::Atom(target_name.into()),
            byte_size: 0,
            generation: source_owner.generation,
        },
        failure: ClipboardSelectionFailureRequest {
            transfer,
            requestor: event.requestor,
            selection: event.selection,
            target: event.target,
            time: event.time,
        },
        property: event.property,
    })
}

pub const MAX_CLIPBOARD_TEXT_HANDOFF_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardTextProperty {
    pub requestor: Window,
    pub property: Atom,
    pub target: Atom,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ClipboardSelectionHandoff {
    pub transfer: PortalTransferId,
    pub property: ClipboardTextProperty,
    pub event: SelectionNotifyEvent,
}

impl ClipboardSelectionHandoff {
    pub fn succeeded_normally(&self) -> bool {
        self.event.property == self.property.property
            && self.event.property != u32::from(AtomEnum::NONE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardSelectionHandoffError {
    NotHandoffCommand,
    TransferMismatch,
    MissingProperty,
    UnsupportedTarget,
    TextTooLarge { len: usize, max: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveClipboardPortalSmokeReport {
    pub display_name: Option<String>,
    pub owner: XWindowId,
    pub requestor: XWindowId,
    pub selection: Atom,
    pub target: Atom,
    pub denied_property: Atom,
    pub approved_property: Atom,
    pub failure_property: Atom,
    pub success_property: Atom,
    pub handoff_bytes: usize,
    pub observed_handoff_bytes: usize,
}

pub fn clipboard_selection_text_handoff_notify(
    command: &PortalCommand,
    request: &ClipboardSelectionPortalRequest,
    text: impl AsRef<str>,
) -> Result<ClipboardSelectionHandoff, ClipboardSelectionHandoffError> {
    let PortalCommand::HandoffClipboard { transfer } = command else {
        return Err(ClipboardSelectionHandoffError::NotHandoffCommand);
    };

    if *transfer != request.request.transfer {
        return Err(ClipboardSelectionHandoffError::TransferMismatch);
    }

    if request.property == u32::from(AtomEnum::NONE) {
        return Err(ClipboardSelectionHandoffError::MissingProperty);
    }

    if !request.request.target.is_text() {
        return Err(ClipboardSelectionHandoffError::UnsupportedTarget);
    }

    let bytes = text.as_ref().as_bytes();
    if bytes.len() > MAX_CLIPBOARD_TEXT_HANDOFF_BYTES {
        return Err(ClipboardSelectionHandoffError::TextTooLarge {
            len: bytes.len(),
            max: MAX_CLIPBOARD_TEXT_HANDOFF_BYTES,
        });
    }

    let bytes = bytes.to_vec();
    let failure = request.failure;
    Ok(ClipboardSelectionHandoff {
        transfer: *transfer,
        property: ClipboardTextProperty {
            requestor: failure.requestor,
            property: request.property,
            target: failure.target,
            bytes,
        },
        event: SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: failure.time,
            requestor: failure.requestor,
            selection: failure.selection,
            target: failure.target,
            property: request.property,
        },
    })
}

pub fn smoke_live_clipboard_portal(
    display_name: Option<&str>,
) -> Result<LiveClipboardPortalSmokeReport, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let screen = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?;
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

    create_clipboard_smoke_window(
        &connection,
        screen.root,
        screen.root_depth,
        screen.root_visual,
        owner,
    )?;
    create_clipboard_smoke_window(
        &connection,
        screen.root,
        screen.root_depth,
        screen.root_visual,
        requestor,
    )?;
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

    connection
        .convert_selection(requestor, selection, target, denied_property, 0u32)
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
    let deny_request = wait_for_selection_request(
        &connection,
        requestor,
        selection,
        denied_property,
        Duration::from_secs(2),
    )?;
    let mut portal = ClipboardPortal::new();
    let deny_dispatch = dispatch_clipboard_selection_request_event(
        &Event::SelectionRequest(deny_request),
        "UTF8_STRING",
        &monitor,
        &mirror,
        PortalTransferId::from_raw(1),
        &mut portal,
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to dispatch denied selection request: {error:?}"),
    })?;
    let PortalCommand::FailSelection { transfer } = portal
        .deny(PortalTransferId::from_raw(1))
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: format!("failed to deny clipboard request: {error:?}"),
        })?
    else {
        return Err(XBridgeError::SelectionMonitor {
            message: "clipboard denial did not return FailSelection".to_owned(),
        });
    };
    let failure = clipboard_selection_failure_notify(deny_dispatch.portal_request.failure);
    if failure.transfer != transfer {
        return Err(XBridgeError::SelectionMonitor {
            message: "clipboard denial returned mismatched transfer".to_owned(),
        });
    }
    apply_clipboard_selection_failure(&connection, &failure)?;
    let failure_notify = wait_for_selection_notify(
        &connection,
        requestor,
        selection,
        target,
        Duration::from_secs(2),
    )?;
    if failure_notify.property != u32::from(AtomEnum::NONE) {
        return Err(XBridgeError::SelectionMonitor {
            message: format!(
                "denied clipboard request returned property {:#x}, expected None",
                failure_notify.property
            ),
        });
    }

    connection
        .convert_selection(requestor, selection, target, approved_property, 0u32)
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
    let approve_request = wait_for_selection_request(
        &connection,
        requestor,
        selection,
        approved_property,
        Duration::from_secs(2),
    )?;
    let approve_dispatch = dispatch_clipboard_selection_request_event(
        &Event::SelectionRequest(approve_request),
        "UTF8_STRING",
        &monitor,
        &mirror,
        PortalTransferId::from_raw(2),
        &mut portal,
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to dispatch approved selection request: {error:?}"),
    })?;
    let command = portal
        .approve_generation(PortalTransferId::from_raw(2), update.current.generation)
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: format!("failed to approve clipboard request: {error:?}"),
        })?;
    let handoff = clipboard_selection_text_handoff_notify(
        &command,
        &approve_dispatch.portal_request,
        "hello from Sophia",
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to build clipboard handoff: {error:?}"),
    })?;
    apply_clipboard_selection_handoff(&connection, &handoff)?;
    let success_notify = wait_for_selection_notify(
        &connection,
        requestor,
        selection,
        target,
        Duration::from_secs(2),
    )?;
    if success_notify.property != approved_property {
        return Err(XBridgeError::SelectionMonitor {
            message: format!(
                "approved clipboard request returned property {:#x}, expected {approved_property:#x}",
                success_notify.property
            ),
        });
    }
    let observed = connection
        .get_property(
            false,
            requestor,
            approved_property,
            target,
            0,
            (MAX_CLIPBOARD_TEXT_HANDOFF_BYTES / 4) as u32,
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    if observed.format != 8 || observed.value != handoff.property.bytes {
        return Err(XBridgeError::SelectionMonitor {
            message: "approved clipboard property did not contain expected text bytes".to_owned(),
        });
    }

    Ok(LiveClipboardPortalSmokeReport {
        display_name: display_name.map(str::to_owned),
        owner: owner_window,
        requestor: requestor_window,
        selection,
        target,
        denied_property,
        approved_property,
        failure_property: failure_notify.property,
        success_property: success_notify.property,
        handoff_bytes: handoff.property.bytes.len(),
        observed_handoff_bytes: observed.value.len(),
    })
}

pub fn apply_clipboard_selection_handoff<C>(
    connection: &C,
    handoff: &ClipboardSelectionHandoff,
) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    connection
        .change_property8(
            PropMode::REPLACE,
            handoff.property.requestor,
            handoff.property.property,
            handoff.property.target,
            &handoff.property.bytes,
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    send_clipboard_selection_notify(connection, handoff.event)
}

#[derive(Clone, Copy, Debug)]
pub struct ClipboardSelectionFailure {
    pub transfer: PortalTransferId,
    pub event: SelectionNotifyEvent,
}

impl ClipboardSelectionFailure {
    pub fn failed_normally(&self) -> bool {
        self.event.property == u32::from(AtomEnum::NONE)
    }
}

pub fn clipboard_selection_failure_notify(
    request: ClipboardSelectionFailureRequest,
) -> ClipboardSelectionFailure {
    ClipboardSelectionFailure {
        transfer: request.transfer,
        event: SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: u32::from(AtomEnum::NONE),
        },
    }
}

pub fn apply_clipboard_selection_failure<C>(
    connection: &C,
    failure: &ClipboardSelectionFailure,
) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    send_clipboard_selection_notify(connection, failure.event)
}

fn send_clipboard_selection_notify<C>(
    connection: &C,
    event: SelectionNotifyEvent,
) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    connection
        .send_event(false, event.requestor, EventMask::NO_EVENT, event)
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
        })
}

pub fn clipboard_portal_owner_change_from_selection_update(
    update: &XSelectionOwnerUpdate,
) -> Option<ClipboardPortalOwnerChange> {
    if update.kind == XSelectionChangeKind::Unknown {
        return None;
    }

    let source_namespace = update
        .current
        .namespace
        .or_else(|| update.previous.and_then(|record| record.namespace))?;

    Some(ClipboardPortalOwnerChange {
        source_namespace,
        generation: update.current.generation,
    })
}

use crate::live::intern_atom;
use crate::prelude::*;
use crate::routed_input::drain_pending_events;
use crate::state::*;

use super::dispatch::dispatch_clipboard_selection_request_event;
use super::handoff::{
    MAX_CLIPBOARD_TEXT_HANDOFF_BYTES, apply_clipboard_selection_failure,
    apply_clipboard_selection_handoff, clipboard_selection_failure_notify,
    clipboard_selection_text_handoff_notify,
};
use super::live_support::{
    clipboard_smoke_mirror, create_clipboard_smoke_window, wait_for_selection_notify,
    wait_for_selection_request,
};

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

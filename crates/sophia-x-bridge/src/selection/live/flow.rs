use super::report::LiveClipboardPortalSmokeReport;
use super::setup::{LiveClipboardPortalSmokeSetup, create_live_clipboard_portal_smoke_setup};
use super::*;

pub fn smoke_live_clipboard_portal(
    display_name: Option<&str>,
) -> Result<LiveClipboardPortalSmokeReport, XBridgeError> {
    let setup = create_live_clipboard_portal_smoke_setup(display_name)?;
    let mut portal = ClipboardPortal::new();
    let failure_property = run_denied_clipboard_request(&setup, &mut portal)?;
    let (success_property, handoff_bytes, observed_handoff_bytes) =
        run_approved_clipboard_request(&setup, &mut portal)?;

    Ok(LiveClipboardPortalSmokeReport {
        display_name: display_name.map(str::to_owned),
        owner: setup.owner_window,
        requestor: setup.requestor_window,
        selection: setup.selection,
        target: setup.target,
        denied_property: setup.denied_property,
        approved_property: setup.approved_property,
        failure_property,
        success_property,
        handoff_bytes,
        observed_handoff_bytes,
    })
}

fn run_denied_clipboard_request(
    setup: &LiveClipboardPortalSmokeSetup,
    portal: &mut ClipboardPortal,
) -> Result<Atom, XBridgeError> {
    setup
        .connection
        .convert_selection(
            setup.requestor,
            setup.selection,
            setup.target,
            setup.denied_property,
            0u32,
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    setup
        .connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    let deny_request = wait_for_selection_request(
        &setup.connection,
        setup.requestor,
        setup.selection,
        setup.denied_property,
        Duration::from_secs(2),
    )?;
    let deny_dispatch = dispatch_clipboard_selection_request_event(
        &Event::SelectionRequest(deny_request),
        "UTF8_STRING",
        &setup.monitor,
        &setup.mirror,
        PortalTransferId::from_raw(1),
        portal,
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
    apply_clipboard_selection_failure(&setup.connection, &failure)?;
    let failure_notify = wait_for_selection_notify(
        &setup.connection,
        setup.requestor,
        setup.selection,
        setup.target,
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

    Ok(failure_notify.property)
}

fn run_approved_clipboard_request(
    setup: &LiveClipboardPortalSmokeSetup,
    portal: &mut ClipboardPortal,
) -> Result<(Atom, usize, usize), XBridgeError> {
    setup
        .connection
        .convert_selection(
            setup.requestor,
            setup.selection,
            setup.target,
            setup.approved_property,
            0u32,
        )
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    setup
        .connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;
    let approve_request = wait_for_selection_request(
        &setup.connection,
        setup.requestor,
        setup.selection,
        setup.approved_property,
        Duration::from_secs(2),
    )?;
    let approve_dispatch = dispatch_clipboard_selection_request_event(
        &Event::SelectionRequest(approve_request),
        "UTF8_STRING",
        &setup.monitor,
        &setup.mirror,
        PortalTransferId::from_raw(2),
        portal,
    )
    .map_err(|error| XBridgeError::SelectionMonitor {
        message: format!("failed to dispatch approved selection request: {error:?}"),
    })?;
    let command = portal
        .approve_generation(PortalTransferId::from_raw(2), setup.selection_generation)
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
    apply_clipboard_selection_handoff(&setup.connection, &handoff)?;
    let success_notify = wait_for_selection_notify(
        &setup.connection,
        setup.requestor,
        setup.selection,
        setup.target,
        Duration::from_secs(2),
    )?;
    if success_notify.property != setup.approved_property {
        return Err(XBridgeError::SelectionMonitor {
            message: format!(
                "approved clipboard request returned property {:#x}, expected {:#x}",
                success_notify.property, setup.approved_property
            ),
        });
    }
    let observed = setup
        .connection
        .get_property(
            false,
            setup.requestor,
            setup.approved_property,
            setup.target,
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

    Ok((
        success_notify.property,
        handoff.property.bytes.len(),
        observed.value.len(),
    ))
}

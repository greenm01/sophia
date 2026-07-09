use super::prelude::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "portal-clipboard-deny-smoke") {
        let transfer = PortalTransferId::from_raw(1);
        let source_namespace = NamespaceId::from_raw(10);
        let target_namespace = NamespaceId::from_raw(20);
        let generation = 7;
        let mut portal = ClipboardPortal::new();
        portal
            .request_import(ClipboardTransferRequest {
                transfer,
                source_namespace,
                target_namespace,
                target: ClipboardTarget::Atom("UTF8_STRING".to_owned()),
                byte_size: 128,
                generation,
            })
            .map_err(|error| format!("clipboard portal import failed: {error:?}"))?;
        let command = portal
            .deny(transfer)
            .map_err(|error| format!("clipboard portal denial failed: {error:?}"))?;
        let PortalCommand::FailSelection { transfer } = command else {
            return Err(format!("expected FailSelection, got {command:?}").into());
        };
        let failure = clipboard_selection_failure_notify(ClipboardSelectionFailureRequest {
            transfer,
            requestor: 0x44,
            selection: 0x100,
            target: 0x200,
            time: 55,
        });

        if !failure.failed_normally() {
            return Err("clipboard denial did not map to SelectionNotify property=None".into());
        }

        println!(
            "portal-clipboard-deny-smoke transfer={} source_ns={} target_ns={} generation={} command=FailSelection selection_notify_property={} normal_failure={}",
            transfer.raw(),
            source_namespace.raw(),
            target_namespace.raw(),
            generation,
            failure.event.property,
            failure.failed_normally(),
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "portal-clipboard-request-smoke")
    {
        let transfer = PortalTransferId::from_raw(2);
        let source_namespace = NamespaceId::from_raw(10);
        let target_namespace = NamespaceId::from_raw(20);
        let owner = XWindowId::new(0x40, 1);
        let requestor = XWindowId::new(0x44, 1);
        let mut mirror = XMirrorState::default();
        mirror.ingest_window(clipboard_mirror(owner, source_namespace));
        mirror.ingest_window(clipboard_mirror(requestor, target_namespace));

        let mut monitor = XSelectionMonitor::new();
        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(owner),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &mirror,
        );
        let request = Event::SelectionRequest(SelectionRequestEvent {
            response_type: 0,
            sequence: 1,
            time: 55,
            owner: owner.xid(),
            requestor: requestor.xid(),
            selection: 0x100,
            target: 0x200,
            property: 0x300,
        });
        let mut portal = ClipboardPortal::new();
        let dispatch = dispatch_clipboard_selection_request_event(
            &request,
            "UTF8_STRING",
            &monitor,
            &mirror,
            transfer,
            &mut portal,
        )
        .map_err(|error| format!("selection request dispatch failed: {error:?}"))?;
        let PortalCommand::FailSelection { transfer } = portal
            .deny(transfer)
            .map_err(|error| format!("clipboard portal denial failed: {error:?}"))?
        else {
            return Err("expected clipboard denial to fail selection".into());
        };
        let failure = clipboard_selection_failure_notify(dispatch.portal_request.failure);

        println!(
            "portal-clipboard-request-smoke transfer={} source_ns={} target_ns={} owner_generation={} requestor={:#x} selection={:#x} target={:#x} property={:#x} failure_property={} normal_failure={}",
            transfer.raw(),
            dispatch.portal_request.request.source_namespace.raw(),
            dispatch.portal_request.request.target_namespace.raw(),
            update.current.generation,
            failure.event.requestor,
            failure.event.selection,
            failure.event.target,
            dispatch.portal_request.property,
            failure.event.property,
            failure.failed_normally(),
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "portal-clipboard-handoff-smoke")
    {
        let transfer = PortalTransferId::from_raw(3);
        let source_namespace = NamespaceId::from_raw(10);
        let target_namespace = NamespaceId::from_raw(20);
        let owner = XWindowId::new(0x40, 1);
        let requestor = XWindowId::new(0x44, 1);
        let mut mirror = XMirrorState::default();
        mirror.ingest_window(clipboard_mirror(owner, source_namespace));
        mirror.ingest_window(clipboard_mirror(requestor, target_namespace));

        let mut monitor = XSelectionMonitor::new();
        let update = monitor.apply_event(
            XSelectionEvent {
                selection: 0x100,
                owner: Some(owner),
                timestamp: 11,
                selection_timestamp: 10,
                kind: XSelectionChangeKind::SetOwner,
            },
            &mirror,
        );
        let request = Event::SelectionRequest(SelectionRequestEvent {
            response_type: 0,
            sequence: 1,
            time: 55,
            owner: owner.xid(),
            requestor: requestor.xid(),
            selection: 0x100,
            target: 0x200,
            property: 0x300,
        });
        let mut portal = ClipboardPortal::new();
        let dispatch = dispatch_clipboard_selection_request_event(
            &request,
            "UTF8_STRING",
            &monitor,
            &mirror,
            transfer,
            &mut portal,
        )
        .map_err(|error| format!("selection request dispatch failed: {error:?}"))?;
        let command = portal
            .approve_generation(transfer, update.current.generation)
            .map_err(|error| format!("clipboard portal approval failed: {error:?}"))?;
        let handoff =
            clipboard_selection_text_handoff_notify(&command, &dispatch.portal_request, "hello")
                .map_err(|error| format!("clipboard handoff failed: {error:?}"))?;

        println!(
            "portal-clipboard-handoff-smoke transfer={} source_ns={} target_ns={} owner_generation={} requestor={:#x} selection={:#x} target={:#x} property={:#x} bytes={} success_property={} normal_success={}",
            transfer.raw(),
            dispatch.portal_request.request.source_namespace.raw(),
            dispatch.portal_request.request.target_namespace.raw(),
            update.current.generation,
            handoff.event.requestor,
            handoff.event.selection,
            handoff.event.target,
            handoff.property.property,
            handoff.property.bytes.len(),
            handoff.event.property,
            handoff.succeeded_normally(),
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "x-smoke-live-clipboard-portal")
    {
        let display = arg_value(args, "--display");
        let report = smoke_live_clipboard_portal(display.as_deref())?;

        println!(
            "x-smoke-live-clipboard-portal display={} owner={:#x} requestor={:#x} selection={:#x} target={:#x} denied_property={:#x} approved_property={:#x} failure_property={:#x} success_property={:#x} handoff_bytes={} observed_handoff_bytes={}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.owner.xid(),
            report.requestor.xid(),
            report.selection,
            report.target,
            report.denied_property,
            report.approved_property,
            report.failure_property,
            report.success_property,
            report.handoff_bytes,
            report.observed_handoff_bytes,
        );
        return Ok(true);
    }

    Ok(false)
}

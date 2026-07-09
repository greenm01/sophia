use crate::prelude::*;
use crate::state::*;

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

use crate::prelude::*;
use crate::state::*;

pub(super) fn create_clipboard_smoke_window<C>(
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

pub(super) fn wait_for_selection_request<C>(
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

pub(super) fn wait_for_selection_notify<C>(
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

pub(super) fn clipboard_smoke_mirror(window: XWindowId, namespace: NamespaceId) -> XWindowMirror {
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

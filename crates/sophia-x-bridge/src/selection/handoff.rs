use crate::prelude::*;
use crate::state::*;

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

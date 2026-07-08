//! Cross-namespace portal policy reducers.
//!
//! Portal code is intentionally off the compositor hot path. It turns
//! namespaced transfer requests into bounded commands that the runtime or
//! X bridge can execute without granting the policy code raw X authority.

use std::collections::BTreeMap;

use sophia_protocol::{
    NamespaceId, PortalDecision, PortalTransfer, PortalTransferId, PortalTransferKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardTransferRequest {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub target: ClipboardTarget,
    pub byte_size: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipboardSourceOwnerChanged {
    pub source_namespace: NamespaceId,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragAndDropTransferRequest {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub offered_types: Vec<String>,
    pub byte_size: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileHandoffRequest {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub mode: FileHandoffMode,
    pub offered_types: Vec<String>,
    pub suggested_name: Option<String>,
    pub byte_size: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenCaptureRequest {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub mode: ScreenCaptureMode,
    pub scope: ScreenCaptureScope,
    pub mime_type: String,
    pub byte_size: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UriOpenRequest {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub uri: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationRequest {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub summary: String,
    pub body: Option<String>,
    pub urgency: NotificationUrgency,
    pub actions: Vec<String>,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileHandoffMode {
    Open,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenCaptureMode {
    Screenshot,
    ScreenRecording,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenCaptureScope {
    Desktop,
    Output,
    Surface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardTarget {
    Atom(String),
    Mime(String),
}

impl ClipboardTarget {
    pub fn as_str(&self) -> &str {
        match self {
            ClipboardTarget::Atom(target) | ClipboardTarget::Mime(target) => target,
        }
    }

    pub fn is_text(&self) -> bool {
        let target = self.as_str();
        let lower = target.to_ascii_lowercase();

        matches!(target, "UTF8_STRING" | "TEXT" | "STRING")
            || lower == "text/plain"
            || lower.starts_with("text/plain;")
            || lower.starts_with("text/")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortalCommand {
    PromptClipboardTransfer(PortalTransfer),
    HandoffClipboard { transfer: PortalTransferId },
    FailSelection { transfer: PortalTransferId },
    PromptDragAndDropTransfer(PortalTransfer),
    HandoffDragAndDrop { transfer: PortalTransferId },
    CancelDragAndDrop { transfer: PortalTransferId },
    PromptFileHandoff(PortalTransfer),
    HandoffFile { transfer: PortalTransferId },
    CancelFileHandoff { transfer: PortalTransferId },
    PromptScreenCapture(PortalTransfer),
    HandoffScreenCapture { transfer: PortalTransferId },
    CancelScreenCapture { transfer: PortalTransferId },
    PromptUriOpen(PortalTransfer),
    HandoffUriOpen { transfer: PortalTransferId },
    CancelUriOpen { transfer: PortalTransferId },
    PromptNotification(PortalTransfer),
    DeliverNotification { transfer: PortalTransferId },
    DropNotification { transfer: PortalTransferId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortalError {
    InvalidTransfer,
    InvalidNamespace,
    UnsupportedTarget,
    MissingTransferType,
    TooManyTransferTypes,
    InvalidSuggestedName,
    UnsupportedCaptureMimeType,
    InvalidUri,
    UnsupportedUriScheme,
    InvalidNotificationText,
    TooManyNotificationActions,
    UnknownTransfer,
    NotPending,
}

pub const MAX_DRAG_AND_DROP_TYPES: usize = 16;
pub const MAX_FILE_HANDOFF_TYPES: usize = 32;
pub const MAX_SUGGESTED_FILE_NAME_LEN: usize = 255;
pub const MAX_URI_LEN: usize = 2048;
pub const MAX_NOTIFICATION_SUMMARY_LEN: usize = 128;
pub const MAX_NOTIFICATION_BODY_LEN: usize = 1024;
pub const MAX_NOTIFICATION_ACTIONS: usize = 4;
pub const MAX_NOTIFICATION_ACTION_LEN: usize = 64;

#[derive(Debug, Default)]
pub struct ClipboardPortal {
    transfers: BTreeMap<PortalTransferId, PortalTransfer>,
}

impl ClipboardPortal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_import(
        &mut self,
        request: ClipboardTransferRequest,
    ) -> Result<PortalCommand, PortalError> {
        validate_request(&request)?;

        let transfer = PortalTransfer {
            transfer: request.transfer,
            source_namespace: request.source_namespace,
            target_namespace: request.target_namespace,
            kind: PortalTransferKind::Clipboard,
            mime_type: Some(request.target.as_str().to_owned()),
            byte_size: request.byte_size,
            decision: PortalDecision::Pending,
            generation: request.generation,
        };

        self.transfers.insert(transfer.transfer, transfer.clone());
        Ok(PortalCommand::PromptClipboardTransfer(transfer))
    }

    pub fn deny(&mut self, transfer: PortalTransferId) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;
        transfer_state.decision = PortalDecision::Denied;

        Ok(PortalCommand::FailSelection { transfer })
    }

    pub fn approve_generation(
        &mut self,
        transfer: PortalTransferId,
        generation: u64,
    ) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;

        if transfer_state.generation != generation {
            transfer_state.decision = PortalDecision::Revoked;
            return Ok(PortalCommand::FailSelection { transfer });
        }

        transfer_state.decision = PortalDecision::Allowed;
        Ok(PortalCommand::HandoffClipboard { transfer })
    }

    pub fn source_owner_changed(
        &mut self,
        source_namespace: NamespaceId,
        generation: u64,
    ) -> Vec<PortalCommand> {
        let mut commands = Vec::new();

        for transfer in self.transfers.values_mut() {
            if transfer.source_namespace == source_namespace
                && transfer.decision == PortalDecision::Pending
                && transfer.generation != generation
            {
                transfer.decision = PortalDecision::Revoked;
                commands.push(PortalCommand::FailSelection {
                    transfer: transfer.transfer,
                });
            }
        }

        commands
    }

    pub fn apply_owner_changed(
        &mut self,
        event: ClipboardSourceOwnerChanged,
    ) -> Vec<PortalCommand> {
        self.source_owner_changed(event.source_namespace, event.generation)
    }

    pub fn transfer(&self, transfer: PortalTransferId) -> Option<&PortalTransfer> {
        self.transfers.get(&transfer)
    }

    fn pending_transfer_mut(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<&mut PortalTransfer, PortalError> {
        let transfer_state = self
            .transfers
            .get_mut(&transfer)
            .ok_or(PortalError::UnknownTransfer)?;

        if transfer_state.decision != PortalDecision::Pending {
            return Err(PortalError::NotPending);
        }

        Ok(transfer_state)
    }
}

#[derive(Debug, Default)]
pub struct DragAndDropPortal {
    transfers: BTreeMap<PortalTransferId, PortalTransfer>,
}

impl DragAndDropPortal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_handoff(
        &mut self,
        request: DragAndDropTransferRequest,
    ) -> Result<PortalCommand, PortalError> {
        validate_drag_and_drop_request(&request)?;

        let transfer = PortalTransfer {
            transfer: request.transfer,
            source_namespace: request.source_namespace,
            target_namespace: request.target_namespace,
            kind: PortalTransferKind::DragAndDrop,
            mime_type: request.offered_types.first().cloned(),
            byte_size: request.byte_size,
            decision: PortalDecision::Pending,
            generation: request.generation,
        };

        self.transfers.insert(transfer.transfer, transfer.clone());
        Ok(PortalCommand::PromptDragAndDropTransfer(transfer))
    }

    pub fn deny(&mut self, transfer: PortalTransferId) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;
        transfer_state.decision = PortalDecision::Denied;

        Ok(PortalCommand::CancelDragAndDrop { transfer })
    }

    pub fn approve_generation(
        &mut self,
        transfer: PortalTransferId,
        generation: u64,
    ) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;

        if transfer_state.generation != generation {
            transfer_state.decision = PortalDecision::Revoked;
            return Ok(PortalCommand::CancelDragAndDrop { transfer });
        }

        transfer_state.decision = PortalDecision::Allowed;
        Ok(PortalCommand::HandoffDragAndDrop { transfer })
    }

    pub fn source_owner_changed(
        &mut self,
        source_namespace: NamespaceId,
        generation: u64,
    ) -> Vec<PortalCommand> {
        let mut commands = Vec::new();

        for transfer in self.transfers.values_mut() {
            if transfer.source_namespace == source_namespace
                && transfer.decision == PortalDecision::Pending
                && transfer.generation != generation
            {
                transfer.decision = PortalDecision::Revoked;
                commands.push(PortalCommand::CancelDragAndDrop {
                    transfer: transfer.transfer,
                });
            }
        }

        commands
    }

    pub fn transfer(&self, transfer: PortalTransferId) -> Option<&PortalTransfer> {
        self.transfers.get(&transfer)
    }

    fn pending_transfer_mut(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<&mut PortalTransfer, PortalError> {
        let transfer_state = self
            .transfers
            .get_mut(&transfer)
            .ok_or(PortalError::UnknownTransfer)?;

        if transfer_state.decision != PortalDecision::Pending {
            return Err(PortalError::NotPending);
        }

        Ok(transfer_state)
    }
}

#[derive(Debug, Default)]
pub struct FileHandoffPortal {
    transfers: BTreeMap<PortalTransferId, PortalTransfer>,
}

impl FileHandoffPortal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_handoff(
        &mut self,
        request: FileHandoffRequest,
    ) -> Result<PortalCommand, PortalError> {
        validate_file_handoff_request(&request)?;

        let transfer = PortalTransfer {
            transfer: request.transfer,
            source_namespace: request.source_namespace,
            target_namespace: request.target_namespace,
            kind: PortalTransferKind::FileHandoff,
            mime_type: Some(file_handoff_type_hint(&request)),
            byte_size: request.byte_size,
            decision: PortalDecision::Pending,
            generation: request.generation,
        };

        self.transfers.insert(transfer.transfer, transfer.clone());
        Ok(PortalCommand::PromptFileHandoff(transfer))
    }

    pub fn deny(&mut self, transfer: PortalTransferId) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;
        transfer_state.decision = PortalDecision::Denied;

        Ok(PortalCommand::CancelFileHandoff { transfer })
    }

    pub fn approve_generation(
        &mut self,
        transfer: PortalTransferId,
        generation: u64,
    ) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;

        if transfer_state.generation != generation {
            transfer_state.decision = PortalDecision::Revoked;
            return Ok(PortalCommand::CancelFileHandoff { transfer });
        }

        transfer_state.decision = PortalDecision::Allowed;
        Ok(PortalCommand::HandoffFile { transfer })
    }

    pub fn source_owner_changed(
        &mut self,
        source_namespace: NamespaceId,
        generation: u64,
    ) -> Vec<PortalCommand> {
        let mut commands = Vec::new();

        for transfer in self.transfers.values_mut() {
            if transfer.source_namespace == source_namespace
                && transfer.decision == PortalDecision::Pending
                && transfer.generation != generation
            {
                transfer.decision = PortalDecision::Revoked;
                commands.push(PortalCommand::CancelFileHandoff {
                    transfer: transfer.transfer,
                });
            }
        }

        commands
    }

    pub fn transfer(&self, transfer: PortalTransferId) -> Option<&PortalTransfer> {
        self.transfers.get(&transfer)
    }

    fn pending_transfer_mut(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<&mut PortalTransfer, PortalError> {
        let transfer_state = self
            .transfers
            .get_mut(&transfer)
            .ok_or(PortalError::UnknownTransfer)?;

        if transfer_state.decision != PortalDecision::Pending {
            return Err(PortalError::NotPending);
        }

        Ok(transfer_state)
    }
}

#[derive(Debug, Default)]
pub struct ScreenCapturePortal {
    transfers: BTreeMap<PortalTransferId, PortalTransfer>,
}

impl ScreenCapturePortal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_capture(
        &mut self,
        request: ScreenCaptureRequest,
    ) -> Result<PortalCommand, PortalError> {
        validate_screen_capture_request(&request)?;

        let transfer = PortalTransfer {
            transfer: request.transfer,
            source_namespace: request.source_namespace,
            target_namespace: request.target_namespace,
            kind: PortalTransferKind::Screenshot,
            mime_type: Some(screen_capture_type_hint(&request)),
            byte_size: request.byte_size,
            decision: PortalDecision::Pending,
            generation: request.generation,
        };

        self.transfers.insert(transfer.transfer, transfer.clone());
        Ok(PortalCommand::PromptScreenCapture(transfer))
    }

    pub fn deny(&mut self, transfer: PortalTransferId) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;
        transfer_state.decision = PortalDecision::Denied;

        Ok(PortalCommand::CancelScreenCapture { transfer })
    }

    pub fn approve_generation(
        &mut self,
        transfer: PortalTransferId,
        generation: u64,
    ) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;

        if transfer_state.generation != generation {
            transfer_state.decision = PortalDecision::Revoked;
            return Ok(PortalCommand::CancelScreenCapture { transfer });
        }

        transfer_state.decision = PortalDecision::Allowed;
        Ok(PortalCommand::HandoffScreenCapture { transfer })
    }

    pub fn source_owner_changed(
        &mut self,
        source_namespace: NamespaceId,
        generation: u64,
    ) -> Vec<PortalCommand> {
        let mut commands = Vec::new();

        for transfer in self.transfers.values_mut() {
            if transfer.source_namespace == source_namespace
                && transfer.decision == PortalDecision::Pending
                && transfer.generation != generation
            {
                transfer.decision = PortalDecision::Revoked;
                commands.push(PortalCommand::CancelScreenCapture {
                    transfer: transfer.transfer,
                });
            }
        }

        commands
    }

    pub fn transfer(&self, transfer: PortalTransferId) -> Option<&PortalTransfer> {
        self.transfers.get(&transfer)
    }

    fn pending_transfer_mut(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<&mut PortalTransfer, PortalError> {
        let transfer_state = self
            .transfers
            .get_mut(&transfer)
            .ok_or(PortalError::UnknownTransfer)?;

        if transfer_state.decision != PortalDecision::Pending {
            return Err(PortalError::NotPending);
        }

        Ok(transfer_state)
    }
}

#[derive(Debug, Default)]
pub struct UriOpenPortal {
    transfers: BTreeMap<PortalTransferId, PortalTransfer>,
}

impl UriOpenPortal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_open(&mut self, request: UriOpenRequest) -> Result<PortalCommand, PortalError> {
        validate_uri_open_request(&request)?;

        let transfer = PortalTransfer {
            transfer: request.transfer,
            source_namespace: request.source_namespace,
            target_namespace: request.target_namespace,
            kind: PortalTransferKind::Notification,
            mime_type: Some(uri_open_type_hint(&request.uri)),
            byte_size: request.uri.len() as u64,
            decision: PortalDecision::Pending,
            generation: request.generation,
        };

        self.transfers.insert(transfer.transfer, transfer.clone());
        Ok(PortalCommand::PromptUriOpen(transfer))
    }

    pub fn deny(&mut self, transfer: PortalTransferId) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;
        transfer_state.decision = PortalDecision::Denied;

        Ok(PortalCommand::CancelUriOpen { transfer })
    }

    pub fn approve_generation(
        &mut self,
        transfer: PortalTransferId,
        generation: u64,
    ) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;

        if transfer_state.generation != generation {
            transfer_state.decision = PortalDecision::Revoked;
            return Ok(PortalCommand::CancelUriOpen { transfer });
        }

        transfer_state.decision = PortalDecision::Allowed;
        Ok(PortalCommand::HandoffUriOpen { transfer })
    }

    pub fn source_owner_changed(
        &mut self,
        source_namespace: NamespaceId,
        generation: u64,
    ) -> Vec<PortalCommand> {
        let mut commands = Vec::new();

        for transfer in self.transfers.values_mut() {
            if transfer.source_namespace == source_namespace
                && transfer.decision == PortalDecision::Pending
                && transfer.generation != generation
            {
                transfer.decision = PortalDecision::Revoked;
                commands.push(PortalCommand::CancelUriOpen {
                    transfer: transfer.transfer,
                });
            }
        }

        commands
    }

    pub fn transfer(&self, transfer: PortalTransferId) -> Option<&PortalTransfer> {
        self.transfers.get(&transfer)
    }

    fn pending_transfer_mut(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<&mut PortalTransfer, PortalError> {
        let transfer_state = self
            .transfers
            .get_mut(&transfer)
            .ok_or(PortalError::UnknownTransfer)?;

        if transfer_state.decision != PortalDecision::Pending {
            return Err(PortalError::NotPending);
        }

        Ok(transfer_state)
    }
}

#[derive(Debug, Default)]
pub struct NotificationPortal {
    transfers: BTreeMap<PortalTransferId, PortalTransfer>,
}

impl NotificationPortal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_display(
        &mut self,
        request: NotificationRequest,
    ) -> Result<PortalCommand, PortalError> {
        validate_notification_request(&request)?;

        let transfer = PortalTransfer {
            transfer: request.transfer,
            source_namespace: request.source_namespace,
            target_namespace: request.target_namespace,
            kind: PortalTransferKind::Notification,
            mime_type: Some(notification_type_hint(request.urgency)),
            byte_size: notification_byte_size(&request),
            decision: PortalDecision::Pending,
            generation: request.generation,
        };

        self.transfers.insert(transfer.transfer, transfer.clone());
        Ok(PortalCommand::PromptNotification(transfer))
    }

    pub fn deny(&mut self, transfer: PortalTransferId) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;
        transfer_state.decision = PortalDecision::Denied;

        Ok(PortalCommand::DropNotification { transfer })
    }

    pub fn approve_generation(
        &mut self,
        transfer: PortalTransferId,
        generation: u64,
    ) -> Result<PortalCommand, PortalError> {
        let transfer_state = self.pending_transfer_mut(transfer)?;

        if transfer_state.generation != generation {
            transfer_state.decision = PortalDecision::Revoked;
            return Ok(PortalCommand::DropNotification { transfer });
        }

        transfer_state.decision = PortalDecision::Allowed;
        Ok(PortalCommand::DeliverNotification { transfer })
    }

    pub fn source_owner_changed(
        &mut self,
        source_namespace: NamespaceId,
        generation: u64,
    ) -> Vec<PortalCommand> {
        let mut commands = Vec::new();

        for transfer in self.transfers.values_mut() {
            if transfer.source_namespace == source_namespace
                && transfer.decision == PortalDecision::Pending
                && transfer.generation != generation
            {
                transfer.decision = PortalDecision::Revoked;
                commands.push(PortalCommand::DropNotification {
                    transfer: transfer.transfer,
                });
            }
        }

        commands
    }

    pub fn transfer(&self, transfer: PortalTransferId) -> Option<&PortalTransfer> {
        self.transfers.get(&transfer)
    }

    fn pending_transfer_mut(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<&mut PortalTransfer, PortalError> {
        let transfer_state = self
            .transfers
            .get_mut(&transfer)
            .ok_or(PortalError::UnknownTransfer)?;

        if transfer_state.decision != PortalDecision::Pending {
            return Err(PortalError::NotPending);
        }

        Ok(transfer_state)
    }
}

fn validate_request(request: &ClipboardTransferRequest) -> Result<(), PortalError> {
    if !request.transfer.is_valid() {
        return Err(PortalError::InvalidTransfer);
    }

    if !request.source_namespace.is_valid() || !request.target_namespace.is_valid() {
        return Err(PortalError::InvalidNamespace);
    }

    if !request.target.is_text() {
        return Err(PortalError::UnsupportedTarget);
    }

    Ok(())
}

fn validate_drag_and_drop_request(request: &DragAndDropTransferRequest) -> Result<(), PortalError> {
    if !request.transfer.is_valid() {
        return Err(PortalError::InvalidTransfer);
    }

    if !request.source_namespace.is_valid() || !request.target_namespace.is_valid() {
        return Err(PortalError::InvalidNamespace);
    }

    if request.offered_types.is_empty() || request.offered_types.iter().any(|kind| kind.is_empty())
    {
        return Err(PortalError::MissingTransferType);
    }

    if request.offered_types.len() > MAX_DRAG_AND_DROP_TYPES {
        return Err(PortalError::TooManyTransferTypes);
    }

    Ok(())
}

fn validate_file_handoff_request(request: &FileHandoffRequest) -> Result<(), PortalError> {
    if !request.transfer.is_valid() {
        return Err(PortalError::InvalidTransfer);
    }

    if !request.source_namespace.is_valid() || !request.target_namespace.is_valid() {
        return Err(PortalError::InvalidNamespace);
    }

    if request.offered_types.is_empty() || request.offered_types.iter().any(|kind| kind.is_empty())
    {
        return Err(PortalError::MissingTransferType);
    }

    if request.offered_types.len() > MAX_FILE_HANDOFF_TYPES {
        return Err(PortalError::TooManyTransferTypes);
    }

    if let Some(name) = &request.suggested_name
        && !valid_suggested_file_name(name)
    {
        return Err(PortalError::InvalidSuggestedName);
    }

    Ok(())
}

fn valid_suggested_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SUGGESTED_FILE_NAME_LEN
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
}

fn file_handoff_type_hint(request: &FileHandoffRequest) -> String {
    let mode = match request.mode {
        FileHandoffMode::Open => "open",
        FileHandoffMode::Save => "save",
    };

    format!("{mode}:{}", request.offered_types[0])
}

fn validate_screen_capture_request(request: &ScreenCaptureRequest) -> Result<(), PortalError> {
    if !request.transfer.is_valid() {
        return Err(PortalError::InvalidTransfer);
    }

    if !request.source_namespace.is_valid() || !request.target_namespace.is_valid() {
        return Err(PortalError::InvalidNamespace);
    }

    if !supported_screen_capture_mime(request.mode, &request.mime_type) {
        return Err(PortalError::UnsupportedCaptureMimeType);
    }

    Ok(())
}

fn supported_screen_capture_mime(mode: ScreenCaptureMode, mime_type: &str) -> bool {
    match mode {
        ScreenCaptureMode::Screenshot => matches!(mime_type, "image/png" | "image/jpeg"),
        ScreenCaptureMode::ScreenRecording => matches!(mime_type, "video/webm" | "video/mp4"),
    }
}

fn screen_capture_type_hint(request: &ScreenCaptureRequest) -> String {
    let mode = match request.mode {
        ScreenCaptureMode::Screenshot => "screenshot",
        ScreenCaptureMode::ScreenRecording => "screen-recording",
    };
    let scope = match request.scope {
        ScreenCaptureScope::Desktop => "desktop",
        ScreenCaptureScope::Output => "output",
        ScreenCaptureScope::Surface => "surface",
    };

    format!("{mode}:{scope}:{}", request.mime_type)
}

fn validate_uri_open_request(request: &UriOpenRequest) -> Result<(), PortalError> {
    if !request.transfer.is_valid() {
        return Err(PortalError::InvalidTransfer);
    }

    if !request.source_namespace.is_valid() || !request.target_namespace.is_valid() {
        return Err(PortalError::InvalidNamespace);
    }

    if request.uri.is_empty()
        || request.uri.len() > MAX_URI_LEN
        || request
            .uri
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(PortalError::InvalidUri);
    }

    let Some(scheme) = uri_scheme(&request.uri) else {
        return Err(PortalError::InvalidUri);
    };

    if !supported_uri_scheme(scheme) {
        return Err(PortalError::UnsupportedUriScheme);
    }

    Ok(())
}

fn uri_scheme(uri: &str) -> Option<&str> {
    let (scheme, _rest) = uri.split_once(':')?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
    {
        return None;
    }

    Some(scheme)
}

fn supported_uri_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https" | "mailto" | "tel")
}

fn uri_open_type_hint(uri: &str) -> String {
    let scheme = uri_scheme(uri).unwrap_or("unknown");
    format!("uri-open:{scheme}")
}

fn validate_notification_request(request: &NotificationRequest) -> Result<(), PortalError> {
    if !request.transfer.is_valid() {
        return Err(PortalError::InvalidTransfer);
    }

    if !request.source_namespace.is_valid() || !request.target_namespace.is_valid() {
        return Err(PortalError::InvalidNamespace);
    }

    if !valid_notification_text(&request.summary, MAX_NOTIFICATION_SUMMARY_LEN)
        || request
            .body
            .as_ref()
            .is_some_and(|body| !valid_notification_text(body, MAX_NOTIFICATION_BODY_LEN))
        || request
            .actions
            .iter()
            .any(|action| !valid_notification_text(action, MAX_NOTIFICATION_ACTION_LEN))
    {
        return Err(PortalError::InvalidNotificationText);
    }

    if request.actions.len() > MAX_NOTIFICATION_ACTIONS {
        return Err(PortalError::TooManyNotificationActions);
    }

    Ok(())
}

fn valid_notification_text(text: &str, max_len: usize) -> bool {
    !text.is_empty() && text.len() <= max_len && !text.chars().any(char::is_control)
}

fn notification_byte_size(request: &NotificationRequest) -> u64 {
    let body_len = request.body.as_ref().map_or(0, String::len);
    let actions_len: usize = request.actions.iter().map(String::len).sum();

    (request.summary.len() + body_len + actions_len) as u64
}

fn notification_type_hint(urgency: NotificationUrgency) -> String {
    let urgency = match urgency {
        NotificationUrgency::Low => "low",
        NotificationUrgency::Normal => "normal",
        NotificationUrgency::Critical => "critical",
    };

    format!("notification:{urgency}")
}

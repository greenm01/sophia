use crate::prelude::*;
use crate::types::*;

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

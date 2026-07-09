use crate::prelude::*;
use crate::types::*;

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

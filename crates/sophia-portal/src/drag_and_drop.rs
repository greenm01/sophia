use crate::prelude::*;
use crate::types::*;

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

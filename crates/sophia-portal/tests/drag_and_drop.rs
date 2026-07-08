use sophia_portal::{
    DragAndDropPortal, DragAndDropTransferRequest, MAX_DRAG_AND_DROP_TYPES, PortalCommand,
    PortalError,
};
use sophia_protocol::{NamespaceId, PortalDecision, PortalTransferId, PortalTransferKind};

fn request(transfer: u64, generation: u64) -> DragAndDropTransferRequest {
    DragAndDropTransferRequest {
        transfer: PortalTransferId::from_raw(transfer),
        source_namespace: NamespaceId::from_raw(10),
        target_namespace: NamespaceId::from_raw(20),
        offered_types: vec!["text/uri-list".to_owned(), "text/plain".to_owned()],
        byte_size: 512,
        generation,
    }
}

#[test]
fn drag_and_drop_handoff_is_pending_by_default() {
    let mut portal = DragAndDropPortal::new();

    let command = portal.request_handoff(request(1, 7)).unwrap();

    match command {
        PortalCommand::PromptDragAndDropTransfer(transfer) => {
            assert_eq!(transfer.transfer, PortalTransferId::from_raw(1));
            assert_eq!(transfer.kind, PortalTransferKind::DragAndDrop);
            assert_eq!(transfer.mime_type, Some("text/uri-list".to_owned()));
            assert_eq!(transfer.decision, PortalDecision::Pending);
            assert_eq!(transfer.generation, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn drag_and_drop_requires_at_least_one_offered_type() {
    let mut portal = DragAndDropPortal::new();
    let mut request = request(1, 7);
    request.offered_types.clear();

    assert_eq!(
        portal.request_handoff(request),
        Err(PortalError::MissingTransferType)
    );
}

#[test]
fn drag_and_drop_rejects_excessive_offered_types() {
    let mut portal = DragAndDropPortal::new();
    let mut request = request(1, 7);
    request.offered_types = (0..=MAX_DRAG_AND_DROP_TYPES)
        .map(|index| format!("application/x-sophia-{index}"))
        .collect();

    assert_eq!(
        portal.request_handoff(request),
        Err(PortalError::TooManyTransferTypes)
    );
}

#[test]
fn denied_drag_and_drop_cancels_handoff() {
    let mut portal = DragAndDropPortal::new();
    portal.request_handoff(request(1, 7)).unwrap();

    assert_eq!(
        portal.deny(PortalTransferId::from_raw(1)),
        Ok(PortalCommand::CancelDragAndDrop {
            transfer: PortalTransferId::from_raw(1)
        })
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Denied
    );
}

#[test]
fn drag_and_drop_approval_requires_matching_generation() {
    let mut portal = DragAndDropPortal::new();
    portal.request_handoff(request(1, 7)).unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 7),
        Ok(PortalCommand::HandoffDragAndDrop {
            transfer: PortalTransferId::from_raw(1)
        })
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Allowed
    );
}

#[test]
fn stale_drag_and_drop_generation_revokes_handoff() {
    let mut portal = DragAndDropPortal::new();
    portal.request_handoff(request(1, 7)).unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 8),
        Ok(PortalCommand::CancelDragAndDrop {
            transfer: PortalTransferId::from_raw(1)
        })
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Revoked
    );
}

#[test]
fn source_owner_change_revokes_pending_drag_and_drop() {
    let mut portal = DragAndDropPortal::new();
    portal.request_handoff(request(1, 7)).unwrap();
    portal.request_handoff(request(2, 9)).unwrap();

    let commands = portal.source_owner_changed(NamespaceId::from_raw(10), 9);

    assert_eq!(
        commands,
        vec![PortalCommand::CancelDragAndDrop {
            transfer: PortalTransferId::from_raw(1)
        }]
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(1))
            .unwrap()
            .decision,
        PortalDecision::Revoked
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(2))
            .unwrap()
            .decision,
        PortalDecision::Pending
    );
}

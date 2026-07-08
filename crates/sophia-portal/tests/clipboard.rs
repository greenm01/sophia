use sophia_portal::{
    ClipboardPortal, ClipboardSourceOwnerChanged, ClipboardTarget, ClipboardTransferRequest,
    PortalCommand, PortalError,
};
use sophia_protocol::{NamespaceId, PortalDecision, PortalTransferId};

fn request(transfer: u64, generation: u64) -> ClipboardTransferRequest {
    ClipboardTransferRequest {
        transfer: PortalTransferId::from_raw(transfer),
        source_namespace: NamespaceId::from_raw(10),
        target_namespace: NamespaceId::from_raw(20),
        target: ClipboardTarget::Atom("UTF8_STRING".to_owned()),
        byte_size: 128,
        generation,
    }
}

#[test]
fn clipboard_imports_are_pending_by_default() {
    let mut portal = ClipboardPortal::new();

    let command = portal.request_import(request(1, 7)).unwrap();

    match command {
        PortalCommand::PromptClipboardTransfer(transfer) => {
            assert_eq!(transfer.transfer, PortalTransferId::from_raw(1));
            assert_eq!(transfer.decision, PortalDecision::Pending);
            assert_eq!(transfer.generation, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn non_text_clipboard_targets_are_rejected() {
    let mut portal = ClipboardPortal::new();
    let mut request = request(1, 7);
    request.target = ClipboardTarget::Mime("image/png".to_owned());

    assert_eq!(
        portal.request_import(request),
        Err(PortalError::UnsupportedTarget)
    );
}

#[test]
fn denied_import_fails_selection_normally() {
    let mut portal = ClipboardPortal::new();
    portal.request_import(request(1, 7)).unwrap();

    assert_eq!(
        portal.deny(PortalTransferId::from_raw(1)),
        Ok(PortalCommand::FailSelection {
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
fn approval_requires_matching_generation() {
    let mut portal = ClipboardPortal::new();
    portal.request_import(request(1, 7)).unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 7),
        Ok(PortalCommand::HandoffClipboard {
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
fn stale_generation_revokes_pending_transfer() {
    let mut portal = ClipboardPortal::new();
    portal.request_import(request(1, 7)).unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 8),
        Ok(PortalCommand::FailSelection {
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
fn source_owner_change_revokes_pending_transfers() {
    let mut portal = ClipboardPortal::new();
    portal.request_import(request(1, 7)).unwrap();
    portal.request_import(request(2, 9)).unwrap();

    let commands = portal.source_owner_changed(NamespaceId::from_raw(10), 9);

    assert_eq!(
        commands,
        vec![PortalCommand::FailSelection {
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

#[test]
fn bridge_owner_change_event_revokes_stale_pending_transfers() {
    let mut portal = ClipboardPortal::new();
    portal.request_import(request(1, 7)).unwrap();
    portal.request_import(request(2, 9)).unwrap();

    let commands = portal.apply_owner_changed(ClipboardSourceOwnerChanged {
        source_namespace: NamespaceId::from_raw(10),
        generation: 9,
    });

    assert_eq!(
        commands,
        vec![PortalCommand::FailSelection {
            transfer: PortalTransferId::from_raw(1)
        }]
    );
    assert_eq!(
        portal
            .transfer(PortalTransferId::from_raw(2))
            .unwrap()
            .decision,
        PortalDecision::Pending
    );
}

use sophia_portal::{
    FileHandoffMode, FileHandoffPortal, FileHandoffRequest, MAX_FILE_HANDOFF_TYPES,
    MAX_SUGGESTED_FILE_NAME_LEN, PortalCommand, PortalError,
};
use sophia_protocol::{NamespaceId, PortalDecision, PortalTransferId, PortalTransferKind};

fn request(transfer: u64, generation: u64, mode: FileHandoffMode) -> FileHandoffRequest {
    FileHandoffRequest {
        transfer: PortalTransferId::from_raw(transfer),
        source_namespace: NamespaceId::from_raw(10),
        target_namespace: NamespaceId::from_raw(20),
        mode,
        offered_types: vec!["application/pdf".to_owned(), "text/plain".to_owned()],
        suggested_name: Some("report.pdf".to_owned()),
        byte_size: 4096,
        generation,
    }
}

#[test]
fn file_open_handoff_is_pending_by_default() {
    let mut portal = FileHandoffPortal::new();

    let command = portal
        .request_handoff(request(1, 7, FileHandoffMode::Open))
        .unwrap();

    match command {
        PortalCommand::PromptFileHandoff(transfer) => {
            assert_eq!(transfer.transfer, PortalTransferId::from_raw(1));
            assert_eq!(transfer.kind, PortalTransferKind::FileHandoff);
            assert_eq!(transfer.mime_type, Some("open:application/pdf".to_owned()));
            assert_eq!(transfer.decision, PortalDecision::Pending);
            assert_eq!(transfer.generation, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn file_save_handoff_records_save_mode_hint() {
    let mut portal = FileHandoffPortal::new();

    let command = portal
        .request_handoff(request(1, 7, FileHandoffMode::Save))
        .unwrap();

    match command {
        PortalCommand::PromptFileHandoff(transfer) => {
            assert_eq!(transfer.mime_type, Some("save:application/pdf".to_owned()));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn file_handoff_requires_at_least_one_offered_type() {
    let mut portal = FileHandoffPortal::new();
    let mut request = request(1, 7, FileHandoffMode::Open);
    request.offered_types.clear();

    assert_eq!(
        portal.request_handoff(request),
        Err(PortalError::MissingTransferType)
    );
}

#[test]
fn file_handoff_rejects_excessive_offered_types() {
    let mut portal = FileHandoffPortal::new();
    let mut request = request(1, 7, FileHandoffMode::Open);
    request.offered_types = (0..=MAX_FILE_HANDOFF_TYPES)
        .map(|index| format!("application/x-sophia-{index}"))
        .collect();

    assert_eq!(
        portal.request_handoff(request),
        Err(PortalError::TooManyTransferTypes)
    );
}

#[test]
fn file_handoff_rejects_path_like_suggested_names() {
    let mut portal = FileHandoffPortal::new();

    for suggested_name in ["../secret.txt", "nested/file.txt", "", "."] {
        let mut request = request(1, 7, FileHandoffMode::Save);
        request.suggested_name = Some(suggested_name.to_owned());

        assert_eq!(
            portal.request_handoff(request),
            Err(PortalError::InvalidSuggestedName)
        );
    }
}

#[test]
fn file_handoff_rejects_overlong_suggested_names() {
    let mut portal = FileHandoffPortal::new();
    let mut request = request(1, 7, FileHandoffMode::Save);
    request.suggested_name = Some("x".repeat(MAX_SUGGESTED_FILE_NAME_LEN + 1));

    assert_eq!(
        portal.request_handoff(request),
        Err(PortalError::InvalidSuggestedName)
    );
}

#[test]
fn denied_file_handoff_cancels_request() {
    let mut portal = FileHandoffPortal::new();
    portal
        .request_handoff(request(1, 7, FileHandoffMode::Open))
        .unwrap();

    assert_eq!(
        portal.deny(PortalTransferId::from_raw(1)),
        Ok(PortalCommand::CancelFileHandoff {
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
fn file_handoff_approval_requires_matching_generation() {
    let mut portal = FileHandoffPortal::new();
    portal
        .request_handoff(request(1, 7, FileHandoffMode::Open))
        .unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 7),
        Ok(PortalCommand::HandoffFile {
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
fn stale_file_handoff_generation_revokes_request() {
    let mut portal = FileHandoffPortal::new();
    portal
        .request_handoff(request(1, 7, FileHandoffMode::Open))
        .unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 8),
        Ok(PortalCommand::CancelFileHandoff {
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
fn source_owner_change_revokes_pending_file_handoff() {
    let mut portal = FileHandoffPortal::new();
    portal
        .request_handoff(request(1, 7, FileHandoffMode::Open))
        .unwrap();
    portal
        .request_handoff(request(2, 9, FileHandoffMode::Save))
        .unwrap();

    let commands = portal.source_owner_changed(NamespaceId::from_raw(10), 9);

    assert_eq!(
        commands,
        vec![PortalCommand::CancelFileHandoff {
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

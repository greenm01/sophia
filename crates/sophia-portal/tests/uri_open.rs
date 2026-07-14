use sophia_portal::{MAX_URI_LEN, PortalCommand, PortalError, UriOpenPortal, UriOpenRequest};
use sophia_protocol::{NamespaceId, PortalDecision, PortalTransferId, PortalTransferKind};

fn request(transfer: u64, generation: u64, uri: &str) -> UriOpenRequest {
    UriOpenRequest {
        transfer: PortalTransferId::from_raw(transfer),
        source_namespace: NamespaceId::from_raw(10),
        target_namespace: NamespaceId::from_raw(20),
        uri: uri.to_owned(),
        generation,
    }
}

#[test]
fn uri_open_request_is_pending_by_default() {
    let mut portal = UriOpenPortal::new();

    let command = portal
        .request_open(request(1, 7, "https://example.test/path"))
        .unwrap();

    match command {
        PortalCommand::PromptUriOpen(transfer) => {
            assert_eq!(transfer.transfer, PortalTransferId::from_raw(1));
            assert_eq!(transfer.kind, PortalTransferKind::UriOpen);
            assert_eq!(transfer.mime_type, Some("uri-open:https".to_owned()));
            assert_eq!(transfer.byte_size, "https://example.test/path".len() as u64);
            assert_eq!(transfer.decision, PortalDecision::Pending);
            assert_eq!(transfer.generation, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn uri_open_accepts_small_scheme_allowlist() {
    let mut portal = UriOpenPortal::new();

    for (index, uri) in [
        "http://example.test",
        "https://example.test",
        "mailto:person@example.test",
        "tel:+15555555555",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            portal
                .request_open(request(index as u64 + 1, 7, uri))
                .is_ok()
        );
    }
}

#[test]
fn uri_open_rejects_missing_or_unsupported_scheme() {
    let mut portal = UriOpenPortal::new();

    assert_eq!(
        portal.request_open(request(1, 7, "example.test/path")),
        Err(PortalError::InvalidUri)
    );
    assert_eq!(
        portal.request_open(request(2, 7, "file:///etc/passwd")),
        Err(PortalError::UnsupportedUriScheme)
    );
}

#[test]
fn uri_open_rejects_whitespace_control_and_overlong_uri() {
    let mut portal = UriOpenPortal::new();

    assert_eq!(
        portal.request_open(request(1, 7, "https://example.test/a path")),
        Err(PortalError::InvalidUri)
    );
    assert_eq!(
        portal.request_open(request(2, 7, "https://example.test/\n")),
        Err(PortalError::InvalidUri)
    );
    assert_eq!(
        portal.request_open(request(
            3,
            7,
            &format!("https://{}", "x".repeat(MAX_URI_LEN))
        )),
        Err(PortalError::InvalidUri)
    );
}

#[test]
fn denied_uri_open_cancels_request() {
    let mut portal = UriOpenPortal::new();
    portal
        .request_open(request(1, 7, "https://example.test"))
        .unwrap();

    assert_eq!(
        portal.deny(PortalTransferId::from_raw(1)),
        Ok(PortalCommand::CancelUriOpen {
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
fn uri_open_approval_requires_matching_generation() {
    let mut portal = UriOpenPortal::new();
    portal
        .request_open(request(1, 7, "https://example.test"))
        .unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 7),
        Ok(PortalCommand::HandoffUriOpen {
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
fn stale_uri_open_generation_revokes_request() {
    let mut portal = UriOpenPortal::new();
    portal
        .request_open(request(1, 7, "https://example.test"))
        .unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 8),
        Ok(PortalCommand::CancelUriOpen {
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
fn source_owner_change_revokes_pending_uri_open() {
    let mut portal = UriOpenPortal::new();
    portal
        .request_open(request(1, 7, "https://example.test/a"))
        .unwrap();
    portal
        .request_open(request(2, 9, "https://example.test/b"))
        .unwrap();

    let commands = portal.source_owner_changed(NamespaceId::from_raw(10), 9);

    assert_eq!(
        commands,
        vec![PortalCommand::CancelUriOpen {
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

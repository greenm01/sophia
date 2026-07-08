use sophia_portal::{
    MAX_NOTIFICATION_ACTIONS, MAX_NOTIFICATION_BODY_LEN, MAX_NOTIFICATION_SUMMARY_LEN,
    NotificationPortal, NotificationRequest, NotificationUrgency, PortalCommand, PortalError,
};
use sophia_protocol::{NamespaceId, PortalDecision, PortalTransferId, PortalTransferKind};

fn request(transfer: u64, generation: u64) -> NotificationRequest {
    NotificationRequest {
        transfer: PortalTransferId::from_raw(transfer),
        source_namespace: NamespaceId::from_raw(10),
        target_namespace: NamespaceId::from_raw(20),
        summary: "Build finished".to_owned(),
        body: Some("Sophia workspace checks completed.".to_owned()),
        urgency: NotificationUrgency::Normal,
        actions: vec!["Open".to_owned(), "Dismiss".to_owned()],
        generation,
    }
}

#[test]
fn notification_request_is_pending_by_default() {
    let mut portal = NotificationPortal::new();

    let command = portal.request_display(request(1, 7)).unwrap();

    match command {
        PortalCommand::PromptNotification(transfer) => {
            assert_eq!(transfer.transfer, PortalTransferId::from_raw(1));
            assert_eq!(transfer.kind, PortalTransferKind::Notification);
            assert_eq!(transfer.mime_type, Some("notification:normal".to_owned()));
            assert_eq!(transfer.decision, PortalDecision::Pending);
            assert_eq!(transfer.generation, 7);
            assert!(transfer.byte_size > 0);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn notification_records_urgency_hint() {
    let mut portal = NotificationPortal::new();
    let mut request = request(1, 7);
    request.urgency = NotificationUrgency::Critical;

    let command = portal.request_display(request).unwrap();

    match command {
        PortalCommand::PromptNotification(transfer) => {
            assert_eq!(transfer.mime_type, Some("notification:critical".to_owned()));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn notification_rejects_empty_or_control_text() {
    let mut portal = NotificationPortal::new();
    let mut empty_summary = request(1, 7);
    empty_summary.summary.clear();
    let mut control_body = request(2, 7);
    control_body.body = Some("bad\nbody".to_owned());

    assert_eq!(
        portal.request_display(empty_summary),
        Err(PortalError::InvalidNotificationText)
    );
    assert_eq!(
        portal.request_display(control_body),
        Err(PortalError::InvalidNotificationText)
    );
}

#[test]
fn notification_rejects_overlong_text() {
    let mut portal = NotificationPortal::new();
    let mut long_summary = request(1, 7);
    long_summary.summary = "x".repeat(MAX_NOTIFICATION_SUMMARY_LEN + 1);
    let mut long_body = request(2, 7);
    long_body.body = Some("x".repeat(MAX_NOTIFICATION_BODY_LEN + 1));

    assert_eq!(
        portal.request_display(long_summary),
        Err(PortalError::InvalidNotificationText)
    );
    assert_eq!(
        portal.request_display(long_body),
        Err(PortalError::InvalidNotificationText)
    );
}

#[test]
fn notification_rejects_excessive_actions() {
    let mut portal = NotificationPortal::new();
    let mut request = request(1, 7);
    request.actions = (0..=MAX_NOTIFICATION_ACTIONS)
        .map(|index| format!("Action {index}"))
        .collect();

    assert_eq!(
        portal.request_display(request),
        Err(PortalError::TooManyNotificationActions)
    );
}

#[test]
fn denied_notification_is_dropped() {
    let mut portal = NotificationPortal::new();
    portal.request_display(request(1, 7)).unwrap();

    assert_eq!(
        portal.deny(PortalTransferId::from_raw(1)),
        Ok(PortalCommand::DropNotification {
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
fn notification_approval_requires_matching_generation() {
    let mut portal = NotificationPortal::new();
    portal.request_display(request(1, 7)).unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 7),
        Ok(PortalCommand::DeliverNotification {
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
fn stale_notification_generation_revokes_request() {
    let mut portal = NotificationPortal::new();
    portal.request_display(request(1, 7)).unwrap();

    assert_eq!(
        portal.approve_generation(PortalTransferId::from_raw(1), 8),
        Ok(PortalCommand::DropNotification {
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
fn source_owner_change_revokes_pending_notification() {
    let mut portal = NotificationPortal::new();
    portal.request_display(request(1, 7)).unwrap();
    portal.request_display(request(2, 9)).unwrap();

    let commands = portal.source_owner_changed(NamespaceId::from_raw(10), 9);

    assert_eq!(
        commands,
        vec![PortalCommand::DropNotification {
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

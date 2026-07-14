use sophia_portal::{PortalLifecycleError, PortalPolicyDecision, PortalRequestGrantLifecycle};
use sophia_protocol::{
    NamespaceId, PortalDecision, PortalGrantState, PortalRequest, PortalTransfer, PortalTransferId,
    PortalTransferKind,
};

fn request(id: u64, generation: u64, deadline_msec: u64) -> PortalRequest {
    PortalRequest {
        transfer: PortalTransfer {
            transfer: PortalTransferId::from_raw(id),
            source_namespace: NamespaceId::from_raw(10),
            target_namespace: NamespaceId::from_raw(20),
            kind: PortalTransferKind::Clipboard,
            mime_type: Some("UTF8_STRING".to_owned()),
            byte_size: 6,
            decision: PortalDecision::Pending,
            generation,
        },
        deadline_msec,
    }
}

#[test]
fn allowed_request_creates_separate_single_use_grant() {
    let mut lifecycle = PortalRequestGrantLifecycle::new(3).unwrap();
    lifecycle.submit(request(1, 7, 2_000), 0).unwrap();
    let grant = lifecycle
        .decide(
            PortalTransferId::from_raw(1),
            PortalPolicyDecision::Allow,
            7,
            10,
        )
        .unwrap()
        .unwrap();
    assert_eq!(grant.state, PortalGrantState::Active);
    assert_eq!(grant.broker_generation, 3);
    lifecycle.complete(PortalTransferId::from_raw(1)).unwrap();
    assert_eq!(
        lifecycle
            .grant(PortalTransferId::from_raw(1))
            .unwrap()
            .state,
        PortalGrantState::Completed
    );
    assert_eq!(
        lifecycle.complete(PortalTransferId::from_raw(1)),
        Err(PortalLifecycleError::GrantNotActive)
    );
}

#[test]
fn denial_stale_generation_and_deadline_fail_closed() {
    let mut lifecycle = PortalRequestGrantLifecycle::new(1).unwrap();
    lifecycle.submit(request(1, 7, 2_000), 0).unwrap();
    assert_eq!(
        lifecycle
            .decide(
                PortalTransferId::from_raw(1),
                PortalPolicyDecision::Deny,
                7,
                1
            )
            .unwrap(),
        None
    );
    lifecycle.submit(request(2, 8, 2_000), 0).unwrap();
    assert_eq!(
        lifecycle.decide(
            PortalTransferId::from_raw(2),
            PortalPolicyDecision::Allow,
            9,
            1
        ),
        Err(PortalLifecycleError::StaleSourceGeneration)
    );
    lifecycle.submit(request(3, 10, 20), 0).unwrap();
    assert_eq!(lifecycle.expire(20), vec![PortalTransferId::from_raw(3)]);
}

#[test]
fn disconnect_executor_failure_and_restart_revoke_active_grants() {
    let mut lifecycle = PortalRequestGrantLifecycle::new(1).unwrap();
    for id in 1..=3 {
        lifecycle.submit(request(id, 7, 2_000), 0).unwrap();
        lifecycle
            .decide(
                PortalTransferId::from_raw(id),
                PortalPolicyDecision::Allow,
                7,
                1,
            )
            .unwrap();
    }
    lifecycle
        .executor_failed(PortalTransferId::from_raw(1))
        .unwrap();
    assert_eq!(
        lifecycle
            .grant(PortalTransferId::from_raw(1))
            .unwrap()
            .state,
        PortalGrantState::Revoked
    );
    assert_eq!(
        lifecycle.namespace_disconnected(NamespaceId::from_raw(20)),
        vec![PortalTransferId::from_raw(2), PortalTransferId::from_raw(3)]
    );

    let mut lifecycle = PortalRequestGrantLifecycle::new(1).unwrap();
    lifecycle.submit(request(4, 7, 2_000), 0).unwrap();
    lifecycle
        .decide(
            PortalTransferId::from_raw(4),
            PortalPolicyDecision::Allow,
            7,
            1,
        )
        .unwrap();
    assert_eq!(
        lifecycle.broker_restarted(2).unwrap(),
        vec![PortalTransferId::from_raw(4)]
    );
}

#[test]
fn lifecycle_enforces_capacity_and_unique_transfers() {
    let mut lifecycle = PortalRequestGrantLifecycle::with_capacity(1, 1).unwrap();
    lifecycle.submit(request(1, 7, 2_000), 0).unwrap();
    assert_eq!(
        lifecycle.submit(request(1, 7, 2_000), 0),
        Err(PortalLifecycleError::DuplicateTransfer)
    );
    assert_eq!(
        lifecycle.submit(request(2, 7, 2_000), 0),
        Err(PortalLifecycleError::CapacityExceeded)
    );
}

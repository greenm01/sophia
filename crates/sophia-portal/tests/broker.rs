use sophia_portal::{
    DeterministicPortalBroker, HeadlessPortalPolicy, PortalBrokerDecision,
    PortalCapabilityAdmission,
};
use sophia_protocol::{
    NamespaceId, PortalDecision, PortalRequest, PortalTransfer, PortalTransferId,
    PortalTransferKind,
};

fn request() -> PortalRequest {
    PortalRequest {
        transfer: PortalTransfer {
            transfer: PortalTransferId::from_raw(1),
            source_namespace: NamespaceId::from_raw(10),
            target_namespace: NamespaceId::from_raw(20),
            kind: PortalTransferKind::Clipboard,
            mime_type: Some("UTF8_STRING".to_owned()),
            byte_size: 6,
            decision: PortalDecision::Pending,
            generation: 7,
        },
        deadline_msec: 2_000,
    }
}

#[test]
fn broker_denies_by_default_even_with_capabilities() {
    let mut broker = DeterministicPortalBroker::new(1, HeadlessPortalPolicy::default()).unwrap();
    assert_eq!(
        broker
            .request(
                request(),
                PortalCapabilityAdmission {
                    source_may_publish: true,
                    target_may_request: true,
                },
                0,
            )
            .unwrap(),
        PortalBrokerDecision::Denied
    );
}

#[test]
fn explicit_allow_still_requires_both_directional_capabilities() {
    for admission in [
        PortalCapabilityAdmission {
            source_may_publish: false,
            target_may_request: true,
        },
        PortalCapabilityAdmission {
            source_may_publish: true,
            target_may_request: false,
        },
    ] {
        let mut broker = DeterministicPortalBroker::new(1, HeadlessPortalPolicy::Allow).unwrap();
        assert_eq!(
            broker.request(request(), admission, 0).unwrap(),
            PortalBrokerDecision::Denied
        );
    }
}

#[test]
fn explicit_allow_with_capabilities_creates_grant() {
    let mut broker = DeterministicPortalBroker::new(4, HeadlessPortalPolicy::Allow).unwrap();
    let decision = broker
        .request(
            request(),
            PortalCapabilityAdmission {
                source_may_publish: true,
                target_may_request: true,
            },
            0,
        )
        .unwrap();
    let PortalBrokerDecision::Allowed(grant) = decision else {
        panic!("expected allowed grant");
    };
    assert_eq!(grant.broker_generation, 4);
    broker.complete(grant.transfer).unwrap();
}

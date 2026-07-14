#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
use sophia_portal::{
    HeadlessPortalPolicy, request_portal_broker, request_portal_broker_with_clipboard_payload,
    run_portal_broker_socket_server_bounded, run_portal_broker_socket_server_once,
    run_portal_clipboard_broker_socket_server_bounded,
};
#[cfg(unix)]
use sophia_protocol::{
    NamespaceId, PortalBrokerRequestPacket, PortalBrokerResponseDecision, PortalDecision,
    PortalRequest, PortalTransfer, PortalTransferId, PortalTransferKind,
};

#[cfg(unix)]
#[test]
fn owner_only_socket_roundtrips_allowed_request() {
    let path = std::env::temp_dir().join(format!("sophia-portal-{}.sock", std::process::id()));
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        run_portal_broker_socket_server_once(server_path, 3, HeadlessPortalPolicy::Allow, 10)
    });
    for _ in 0..100 {
        if let Ok(metadata) = std::fs::metadata(&path) {
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let transfer = PortalTransferId::from_raw(1);
    let response = request_portal_broker(
        &path,
        &PortalBrokerRequestPacket {
            request: PortalRequest {
                transfer: PortalTransfer {
                    transfer,
                    source_namespace: NamespaceId::from_raw(10),
                    target_namespace: NamespaceId::from_raw(20),
                    kind: PortalTransferKind::Clipboard,
                    mime_type: Some("UTF8_STRING".to_owned()),
                    byte_size: 6,
                    decision: PortalDecision::Pending,
                    generation: 7,
                },
                deadline_msec: 2_000,
            },
            source_may_publish: true,
            target_may_request: true,
        },
    )
    .unwrap();
    assert!(matches!(
        response.decision,
        PortalBrokerResponseDecision::Allowed(_)
    ));
    server.join().unwrap().unwrap();
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn allowed_payload_reaches_executor_only_after_correlated_grant() {
    let path =
        std::env::temp_dir().join(format!("sophia-portal-payload-{}.sock", std::process::id()));
    let observed = std::sync::Arc::new(std::sync::Mutex::new(None));
    let server_observed = observed.clone();
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        run_portal_clipboard_broker_socket_server_bounded(
            server_path,
            12,
            HeadlessPortalPolicy::Allow,
            10,
            1,
            move |grant, payload| {
                *server_observed.lock().unwrap() =
                    Some((grant.transfer, grant.broker_generation, payload.to_vec()));
                Ok(())
            },
        )
    });
    for _ in 0..100 {
        if path.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let transfer = PortalTransferId::from_raw(44);
    let response = request_portal_broker_with_clipboard_payload(
        &path,
        &PortalBrokerRequestPacket {
            request: PortalRequest {
                transfer: PortalTransfer {
                    transfer,
                    source_namespace: NamespaceId::from_raw(10),
                    target_namespace: NamespaceId::from_raw(20),
                    kind: PortalTransferKind::Clipboard,
                    mime_type: Some("UTF8_STRING".to_owned()),
                    byte_size: 6,
                    decision: PortalDecision::Pending,
                    generation: 7,
                },
                deadline_msec: 2_000,
            },
            source_may_publish: true,
            target_may_request: true,
        },
        b"sophia",
    )
    .unwrap();
    assert!(matches!(
        response.decision,
        PortalBrokerResponseDecision::Allowed(_)
    ));
    server.join().unwrap().unwrap();
    assert_eq!(
        *observed.lock().unwrap(),
        Some((transfer, 12, b"sophia".to_vec()))
    );
}

#[cfg(unix)]
#[test]
fn bounded_server_retains_one_broker_generation_across_connections() {
    let path =
        std::env::temp_dir().join(format!("sophia-portal-bounded-{}.sock", std::process::id()));
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        run_portal_broker_socket_server_bounded(server_path, 9, HeadlessPortalPolicy::Allow, 10, 2)
    });
    for _ in 0..100 {
        if path.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let request = |transfer| PortalBrokerRequestPacket {
        request: PortalRequest {
            transfer: PortalTransfer {
                transfer: PortalTransferId::from_raw(transfer),
                source_namespace: NamespaceId::from_raw(10),
                target_namespace: NamespaceId::from_raw(20),
                kind: PortalTransferKind::Clipboard,
                mime_type: Some("UTF8_STRING".to_owned()),
                byte_size: 6,
                decision: PortalDecision::Pending,
                generation: 7,
            },
            deadline_msec: 2_000,
        },
        source_may_publish: true,
        target_may_request: true,
    };
    for transfer in [1, 2] {
        let response = request_portal_broker(&path, &request(transfer)).unwrap();
        let PortalBrokerResponseDecision::Allowed(grant) = response.decision else {
            panic!("expected grant");
        };
        assert_eq!(grant.broker_generation, 9);
        assert_eq!(grant.transfer, PortalTransferId::from_raw(transfer));
    }
    server.join().unwrap().unwrap();
    assert!(!path.exists());
}

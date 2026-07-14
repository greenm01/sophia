#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
use sophia_portal::{
    HeadlessPortalPolicy, request_portal_broker, run_portal_broker_socket_server_once,
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

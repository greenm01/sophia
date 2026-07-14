use sophia_protocol::{
    ClientAdmissionId, ClientAuthenticationMethod, NamespaceCapabilities, NamespaceId,
    NamespacePortalCapability, NamespaceProfile,
};
use sophia_runtime::{NamespaceRegistry, NamespaceRegistryError, SophiaErrorExt, SophiaErrorKind};

#[test]
fn classic_admissions_share_one_immutable_namespace() {
    let mut registry = NamespaceRegistry::new(7).unwrap();
    let capabilities = NamespaceCapabilities::NONE
        .with_request(NamespacePortalCapability::Clipboard)
        .with_publish(NamespacePortalCapability::Clipboard);
    let namespace = registry.create_namespace(NamespaceProfile::ClassicShared, capabilities);

    let first = registry
        .admit(namespace.id, ClientAuthenticationMethod::MitMagicCookie1)
        .unwrap();
    let second = registry
        .admit(namespace.id, ClientAuthenticationMethod::PeerCredentials)
        .unwrap();

    assert_ne!(first.client_id, second.client_id);
    assert_eq!(first.namespace, namespace);
    assert_eq!(second.namespace, namespace);
    assert_eq!(first.auth_provenance.session_generation, 7);
    assert!(registry.is_current_admission(first));
    assert!(registry.is_current_admission(second));
    assert_eq!(registry.namespace_count(), 1);
    assert_eq!(registry.admission_count(), 2);
}

#[test]
fn confined_client_groups_receive_disjoint_namespaces() {
    let mut registry = NamespaceRegistry::new(1).unwrap();
    let first_namespace =
        registry.create_namespace(NamespaceProfile::Confined, NamespaceCapabilities::NONE);
    let second_namespace =
        registry.create_namespace(NamespaceProfile::Confined, NamespaceCapabilities::NONE);

    let first = registry
        .admit(
            first_namespace.id,
            ClientAuthenticationMethod::PeerCredentials,
        )
        .unwrap();
    let second = registry
        .admit(
            second_namespace.id,
            ClientAuthenticationMethod::PeerCredentials,
        )
        .unwrap();

    assert_ne!(first.namespace.id, second.namespace.id);
    assert_eq!(first.namespace.profile, NamespaceProfile::Confined);
    assert_eq!(second.namespace.profile, NamespaceProfile::Confined);
}

#[test]
fn revoking_one_admission_preserves_its_classic_peer() {
    let mut registry = NamespaceRegistry::new(2).unwrap();
    let namespace =
        registry.create_namespace(NamespaceProfile::ClassicShared, NamespaceCapabilities::ALL);
    let first = registry
        .admit(namespace.id, ClientAuthenticationMethod::TrustedLocal)
        .unwrap();
    let second = registry
        .admit(namespace.id, ClientAuthenticationMethod::TrustedLocal)
        .unwrap();

    assert_eq!(registry.revoke_admission(first.client_id), Ok(first));
    assert!(!registry.is_current_admission(first));
    assert!(registry.is_current_admission(second));
    assert_eq!(registry.admission_count(), 1);
}

#[test]
fn revoking_a_namespace_atomically_invalidates_its_admissions() {
    let mut registry = NamespaceRegistry::new(3).unwrap();
    let revoked_namespace =
        registry.create_namespace(NamespaceProfile::Confined, NamespaceCapabilities::NONE);
    let retained_namespace =
        registry.create_namespace(NamespaceProfile::Confined, NamespaceCapabilities::NONE);
    let first = registry
        .admit(
            revoked_namespace.id,
            ClientAuthenticationMethod::MitMagicCookie1,
        )
        .unwrap();
    let second = registry
        .admit(
            revoked_namespace.id,
            ClientAuthenticationMethod::MitMagicCookie1,
        )
        .unwrap();
    let retained = registry
        .admit(
            retained_namespace.id,
            ClientAuthenticationMethod::MitMagicCookie1,
        )
        .unwrap();

    let revocation = registry.revoke_namespace(revoked_namespace.id).unwrap();

    assert_eq!(revocation.namespace, revoked_namespace);
    assert_eq!(revocation.admissions, vec![first, second]);
    assert!(!registry.is_current_admission(first));
    assert!(!registry.is_current_admission(second));
    assert!(registry.is_current_admission(retained));
    assert_eq!(registry.namespace_count(), 1);
    assert_eq!(registry.admission_count(), 1);
}

#[test]
fn registry_fails_closed_for_invalid_or_unknown_identity() {
    assert_eq!(
        NamespaceRegistry::new(0).unwrap_err(),
        NamespaceRegistryError::InvalidSessionGeneration
    );

    let mut registry = NamespaceRegistry::new(1).unwrap();
    let unknown_namespace = NamespaceId::from_raw(44);
    let error = registry
        .admit(unknown_namespace, ClientAuthenticationMethod::TrustedLocal)
        .unwrap_err();
    assert_eq!(
        error,
        NamespaceRegistryError::UnknownNamespace {
            namespace: unknown_namespace
        }
    );
    assert_eq!(error.kind(), SophiaErrorKind::InvalidNamespace);
    assert_eq!(
        registry.revoke_admission(ClientAdmissionId::from_raw(9)),
        Err(NamespaceRegistryError::UnknownAdmission {
            client: ClientAdmissionId::from_raw(9)
        })
    );
}

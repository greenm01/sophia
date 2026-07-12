mod support;

use support::*;

#[test]
fn probe_reports_missing_required_extensions() {
    let probe = XConnectionProbe {
        display_name: Some(":99".to_owned()),
        screen_num: 0,
        required_extensions: vec![
            status(RequiredExtension::Composite, true),
            status(RequiredExtension::Damage, false),
        ],
        namespaces: StaticNamespaceConfig::default(),
    };

    assert_eq!(probe.missing_extensions(), vec![RequiredExtension::Damage]);
    assert!(!probe.has_required_extensions());
}

#[test]
fn static_namespace_config_records_known_namespaces() {
    let config = StaticNamespaceConfig::new(vec![NamespaceRecord {
        namespace: NamespaceId::from_raw(1),
        label: "trusted".to_owned(),
        source: NamespaceSource::StaticConfig,
    }]);

    assert_eq!(config.namespaces().len(), 1);
    assert_eq!(config.namespaces()[0].label, "trusted");
    assert_eq!(config.namespaces()[0].source, NamespaceSource::StaticConfig);
}

#[test]
fn discovered_namespace_records_replace_static_records() {
    let config = StaticNamespaceConfig::new(vec![NamespaceRecord {
        namespace: NamespaceId::from_raw(1),
        label: "trusted-static".to_owned(),
        source: NamespaceSource::StaticConfig,
    }])
    .with_discovered(vec![
        NamespaceRecord {
            namespace: NamespaceId::from_raw(1),
            label: "trusted-server".to_owned(),
            source: NamespaceSource::XServer,
        },
        NamespaceRecord {
            namespace: NamespaceId::from_raw(2),
            label: "browser".to_owned(),
            source: NamespaceSource::XServer,
        },
    ]);

    assert_eq!(config.namespaces().len(), 2);
    assert_eq!(config.namespaces()[0].label, "trusted-server");
    assert_eq!(config.namespaces()[0].source, NamespaceSource::XServer);
    assert_eq!(config.namespaces()[1].label, "browser");
}

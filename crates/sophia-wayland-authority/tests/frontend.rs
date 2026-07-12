use sophia_protocol::Size;
use sophia_wayland_authority::WaylandFrontend;

#[test]
fn frontend_binds_private_socket_and_dispatches_without_clients() {
    let runtime = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("XDG_RUNTIME_DIR", runtime.path());
    }
    let mut frontend = WaylandFrontend::bind(
        "sophia-test-0",
        Size {
            width: 800,
            height: 600,
        },
    )
    .unwrap();

    assert_eq!(frontend.display_name(), Some("sophia-test-0"));
    assert!(frontend.dispatch().unwrap().is_empty());
    assert!(runtime.path().join("sophia-test-0").exists());
}

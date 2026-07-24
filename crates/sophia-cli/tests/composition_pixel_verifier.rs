use std::process::Command;

fn evidence_file(name: &str, dmabuf_checksum: u64, dmabuf_rgb: usize) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sophia-composition-pixels-{name}-{}.log",
        std::process::id()
    ));
    let evidence = format!(
        "sophia_native_composition_pixels schema=1 status=read stage=cpu layer=0 target=10x10_0_0 format=0x34325258 modifier=0xffffffffffffffff stride=40 pixels=100 nonzero_rgb_pixels=0 alpha_zero_pixels=0 alpha_partial_pixels=0 alpha_opaque_pixels=100 checksum=11\n\
         sophia_native_composition_pixels schema=1 status=read stage=dmabuf layer=1 target=10x10_0_0 format=0x34325241 modifier=0x0 stride=40 pixels=100 nonzero_rgb_pixels={dmabuf_rgb} alpha_zero_pixels=2 alpha_partial_pixels=3 alpha_opaque_pixels=95 checksum={dmabuf_checksum}\n"
    );
    std::fs::write(&path, evidence).unwrap();
    path
}

fn verifier(path: &std::path::Path) -> std::process::Output {
    Command::new("bash")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/verify_sophia_native_composition_pixels.sh"
        ))
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn verifier_accepts_visible_client_layer_delta() {
    let path = evidence_file("visible", 22, 8);
    let output = verifier(&path);
    let _ = std::fs::remove_file(path);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("boundary=client_layer_visible"));
}

#[test]
fn verifier_rejects_a_client_layer_without_framebuffer_delta() {
    let path = evidence_file("unchanged", 11, 0);
    let output = verifier(&path);
    let _ = std::fs::remove_file(path);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("boundary=client_layer_no_framebuffer_delta")
    );
}

#![cfg(feature = "atomic-scanout-smoke-live")]

use sophia_cli::backend_evidence::runtime_rendered_scanout_evidence_is_clean;

fn lines(contents: &str) -> Vec<String> {
    contents.lines().map(str::to_owned).collect()
}

#[test]
fn accepts_clean_runtime_rendered_scanout_evidence() {
    let evidence =
        include_str!("../../../tools/fixtures/runtime_rendered_scanout_evidence_pass.log");

    assert!(runtime_rendered_scanout_evidence_is_clean(&lines(evidence)));
}

#[test]
fn accepts_clean_runtime_rendered_scanout_evidence_with_modifiers() {
    let evidence = include_str!(
        "../../../tools/fixtures/runtime_rendered_scanout_evidence_pass_modifiers.log"
    );

    assert!(runtime_rendered_scanout_evidence_is_clean(&lines(evidence)));
}

#[test]
fn rejects_missing_retire_runtime_rendered_scanout_evidence() {
    let evidence = include_str!(
        "../../../tools/fixtures/runtime_rendered_scanout_evidence_missing_retire.log"
    );

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_cleanup_debt_runtime_rendered_scanout_evidence() {
    let evidence =
        include_str!("../../../tools/fixtures/runtime_rendered_scanout_evidence_cleanup_debt.log");

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_cleanup_retry_runtime_rendered_scanout_evidence() {
    let evidence =
        include_str!("../../../tools/fixtures/runtime_rendered_scanout_evidence_cleanup_retry.log");

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_unknown_runtime_rendered_scanout_field() {
    let evidence =
        include_str!("../../../tools/fixtures/runtime_rendered_scanout_evidence_unknown_field.log");

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_duplicate_runtime_rendered_scanout_field() {
    let evidence = include_str!(
        "../../../tools/fixtures/runtime_rendered_scanout_evidence_duplicate_field.log"
    );

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_malformed_runtime_rendered_scanout_field() {
    let evidence = include_str!(
        "../../../tools/fixtures/runtime_rendered_scanout_evidence_malformed_field.log"
    );

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_runtime_rendered_scanout_failure_line() {
    let evidence =
        include_str!("../../../tools/fixtures/runtime_rendered_scanout_evidence_failure.log");

    assert!(!runtime_rendered_scanout_evidence_is_clean(&lines(
        evidence
    )));
}

#[test]
fn rejects_setup_failure_atomic_scanout_evidence() {
    let evidence = vec![
        "sophia_atomic_scanout_evidence schema=8 phase=InitialModeset status=NoPrimaryCard scanout_target=none rendered_context=none gbm_export=none gbm_export_detail=none scanout_buffer=none properties=none resources=none framebuffer=none request=none submit=none request_scope=none commit_page_flip_event=none commit_nonblocking=none commit_allow_modeset=none commit_test_only=none page_flip_wait=none page_flip_poll=none page_flip=none retire=none retire_destroy=none retire_cleanup_pending=false".to_owned(),
    ];

    assert!(!runtime_rendered_scanout_evidence_is_clean(&evidence));
}

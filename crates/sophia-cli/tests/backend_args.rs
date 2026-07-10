#![cfg(feature = "atomic-scanout-smoke-live")]

use std::time::Duration;

use sophia_cli::backend_args::{
    ATOMIC_SCANOUT_SMOKE_CHILD_TIMEOUT_MS, atomic_scanout_smoke_child_args,
    atomic_scanout_smoke_child_timeout,
};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn atomic_scanout_smoke_child_timeout_defaults_to_bounded_watchdog() {
    let timeout = atomic_scanout_smoke_child_timeout(&args(&["atomic-scanout-smoke"]))
        .expect("default timeout should parse");

    assert_eq!(
        timeout,
        Duration::from_millis(ATOMIC_SCANOUT_SMOKE_CHILD_TIMEOUT_MS)
    );
}

#[test]
fn atomic_scanout_smoke_child_timeout_accepts_operator_override() {
    let timeout = atomic_scanout_smoke_child_timeout(&args(&[
        "atomic-scanout-smoke",
        "--child-timeout-ms=25000",
    ]))
    .expect("override timeout should parse");

    assert_eq!(timeout, Duration::from_millis(25_000));
}

#[test]
fn atomic_scanout_smoke_child_timeout_rejects_zero() {
    let error = atomic_scanout_smoke_child_timeout(&args(&[
        "atomic-scanout-smoke",
        "--child-timeout-ms=0",
    ]))
    .expect_err("zero timeout should be rejected");

    assert!(error.to_string().contains("must be nonzero"));
}

#[test]
fn atomic_scanout_smoke_child_args_do_not_forward_parent_watchdog() {
    let child_args = atomic_scanout_smoke_child_args(&args(&[
        "atomic-scanout-smoke",
        "--slot=2",
        "--output=3",
        "--authority=4",
        "--page-flip-timeout-ms=500",
        "--child-timeout-ms=25000",
    ]));

    assert_eq!(
        child_args,
        vec![
            "--slot=2",
            "--output=3",
            "--authority=4",
            "--page-flip-timeout-ms=500"
        ]
    );
}

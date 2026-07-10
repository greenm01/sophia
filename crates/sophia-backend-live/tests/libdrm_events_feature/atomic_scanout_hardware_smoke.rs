use std::time::{Duration, Instant};

use super::*;
use sophia_backend_live::run_real_atomic_scanout_smoke_phases;

#[test]
fn native_atomic_scanout_smokes_real_primary_card_when_enabled() {
    if std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
        return;
    }

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("atomic_scanout_hardware_smoke::native_atomic_scanout_real_primary_card_child")
        .arg("--nocapture")
        .env("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD", "1")
        .spawn()
        .expect("real atomic scanout smoke child should start");
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if let Some(status) = child
            .try_wait()
            .expect("real atomic scanout smoke child should be waitable")
        {
            assert!(
                status.success(),
                "real atomic scanout smoke child failed with status {status}"
            );
            return;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            println!(
                "{}",
                LibdrmNativeAtomicScanoutSmokeEvidence::smoke_child_timeout().reduced_log_line()
            );
            panic!("real atomic scanout smoke child timed out waiting for page-flip evidence");
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn native_atomic_scanout_real_primary_card_child() {
    if std::env::var_os("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD").is_none() {
        return;
    }

    for evidence in run_real_atomic_scanout_smoke_phases() {
        require_atomic_scanout_smoke_passed(evidence);
    }
}

fn require_atomic_scanout_smoke_passed(evidence: LibdrmNativeAtomicScanoutSmokeEvidence) {
    println!("{}", evidence.reduced_log_line());
    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::Passed
    );
}

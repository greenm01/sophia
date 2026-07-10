use std::time::{Duration, Instant};

use super::*;
use sophia_backend_live::{
    LivePageFlipCallbackIntake, RealAtomicScanoutPageFlipWaitPolicy,
    select_real_atomic_scanout_card,
};

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

    let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
    let output = OutputId::from_raw(1);
    let authority =
        LibdrmBackendFdAuthority::new(1).expect("nonzero authority generation should mint");
    let mut session_result =
        select_real_atomic_scanout_card().into_page_flip_session(slot, output, authority);
    let Some(mut session) = session_result.session.take() else {
        fail_atomic_scanout_smoke(
            session_result
                .failure_evidence()
                .unwrap_or_else(LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed),
        );
    };
    let discovery = match session.render_device_discovery() {
        Ok(discovery) => discovery,
        Err(_) => {
            fail_atomic_scanout_smoke(
                LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                    LiveKmsScanoutTargetStatus::Ready,
                    Some(LibdrmNativeRenderedScanoutContextStatus::Unavailable),
                    LiveRendererScanoutBufferExportStatus::Unavailable,
                    None,
                    None,
                    None,
                    None,
                ),
            );
        }
    };
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery);

    let mut intake = LivePageFlipCallbackIntake::new(output);
    let evidence = session.run_native_gbm_rendered_primary_plane_smoke_phase(
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
        &mut exporter,
        &mut intake,
        RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke(),
    );
    require_atomic_scanout_smoke_passed(evidence);

    let steady_evidence = session.run_native_gbm_rendered_primary_plane_smoke_phase(
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip,
        &mut exporter,
        &mut intake,
        RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke(),
    );
    require_atomic_scanout_smoke_passed(steady_evidence);
}

fn require_atomic_scanout_smoke_passed(evidence: LibdrmNativeAtomicScanoutSmokeEvidence) {
    println!("{}", evidence.reduced_log_line());
    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::Passed
    );
}

fn fail_atomic_scanout_smoke(evidence: LibdrmNativeAtomicScanoutSmokeEvidence) -> ! {
    println!("{}", evidence.reduced_log_line());
    panic!(
        "real atomic scanout smoke failed with status {:?}",
        evidence.status
    );
}

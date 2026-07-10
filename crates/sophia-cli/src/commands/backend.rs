#[cfg(feature = "atomic-scanout-smoke-live")]
use std::time::{Duration, Instant};

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "atomic-scanout-preflight") {
        let report = sophia_backend_live::real_atomic_scanout_preflight_report();
        println!("{}", report.reduced_log_line());

        if report.status
            != sophia_backend_live::LiveAtomicScanoutPreflightStatus::CandidatePrimaryCardsAtomicReady
        {
            return Err(format!(
                "atomic scanout preflight did not find a smoke-ready host: {:?}",
                report.status
            )
            .into());
        }

        return Ok(true);
    }

    #[cfg(feature = "atomic-scanout-smoke-live")]
    if args.iter().any(|arg| arg == "atomic-scanout-smoke") {
        if std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
            return Err("set SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 to run destructive atomic scanout smoke".into());
        }

        run_atomic_scanout_smoke_parent()?;
        return Ok(true);
    }

    #[cfg(feature = "atomic-scanout-smoke-live")]
    if args.iter().any(|arg| arg == "atomic-scanout-smoke-child") {
        if std::env::var_os("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD").is_none() {
            return Ok(true);
        }

        for evidence in sophia_backend_live::run_real_atomic_scanout_smoke_phases() {
            println!("{}", evidence.reduced_log_line());
            if evidence.status != sophia_backend_live::LibdrmNativeAtomicScanoutSmokeStatus::Passed
            {
                return Err(format!(
                    "real atomic scanout smoke failed with status {:?}",
                    evidence.status
                )
                .into());
            }
        }
        return Ok(true);
    }

    Ok(false)
}

#[cfg(feature = "atomic-scanout-smoke-live")]
fn run_atomic_scanout_smoke_parent() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .arg("atomic-scanout-smoke-child")
        .env("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD", "1")
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(
                format!("real atomic scanout smoke child failed with status {status}").into(),
            );
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            println!(
                "{}",
                sophia_backend_live::LibdrmNativeAtomicScanoutSmokeEvidence::smoke_child_timeout()
                    .reduced_log_line()
            );
            return Err(
                "real atomic scanout smoke child timed out waiting for page-flip evidence".into(),
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

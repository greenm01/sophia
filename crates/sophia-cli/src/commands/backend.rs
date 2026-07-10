#[cfg(feature = "atomic-scanout-smoke-live")]
use std::time::{Duration, Instant};

#[cfg(feature = "atomic-scanout-smoke-live")]
use super::prelude::{arg_value, parse_u64};
#[cfg(feature = "atomic-scanout-smoke-live")]
use sophia_cli::backend_args::{
    atomic_scanout_smoke_child_args, atomic_scanout_smoke_child_timeout,
};
#[cfg(feature = "atomic-scanout-smoke-live")]
use sophia_cli::backend_evidence::runtime_rendered_scanout_evidence_is_clean;

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

    if args
        .iter()
        .any(|arg| arg == "live-session-composition-smoke")
    {
        let authority_batches =
            super::x_authority::collect_x_authority_present_pixmap_authority_batches()?;
        let report = sophia_backend_live::run_live_session_composition_smoke(authority_batches);
        println!("{}", report.reduced_log_line());

        if report.status != sophia_backend_live::LiveSessionCompositionSmokeStatus::Passed {
            return Err(format!(
                "live session composition smoke failed with status {:?}",
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

        let _config = atomic_scanout_smoke_cli_config(args)?;
        run_atomic_scanout_smoke_parent(args)?;
        return Ok(true);
    }

    #[cfg(feature = "atomic-scanout-smoke-live")]
    if args
        .iter()
        .any(|arg| arg == "atomic-scanout-runtime-evidence")
    {
        if std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
            return Err("set SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 to run destructive atomic scanout runtime evidence".into());
        }

        let config = atomic_scanout_smoke_cli_config(args)?;
        let lines =
            sophia_backend_live::run_real_atomic_runtime_rendered_scanout_evidence_with(config);
        for line in &lines {
            println!("{line}");
        }
        if !runtime_rendered_scanout_evidence_is_clean(&lines) {
            return Err(
                "atomic scanout runtime evidence did not capture a clean submit-to-retire frame"
                    .into(),
            );
        }
        return Ok(true);
    }

    #[cfg(feature = "atomic-scanout-smoke-live")]
    if args.iter().any(|arg| arg == "atomic-scanout-smoke-child") {
        if std::env::var_os("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD").is_none() {
            return Ok(true);
        }

        let config = atomic_scanout_smoke_cli_config(args)?;
        for evidence in sophia_backend_live::run_real_atomic_scanout_smoke_phases_with(config) {
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
fn run_atomic_scanout_smoke_parent(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let child_timeout = atomic_scanout_smoke_child_timeout(args)?;
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("atomic-scanout-smoke-child")
        .env("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD", "1");
    for arg in atomic_scanout_smoke_child_args(args) {
        command.arg(arg);
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + child_timeout;

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

#[cfg(feature = "atomic-scanout-smoke-live")]
fn atomic_scanout_smoke_cli_config(
    args: &[String],
) -> Result<sophia_backend_live::RealAtomicScanoutSmokeConfig, Box<dyn std::error::Error>> {
    let slot = arg_value(args, "--slot")
        .as_deref()
        .map(parse_u64)
        .transpose()?
        .unwrap_or(1);
    let slot = u16::try_from(slot)
        .map_err(|_| format!("atomic scanout slot {slot} does not fit in u16"))?;
    let output = arg_value(args, "--output")
        .as_deref()
        .map(parse_u64)
        .transpose()?
        .unwrap_or(1);
    let authority = arg_value(args, "--authority")
        .as_deref()
        .map(parse_u64)
        .transpose()?
        .unwrap_or(1);
    let mut wait_policy =
        sophia_backend_live::RealAtomicScanoutPageFlipWaitPolicy::hardware_smoke();
    if let Some(timeout_ms) = arg_value(args, "--page-flip-timeout-ms")
        .as_deref()
        .map(parse_u64)
        .transpose()?
    {
        wait_policy.timeout = Duration::from_millis(timeout_ms);
    }

    sophia_backend_live::RealAtomicScanoutSmokeConfig::from_raw(
        slot,
        output,
        authority,
        wait_policy,
    )
    .ok_or_else(|| {
        format!(
            "invalid atomic scanout smoke config: slot={slot} output={output} authority={authority}"
        )
        .into()
    })
}

#[cfg(feature = "atomic-scanout-smoke-live")]
use std::time::{Duration, Instant};

use super::prelude::{arg_value, parse_u64};

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

        let _config = atomic_scanout_smoke_cli_config(args)?;
        run_atomic_scanout_smoke_parent(args)?;
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
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("atomic-scanout-smoke-child")
        .env("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD", "1");
    for arg in atomic_scanout_smoke_child_args(args) {
        command.arg(arg);
    }
    let mut child = command.spawn()?;
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

#[cfg(feature = "atomic-scanout-smoke-live")]
fn atomic_scanout_smoke_child_args(args: &[String]) -> Vec<String> {
    [
        "--slot",
        "--output",
        "--authority",
        "--page-flip-timeout-ms",
    ]
    .into_iter()
    .filter_map(|key| arg_value(args, key).map(|value| format!("{key}={value}")))
    .collect()
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

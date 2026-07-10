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

    Ok(false)
}

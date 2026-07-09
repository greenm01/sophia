use super::prelude::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "x-smoke-routed-input") {
        let display = arg_value(args, "--display");
        let report = smoke_routed_input(display.as_deref())?;
        println!(
            "x-smoke-routed-input display={} opcode={} target={:#x} device={} outcome={:?} event=button{}@{},{} request_bytes={} dispatch_us={}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.extension_opcode,
            report.target_window.xid(),
            report.device.raw(),
            report.decision.outcome,
            report.button,
            report.event_x,
            report.event_y,
            report.request_bytes,
            report.dispatch_elapsed.as_micros()
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-smoke-routed-input-edges") {
        let reports = smoke_routed_input_edges(sophia_protocol::XWindowId::new(0x30, 1));
        for report in reports {
            println!(
                "x-smoke-routed-input-edges edge={:?} target={:#x} outcome={:?} delivery_allowed={}",
                report.edge,
                report.decision.target_window.xid(),
                report.decision.outcome,
                report.delivery_allowed
            );
        }
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "x-stress-routed-input") {
        let display = arg_value(args, "--display");
        let iterations = arg_value(args, "--iterations")
            .as_deref()
            .map(parse_usize)
            .transpose()?
            .unwrap_or(1_000);
        let threshold_us = arg_value(args, "--threshold-us")
            .as_deref()
            .map(parse_u64)
            .transpose()?
            .unwrap_or(500);
        let threshold = std::time::Duration::from_micros(threshold_us);
        let report = stress_routed_input(display.as_deref(), iterations, threshold)?;
        println!(
            "x-stress-routed-input display={} opcode={} target={:#x} device={} iterations={} accepted={} request_bytes={} min_us={} avg_us={} p95_us={} max_us={} threshold_us={} recommendation={:?}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.extension_opcode,
            report.target_window.xid(),
            report.device.raw(),
            report.iterations,
            report.accepted,
            report.request_bytes,
            duration_us(report.stats.min()),
            duration_us(report.stats.average()),
            duration_us(report.stats.percentile_nearest(95)),
            duration_us(report.stats.max()),
            report.threshold.as_micros(),
            report.recommendation
        );
        return Ok(true);
    }

    Ok(false)
}

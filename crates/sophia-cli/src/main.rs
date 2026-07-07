use sophia_protocol::Size;
use sophia_runtime::{TraceLevel, init_tracing};
use sophia_x_bridge::{TestClientConfig, run_test_client_window, smoke_readback_display};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let verbose = args.iter().any(|arg| arg == "-v" || arg == "--verbose");
    let level = if verbose {
        TraceLevel::Debug
    } else {
        TraceLevel::Info
    };

    init_tracing(level)?;

    if args.iter().any(|arg| arg == "x-test-client") {
        let config = TestClientConfig {
            display_name: arg_value(&args, "--display"),
            size: Size {
                width: arg_value(&args, "--width")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(320),
                height: arg_value(&args, "--height")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(200),
            },
            hold_millis: arg_value(&args, "--seconds")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(5)
                .saturating_mul(1_000),
        };
        let window = run_test_client_window(config)?;
        println!(
            "x-test-client window={:#x} size={}x{}",
            window.window.xid(),
            window.size.width,
            window.size.height
        );
        return Ok(());
    }

    if args.iter().any(|arg| arg == "x-smoke-readback") {
        let display = arg_value(&args, "--display");
        let report = smoke_readback_display(display.as_deref())?;
        println!(
            "x-smoke-readback display={} windows={} surfaces={} targets={} readbacks={} bytes={}",
            report.display_name.as_deref().unwrap_or("<default>"),
            report.mirrored_windows,
            report.surfaces,
            report.redirect_targets,
            report.readbacks,
            report.total_bytes
        );
        return Ok(());
    }

    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-bridge, protocol, wm-demo");
    println!("commands: x-test-client [--display=:99] [--seconds=5] [--width=320] [--height=200]");
    println!("commands: x-smoke-readback [--display=:99]");

    if verbose {
        tracing::debug!("verbose tracing enabled");
        println!("logging: tracing subscriber initialized");
    }

    Ok(())
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

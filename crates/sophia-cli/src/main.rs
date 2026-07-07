use sophia_runtime::{TraceLevel, init_tracing};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let verbose = std::env::args().any(|arg| arg == "-v" || arg == "--verbose");
    let level = if verbose {
        TraceLevel::Debug
    } else {
        TraceLevel::Info
    };

    init_tracing(level)?;

    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-bridge, protocol, wm-demo");

    if verbose {
        tracing::debug!("verbose tracing enabled");
        println!("logging: tracing subscriber initialized");
    }

    Ok(())
}

pub(crate) fn print(verbose: bool) {
    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-bridge, protocol, wm-demo");
    println!("commands: x-test-client [--display=:99] [--seconds=5] [--width=320] [--height=200]");
    println!("commands: x-smoke-readback [--display=:99]");
    println!("commands: x-smoke-frame [--display=:99]");
    println!("commands: x-smoke-policy-frame [--display=:99]");
    println!("commands: x-smoke-runtime-tick [--display=:99]");
    println!("commands: runtime-damage-epoch-smoke");
    println!("commands: headless-session-driver-smoke");
    println!("commands: runtime-brokers-smoke [--portal=/usr/bin/true] [--metadata=/usr/bin/true]");
    println!("commands: portal-broker-health-smoke");
    println!("commands: metadata-broker-health-smoke");
    println!("commands: x-smoke-external-wm [--display=:99] [--wm=target/debug/sophia-wm-demo]");
    println!(
        "commands: x-smoke-live-runtime-wm-socket [--display=:99] [--wm=target/debug/sophia-wm-demo]"
    );
    println!("commands: wm-supervisor-smoke [--wm=target/debug/sophia-wm-demo]");
    println!("commands: portal-clipboard-deny-smoke");
    println!("commands: portal-clipboard-request-smoke");
    println!("commands: portal-clipboard-handoff-smoke");
    println!("commands: x-smoke-live-clipboard-portal [--display=:99]");
    println!("commands: x-smoke-routed-input [--display=:99]");
    println!("commands: x-smoke-routed-input-edges");
    println!(
        "commands: x-stress-routed-input [--display=:99] [--iterations=1000] [--threshold-us=500]"
    );
    println!("commands: x-authority-runtime-smoke");

    if verbose {
        tracing::debug!("verbose tracing enabled");
        println!("logging: tracing subscriber initialized");
    }
}

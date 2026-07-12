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
    println!("commands: x-authority-x11-smoke");
    println!("commands: x-authority-x11rb-smoke");
    println!("commands: x-authority-xdpyinfo-smoke");
    println!("commands: x-authority-xlib-smoke");
    println!("commands: x-authority-xlib-drawing-smoke");
    println!("commands: x-authority-xlib-put-image-smoke");
    println!("commands: x-authority-xclock-smoke");
    println!("commands: x-authority-xeyes-smoke");
    println!("commands: x-authority-xwininfo-root-smoke");
    println!("commands: x-authority-xprop-root-smoke");
    println!("commands: x-authority-xsetroot-name-smoke");
    println!("commands: x-authority-xlogo-smoke");
    println!("commands: x-authority-xmessage-smoke");
    println!("commands: x-authority-xrandr-query-smoke");
    println!("commands: x-authority-xcalc-smoke");
    println!("commands: x-authority-xterm-smoke");
    println!("commands: x-authority-xterm-render-smoke");
    println!("commands: x-authority-xterm-input-smoke");
    println!("commands: x-authority-zenity-smoke");
    println!("commands: x-authority-present-pixmap-smoke");
    #[cfg(feature = "atomic-scanout-live")]
    println!(
        "commands: sophia-live-session [--client-backend=sophia-x|xlibre-compat] [--client=PATH] [--client-arg=ARG ...] [--compat-display=:178] [--display=:77] [--terminal=xterm] [--terminal-exec=PATH] [--terminal-exec-arg=ARG ...] [--input-devices=/dev/input/eventN,...] [--native-scanout] [--wm-process=PATH] [--wm-process-arg=ARG ...] [--max-runtime-ms=N] [--max-ticks=N] [--inject-text=lowercase|--expect-physical-text=lowercase] [--expect-physical-pointer] [--exit-after-input-proof] [--proof]"
    );
    #[cfg(feature = "atomic-scanout-live")]
    println!("commands: live-session-composition-smoke");
    #[cfg(feature = "atomic-scanout-live")]
    println!("commands: atomic-scanout-preflight");
    #[cfg(feature = "atomic-scanout-smoke-live")]
    println!("commands: atomic-vrr-inspect");
    #[cfg(feature = "atomic-scanout-smoke-live")]
    println!(
        "commands: sophia-live-session-content-hardware-proof [--terminal=xterm] [--slot=1] [--output=1] [--authority=1] [--page-flip-timeout-ms=8000]"
    );
    #[cfg(feature = "atomic-scanout-smoke-live")]
    println!(
        "commands: atomic-scanout-smoke [--slot=1] [--output=1] [--authority=1] [--page-flip-timeout-ms=8000] [--child-timeout-ms=30000]"
    );
    #[cfg(feature = "atomic-scanout-smoke-live")]
    println!(
        "commands: atomic-vrr-smoke [--slot=1] [--output=1] [--authority=1] [--page-flip-timeout-ms=8000] [--child-timeout-ms=30000]"
    );
    #[cfg(feature = "atomic-scanout-smoke-live")]
    println!(
        "commands: atomic-scanout-runtime-evidence [--slot=1] [--output=1] [--authority=1] [--page-flip-timeout-ms=8000]"
    );

    if verbose {
        tracing::debug!("verbose tracing enabled");
        println!("logging: tracing subscriber initialized");
    }
}

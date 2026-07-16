pub(crate) fn print(verbose: bool) {
    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-authority, wayland-authority, protocol, wm-demo");
    println!("commands: runtime-damage-epoch-smoke");
    println!("commands: headless-session-driver-smoke");
    println!("commands: runtime-brokers-smoke [--portal=/usr/bin/true] [--metadata=/usr/bin/true]");
    println!("commands: portal-broker-health-smoke");
    println!("commands: metadata-broker-health-smoke");
    println!("commands: wm-supervisor-smoke [--wm=target/debug/sophia-wm-demo]");
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
    println!("commands: x-authority-xterm-two-client-smoke");
    println!("commands: x-authority-zenity-smoke");
    println!("commands: x-authority-present-pixmap-smoke");
    #[cfg(feature = "atomic-scanout-live")]
    println!(
        "commands: sophia-live-session [--client-backend=wayland|sophia-x] [--client=PATH] [--client-arg=ARG ...] [--display=:77] [--terminal=xterm] [--terminal-exec=PATH] [--terminal-exec-arg=ARG ...] [--secondary-terminal] [--namespace-profile=classic|confined] [--input-devices=/dev/input/eventN,...] [--native-scanout] [--wm-process=PATH] [--wm-process-arg=ARG ...] [--max-runtime-ms=N] [--max-ticks=N] [--inject-text=lowercase|--expect-physical-text=lowercase] [--expect-physical-pointer] [--exit-after-input-proof] [--proof]"
    );
    #[cfg(feature = "atomic-scanout-live")]
    println!(
        "commands: native-egl-vkcube-mixed-smoke [--display=:184] [--terminal=xterm] [--max-runtime-ms=6000]"
    );
    println!(
        "commands: sophia-wayland-session --client=PATH [--client-arg=ARG ...] [--wayland-display=sophia-0] [--input-devices=/dev/input/eventN,...] [--native-scanout] [--experimental-dmabuf] [--resize=WIDTHxHEIGHT] [--resize-after-ms=N] [--expect-keycodes=CODE,...] [--expect-pointer-input] [--expect-input-presentation|--expect-input-pixel-change] [--max-input-latency-ms=100] [--max-runtime-ms=N]"
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

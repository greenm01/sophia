fn main() {
    let verbose = std::env::args().any(|arg| arg == "-v" || arg == "--verbose");

    println!("sophia {}", env!("CARGO_PKG_VERSION"));
    println!("components: engine, x-bridge, protocol, wm-demo");

    if verbose {
        println!("logging: stderr placeholder; tracing crate not wired yet");
    }
}

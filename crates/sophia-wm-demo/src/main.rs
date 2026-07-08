fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("serve-socket") {
        let socket = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--socket="))
            .ok_or("missing --socket=PATH")?;
        sophia_wm_demo::run_socket_server(socket)?;
        return Ok(());
    }

    let response = sophia_wm_demo::run_process_request(&args)?;
    print!("{response}");
    Ok(())
}

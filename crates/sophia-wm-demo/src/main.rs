fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let response = sophia_wm_demo::run_process_request(&args)?;
    print!("{response}");
    Ok(())
}

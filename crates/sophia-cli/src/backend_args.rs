use std::time::Duration;

pub const ATOMIC_SCANOUT_SMOKE_CHILD_TIMEOUT_MS: u64 = 30_000;

pub fn atomic_scanout_smoke_child_args(args: &[String]) -> Vec<String> {
    [
        "--slot",
        "--output",
        "--authority",
        "--page-flip-timeout-ms",
    ]
    .into_iter()
    .filter_map(|key| arg_value(args, key).map(|value| format!("{key}={value}")))
    .collect()
}

pub fn atomic_scanout_smoke_child_timeout(
    args: &[String],
) -> Result<Duration, Box<dyn std::error::Error>> {
    let timeout_ms = arg_value(args, "--child-timeout-ms")
        .as_deref()
        .map(parse_u64)
        .transpose()?
        .unwrap_or(ATOMIC_SCANOUT_SMOKE_CHILD_TIMEOUT_MS);
    if timeout_ms == 0 {
        return Err("atomic scanout smoke child timeout must be nonzero".into());
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn arg_value(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix).map(str::to_owned))
}

fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid u64 value {value:?}: {error}").into())
}

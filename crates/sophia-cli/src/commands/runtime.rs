use super::prelude::*;

mod brokers;
mod session;
mod wm;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if session::try_run(args)? {
        return Ok(true);
    }
    if brokers::try_run(args)? {
        return Ok(true);
    }
    wm::try_run(args)
}

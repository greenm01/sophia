use super::prelude::*;

mod local;
mod wm;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if local::try_run(args)? {
        return Ok(true);
    }
    wm::try_run(args)
}

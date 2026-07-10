#[cfg(feature = "libinput-events")]
mod libinput;

#[cfg(feature = "libinput-events")]
pub use libinput::*;

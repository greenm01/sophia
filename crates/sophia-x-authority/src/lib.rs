//! Passive Sophia X Authority resource model.
//!
//! This crate intentionally starts without a live socket parser. It models the
//! authority-owned tables that later X protocol dispatch will mutate.

mod clipboard;
mod codec;
mod drawing;
mod event;
mod packet;
mod resource;
mod runtime;
mod selection;
mod socket;
mod window;

pub use clipboard::*;
pub use codec::*;
pub use drawing::*;
pub use event::*;
pub use packet::*;
pub use resource::*;
pub use runtime::*;
pub use selection::*;
pub use socket::*;
pub use window::*;

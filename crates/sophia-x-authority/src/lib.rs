//! Passive Sophia X Authority resource model.
//!
//! This crate intentionally starts without a live socket parser. It models the
//! authority-owned tables that later X protocol dispatch will mutate.

mod clipboard;
mod drawing;
mod event;
mod resource;
mod selection;
mod window;

pub use clipboard::*;
pub use drawing::*;
pub use event::*;
pub use resource::*;
pub use selection::*;
pub use window::*;

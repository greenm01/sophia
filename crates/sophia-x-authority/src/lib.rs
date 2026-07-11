//! Passive Sophia X Authority resource model.
//!
//! This crate intentionally starts without a live socket parser. It models the
//! authority-owned tables that later X protocol dispatch will mutate.

mod atom;
mod client_output;
mod clipboard;
mod codec;
mod dispatch;
mod drawing;
mod event;
mod keyboard;
mod packet;
mod property;
mod resource;
mod runtime;
mod selection;
mod setup;
mod shm;
mod socket;
mod software;
mod transport;
mod window;
mod wire;
mod x11_socket;

pub use atom::*;
pub use client_output::*;
pub use clipboard::*;
pub use codec::*;
pub use dispatch::*;
pub use drawing::*;
pub use event::*;
pub use keyboard::*;
pub use packet::*;
pub use property::*;
pub use resource::*;
pub use runtime::*;
pub use selection::*;
pub use setup::*;
pub use shm::*;
pub use socket::*;
pub use software::*;
pub use transport::*;
pub use window::*;
pub use wire::*;
pub use x11_socket::*;

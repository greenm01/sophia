//! Sophia X Server Frontend implementation seed.
//!
//! This crate terminates a bounded, modern X11 subset and translates its
//! authority-owned resource state into Sophia transactions. It does not own
//! physical input, compositor policy, rendering, or DRM/KMS. The current crate
//! name remains `sophia-x-authority` while the source layout matures.

mod atom;
mod client_output;
mod clipboard;
mod codec;
mod dispatch;
mod drawing;
mod event;
mod graphics_context;
mod keyboard;
mod packet;
mod pointer;
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
pub use graphics_context::*;
pub use keyboard::*;
pub use packet::*;
pub use pointer::*;
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

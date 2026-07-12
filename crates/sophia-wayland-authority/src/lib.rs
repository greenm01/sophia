//! Sophia-owned Wayland protocol authority.
//!
//! Smithay is used only for Wayland frontend object machinery. This crate owns
//! protocol resources and reduces them into protocol-neutral Sophia packets;
//! it does not own scene policy, rendering, physical input, or KMS state.

mod frontend;
mod reducer;

pub use frontend::*;
pub use reducer::*;

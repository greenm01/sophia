//! Passive data shared between Sophia processes.
//!
//! This crate deliberately has no compositor, X11, or IPC dependencies. It is
//! the executable form of the data model in `docs/dod.md`.

pub mod geometry;
pub mod ids;
pub mod ipc;
pub mod packets;
pub mod table;

pub use geometry::*;
pub use ids::*;
pub use ipc::*;
pub use packets::*;
pub use table::*;

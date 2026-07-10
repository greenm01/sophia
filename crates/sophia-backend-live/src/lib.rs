//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

mod api;
mod dependency;
mod drm;
mod hardware_validation;
mod input;
mod prelude;
mod runtime;
mod scanout;
mod session_loop;
mod startup;

pub use api::*;

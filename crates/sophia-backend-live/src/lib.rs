//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

#[cfg(feature = "libdrm-events")]
mod native_atomic;
#[cfg(feature = "libdrm-events")]
mod native_kms;
#[cfg(feature = "libdrm-events")]
mod native_page_flip;
#[cfg(feature = "libdrm-events")]
mod native_primary_plane;
#[cfg(feature = "libdrm-events")]
mod native_scanout;
mod page_flip;
mod prelude;
#[cfg(feature = "libdrm-events")]
mod rendered_scanout;

mod api;
mod dependency;
mod hardware_validation;
#[cfg(feature = "libinput-events")]
mod libinput;
mod runtime;
mod scanout_status;
mod startup;

pub use api::*;

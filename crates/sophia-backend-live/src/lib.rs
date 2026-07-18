//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.
//!
//! Keep this file as the crate boundary. Backend code belongs in domain modules:
//! input capture, DRM/KMS discovery, scanout, runtime assembly, session loop,
//! startup probing, and hardware validation.

mod api;
mod dependency;
mod drm;
mod hardware_validation;
mod input;
mod prelude;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
mod presentation;
mod production_cpu_cycle;
mod production_intake;
mod production_session;
mod runtime;
mod scanout;
mod session_loop;
mod startup;

pub use api::*;
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub use presentation::*;
pub use production_cpu_cycle::*;
pub use production_intake::*;
pub use production_session::*;
pub use sophia_renderer_live::LivePresentationDisconnectReport;

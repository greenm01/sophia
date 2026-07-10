#[cfg(feature = "libdrm-events")]
mod cleanup;
#[cfg(feature = "libdrm-events")]
mod failure;
#[cfg(feature = "libdrm-events")]
mod retire;
#[cfg(feature = "libdrm-events")]
mod submit;

#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub use cleanup::*;
#[cfg(feature = "libdrm-events")]
pub use failure::*;
#[cfg(feature = "libdrm-events")]
pub use retire::*;
#[cfg(feature = "libdrm-events")]
pub use submit::*;

#[cfg(feature = "libdrm-events")]
fn reduced_status<T: std::fmt::Debug>(status: Option<T>) -> String {
    status
        .map(|status| format!("{status:?}"))
        .unwrap_or_else(|| "none".to_owned())
}

#[cfg(feature = "libdrm-events")]
fn reduced_size(size: Option<Size>) -> String {
    size.map(|size| format!("{}x{}", size.width, size.height))
        .unwrap_or_else(|| "none".to_owned())
}

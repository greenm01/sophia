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

#[cfg(feature = "libdrm-events")]
pub use native_atomic::*;
#[cfg(feature = "libdrm-events")]
pub use native_kms::*;
#[cfg(feature = "libdrm-events")]
pub use native_page_flip::*;
#[cfg(feature = "libdrm-events")]
pub use native_primary_plane::*;
#[cfg(feature = "libdrm-events")]
pub use native_scanout::*;

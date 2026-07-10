mod config;
mod scanout;
mod smoke;

pub use scanout::*;
pub use smoke::*;

const EGL_PLATFORM_GBM_KHR: khronos_egl::Enum = 0x31D7;

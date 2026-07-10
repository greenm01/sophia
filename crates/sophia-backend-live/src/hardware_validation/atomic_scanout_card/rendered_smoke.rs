mod config;
mod phase;
mod runner;

pub use config::RealAtomicScanoutSmokeConfig;
pub use runner::{run_real_atomic_scanout_smoke_phases, run_real_atomic_scanout_smoke_phases_with};

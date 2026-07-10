use crate::prelude::*;
use std::path::PathBuf;

use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};

pub fn discover_live_backend(config: &LiveBackendConfig) -> LiveBackendStartupReport {
    let output_backend = SysfsDrmKmsOutputBackend::new(&config.drm_sysfs_root);
    let input_backend = StaticInputDiscoveryBackend::new(config.input_devices.clone());

    LiveBackendStartupReport {
        discovery: discover_live_compositor_backend(&output_backend, &input_backend),
        renderer_import: config.renderer_import,
        renderer_preference: config.renderer_preference,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveBackendConfig {
    pub drm_sysfs_root: PathBuf,
    pub input_devices: Vec<LibinputDeviceDescriptor>,
    pub renderer_import: LiveRendererImportBoundary,
    pub renderer_preference: LiveRendererPreference,
}

impl LiveBackendConfig {
    pub fn new(drm_sysfs_root: impl Into<PathBuf>) -> Self {
        Self {
            drm_sysfs_root: drm_sysfs_root.into(),
            input_devices: Vec::new(),
            renderer_import: LiveRendererImportBoundary::cpu_only(),
            renderer_preference: LiveRendererPreference::default(),
        }
    }

    pub fn with_input_device(mut self, device: LibinputDeviceDescriptor) -> Self {
        self.input_devices.push(device);
        self
    }

    pub fn with_renderer_import_boundary(
        mut self,
        renderer_import: LiveRendererImportBoundary,
    ) -> Self {
        self.renderer_import = renderer_import;
        self
    }

    pub fn with_renderer_preference(mut self, renderer_preference: LiveRendererPreference) -> Self {
        self.renderer_preference = renderer_preference;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveRendererPreference {
    #[default]
    GpuPreferred,
    CpuOnly,
    GpuRequired,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendStartupReport {
    pub discovery: LiveCompositorBackendDiscoveryReport,
    pub renderer_import: LiveRendererImportBoundary,
    pub renderer_preference: LiveRendererPreference,
}

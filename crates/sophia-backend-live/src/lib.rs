//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

use std::path::PathBuf;

pub use sophia_engine::{
    DrmKmsOutputRegistry, HeadlessCompositorBackendAssembly, HeadlessOutput,
    LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventSource,
    LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus, QueuedInputPoller,
    RendererSelection,
};
use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};
pub use sophia_protocol::{DeviceId, OutputId, SeatId, Size};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveBackendConfig {
    pub drm_sysfs_root: PathBuf,
    pub input_devices: Vec<LibinputDeviceDescriptor>,
}

impl LiveBackendConfig {
    pub fn new(drm_sysfs_root: impl Into<PathBuf>) -> Self {
        Self {
            drm_sysfs_root: drm_sysfs_root.into(),
            input_devices: Vec::new(),
        }
    }

    pub fn with_input_device(mut self, device: LibinputDeviceDescriptor) -> Self {
        self.input_devices.push(device);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendStartupReport {
    pub discovery: LiveCompositorBackendDiscoveryReport,
}

impl LiveBackendStartupReport {
    pub fn status(&self) -> &LiveCompositorBackendDiscoveryStatus {
        &self.discovery.status
    }

    pub fn selected_output(&self) -> Option<HeadlessOutput> {
        self.discovery.selected_output
    }

    pub fn into_headless_assembly(
        self,
        poller: QueuedInputPoller,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        self.discovery.into_headless_assembly(poller, renderer)
    }
}

pub fn discover_live_backend(config: &LiveBackendConfig) -> LiveBackendStartupReport {
    let output_backend = SysfsDrmKmsOutputBackend::new(&config.drm_sysfs_root);
    let input_backend = StaticInputDiscoveryBackend::new(config.input_devices.clone());

    LiveBackendStartupReport {
        discovery: discover_live_compositor_backend(&output_backend, &input_backend),
    }
}

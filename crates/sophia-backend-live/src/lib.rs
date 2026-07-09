//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

use std::path::PathBuf;

pub use sophia_engine::{
    BufferImportPath, CompositorBackendAssemblyError, CompositorBackendTickInput,
    CompositorBackendTickReport, DrmKmsOutputRegistry, HeadlessCompositorBackendAssembly,
    HeadlessOutput, LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventSource,
    LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus, QueuedInputPoller,
    RendererSelection,
};
use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};
pub use sophia_protocol::{BufferSource, DeviceId, OutputId, SeatId, Size};
pub use sophia_renderer_live::{
    LiveRendererImportBoundary, LiveRendererImportDecision, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportRejection, LiveRendererImportStartupStatus,
    LiveRendererRuntimeObservation, LiveRendererSelectionObservation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyKind {
    LibDrm,
    LibInput,
    Gbm,
    Egl,
    DmaBuf,
    MitShm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyUse {
    Discovery,
    RuntimePolling,
    RendererImport,
    SharedMemoryImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyDecision {
    Allowed,
    Deferred { required_boundary: &'static str },
}

impl LiveBackendDependencyDecision {
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

pub fn live_backend_dependency_decision(
    kind: LiveBackendDependencyKind,
    use_case: LiveBackendDependencyUse,
) -> LiveBackendDependencyDecision {
    use LiveBackendDependencyDecision::{Allowed, Deferred};
    use LiveBackendDependencyKind::{DmaBuf, Egl, Gbm, LibDrm, LibInput, MitShm};
    use LiveBackendDependencyUse::{Discovery, RendererImport, RuntimePolling, SharedMemoryImport};

    match (kind, use_case) {
        (LibDrm | LibInput, Discovery | RuntimePolling) => Allowed,
        (MitShm, _) | (_, SharedMemoryImport) => Deferred {
            required_boundary: "bounded shared-memory import boundary",
        },
        (Gbm | Egl | DmaBuf, _) | (_, RendererImport) => Deferred {
            required_boundary: "live renderer import boundary",
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveBackendConfig {
    pub drm_sysfs_root: PathBuf,
    pub input_devices: Vec<LibinputDeviceDescriptor>,
    pub renderer_import: LiveRendererImportBoundary,
}

impl LiveBackendConfig {
    pub fn new(drm_sysfs_root: impl Into<PathBuf>) -> Self {
        Self {
            drm_sysfs_root: drm_sysfs_root.into(),
            input_devices: Vec::new(),
            renderer_import: LiveRendererImportBoundary::cpu_only(),
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendStartupReport {
    pub discovery: LiveCompositorBackendDiscoveryReport,
    pub renderer_import: LiveRendererImportBoundary,
}

impl LiveBackendStartupReport {
    pub fn status(&self) -> &LiveCompositorBackendDiscoveryStatus {
        &self.discovery.status
    }

    pub fn selected_output(&self) -> Option<HeadlessOutput> {
        self.discovery.selected_output
    }

    pub fn renderer_selection(&self) -> RendererSelection {
        if self.renderer_import.import_xpixmap || self.renderer_import.import_dmabuf {
            RendererSelection::ImportCapable {
                import_xpixmap: self.renderer_import.import_xpixmap,
                import_dmabuf: self.renderer_import.import_dmabuf,
            }
        } else {
            RendererSelection::CpuFallback
        }
    }

    pub fn renderer_import_status(&self) -> LiveRendererImportStartupStatus {
        self.renderer_import.startup_status()
    }

    pub fn into_configured_headless_assembly(
        self,
        poller: QueuedInputPoller,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        let renderer = self.renderer_selection();
        self.into_headless_assembly(poller, renderer)
    }

    pub fn into_live_runtime_assembly(
        self,
        poller: QueuedInputPoller,
    ) -> Option<LiveBackendRuntimeAssembly> {
        let renderer_status = self.renderer_import_status();
        let renderer_selection = self.renderer_selection();
        let renderer_observation = LiveRendererRuntimeObservation::from_startup_status(
            renderer_status,
            selection_observation(renderer_selection),
        );
        self.into_headless_assembly(poller, renderer_selection)
            .map(|assembly| LiveBackendRuntimeAssembly {
                assembly,
                renderer_observation,
            })
    }

    pub fn into_headless_assembly(
        self,
        poller: QueuedInputPoller,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        self.discovery.into_headless_assembly(poller, renderer)
    }
}

pub struct LiveBackendRuntimeAssembly {
    assembly: HeadlessCompositorBackendAssembly,
    renderer_observation: LiveRendererRuntimeObservation,
}

impl LiveBackendRuntimeAssembly {
    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
    }

    pub fn run_tick(
        &mut self,
        input: CompositorBackendTickInput,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError> {
        let engine = self.assembly.run_tick(input)?;

        Ok(LiveBackendRuntimeTickReport {
            engine,
            renderer: self.renderer_observation,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeTickReport {
    pub engine: CompositorBackendTickReport,
    pub renderer: LiveRendererRuntimeObservation,
}

pub fn discover_live_backend(config: &LiveBackendConfig) -> LiveBackendStartupReport {
    let output_backend = SysfsDrmKmsOutputBackend::new(&config.drm_sysfs_root);
    let input_backend = StaticInputDiscoveryBackend::new(config.input_devices.clone());

    LiveBackendStartupReport {
        discovery: discover_live_compositor_backend(&output_backend, &input_backend),
        renderer_import: config.renderer_import,
    }
}

fn selection_observation(selection: RendererSelection) -> LiveRendererSelectionObservation {
    match selection {
        RendererSelection::CpuFallback => LiveRendererSelectionObservation::CpuFallback,
        RendererSelection::ImportCapable { .. } => {
            LiveRendererSelectionObservation::NativeImportCapable
        }
    }
}

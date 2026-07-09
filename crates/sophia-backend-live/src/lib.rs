//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

use std::path::PathBuf;

pub use sophia_engine::{
    BufferImportPath, DrmKmsOutputRegistry, HeadlessCompositorBackendAssembly, HeadlessOutput,
    LibinputDeviceDescriptor, LibinputDeviceKind, LibinputEventSource,
    LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus, QueuedInputPoller,
    RendererSelection,
};
use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};
pub use sophia_protocol::{BufferSource, DeviceId, OutputId, SeatId, Size};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererImportBoundary {
    pub import_xpixmap: bool,
    pub import_dmabuf: bool,
}

impl LiveRendererImportBoundary {
    pub const fn cpu_only() -> Self {
        Self {
            import_xpixmap: false,
            import_dmabuf: false,
        }
    }

    pub const fn with_native_imports(import_xpixmap: bool, import_dmabuf: bool) -> Self {
        Self {
            import_xpixmap,
            import_dmabuf,
        }
    }

    pub fn decide(self, source: BufferSource) -> LiveRendererImportDecision {
        match source {
            BufferSource::None => LiveRendererImportDecision::Rejected {
                reason: LiveRendererImportRejection::EmptySource,
            },
            BufferSource::CpuBuffer { .. } => LiveRendererImportDecision::Accepted {
                path: BufferImportPath::CpuReadback,
            },
            BufferSource::XPixmap { .. } if self.import_xpixmap => {
                LiveRendererImportDecision::Accepted {
                    path: BufferImportPath::XPixmap,
                }
            }
            BufferSource::DmaBuf { .. } if self.import_dmabuf => {
                LiveRendererImportDecision::Accepted {
                    path: BufferImportPath::DmaBuf,
                }
            }
            BufferSource::XPixmap { .. } => LiveRendererImportDecision::Deferred {
                requested: BufferImportPath::XPixmap,
                required_boundary: "live XPixmap renderer import",
            },
            BufferSource::DmaBuf { .. } => LiveRendererImportDecision::Deferred {
                requested: BufferImportPath::DmaBuf,
                required_boundary: "live DMA-BUF renderer import",
            },
        }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        LiveRendererImportStartupStatus {
            health: if self.import_xpixmap || self.import_dmabuf {
                LiveRendererImportHealth::NativeImportCapable
            } else {
                LiveRendererImportHealth::CpuFallback
            },
            xpixmap: if self.import_xpixmap {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Disabled
            },
            dmabuf: if self.import_dmabuf {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Disabled
            },
        }
    }
}

impl Default for LiveRendererImportBoundary {
    fn default() -> Self {
        Self::cpu_only()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportDecision {
    Accepted {
        path: BufferImportPath,
    },
    Deferred {
        requested: BufferImportPath,
        required_boundary: &'static str,
    },
    Rejected {
        reason: LiveRendererImportRejection,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportRejection {
    EmptySource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererImportStartupStatus {
    pub health: LiveRendererImportHealth,
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportHealth {
    CpuFallback,
    NativeImportCapable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportPathStatus {
    Disabled,
    Enabled,
    Degraded,
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
        renderer_import: config.renderer_import,
    }
}

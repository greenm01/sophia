//! Live compositor backend boundary.
//!
//! This crate is where real kernel-facing dependencies belong. The current
//! implementation deliberately stays on deterministic engine traits: sysfs-style
//! DRM/KMS discovery and static input descriptors. Future libdrm/libinput code
//! can replace these adapters without changing Sophia Engine, WM IPC, or
//! protocol authority packets.

use std::path::PathBuf;
#[cfg(feature = "gbm-probe")]
use std::{io, os::fd::AsFd};

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
#[cfg(feature = "gbm-probe")]
pub use sophia_renderer_live::NativeGbmCapabilityProbe;
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

impl LiveBackendStartupReport {
    pub fn status(&self) -> &LiveCompositorBackendDiscoveryStatus {
        &self.discovery.status
    }

    pub fn selected_output(&self) -> Option<HeadlessOutput> {
        self.discovery.selected_output
    }

    pub fn renderer_selection(&self) -> RendererSelection {
        self.try_renderer_selection()
            .unwrap_or(RendererSelection::CpuFallback)
    }

    pub fn try_renderer_selection(&self) -> Option<RendererSelection> {
        self.renderer_selection_for_status(self.renderer_import_status())
    }

    pub fn renderer_selection_for_status(
        &self,
        status: LiveRendererImportStartupStatus,
    ) -> Option<RendererSelection> {
        match self.renderer_preference {
            LiveRendererPreference::CpuOnly => Some(RendererSelection::CpuFallback),
            LiveRendererPreference::GpuPreferred => {
                Some(selection_from_native_status(status).unwrap_or(RendererSelection::CpuFallback))
            }
            LiveRendererPreference::GpuRequired => selection_from_native_status(status),
        }
    }

    pub fn renderer_runtime_status_for_preference(
        &self,
        status: LiveRendererImportStartupStatus,
    ) -> LiveRendererImportStartupStatus {
        match self.renderer_preference {
            LiveRendererPreference::CpuOnly => cpu_fallback_renderer_status(),
            LiveRendererPreference::GpuPreferred | LiveRendererPreference::GpuRequired => status,
        }
    }

    pub fn renderer_import_status(&self) -> LiveRendererImportStartupStatus {
        self.renderer_import.startup_status()
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_import_status_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveRendererImportStartupStatus
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.renderer_probe_report_with_gbm_device(discovery)
            .renderer_import
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_selection_with_gbm_device<D>(&self, discovery: &D) -> Option<RendererSelection>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.renderer_selection_for_status(self.renderer_import_status_with_gbm_device(discovery))
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_probe_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveBackendRendererProbeReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        if self.renderer_preference == LiveRendererPreference::CpuOnly
            || !self.renderer_import.import_dmabuf
        {
            return LiveBackendRendererProbeReport {
                render_device: LiveRenderDeviceDiscoveryReport {
                    status: LiveRenderDeviceDiscoveryStatus::NotRequested,
                },
                renderer_import: self
                    .renderer_runtime_status_for_preference(self.renderer_import_status()),
            };
        }

        let device = discovery.open_render_device();
        let render_device = LiveRenderDeviceDiscoveryReport::from_open_result(&device);
        let renderer_import = self.renderer_runtime_status_for_preference(
            self.renderer_import_status_from_gbm_device_result(device),
        );

        LiveBackendRendererProbeReport {
            render_device,
            renderer_import,
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub fn renderer_import_status_from_gbm_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveRendererImportStartupStatus {
        let configured = self.renderer_import_status();

        if !self.renderer_import.import_dmabuf {
            return configured;
        }

        let gbm_status =
            NativeGbmCapabilityProbe::startup_status_from_backend_device_result(device);
        LiveRendererImportStartupStatus::from_path_statuses(configured.xpixmap, gbm_status.dmabuf)
    }

    pub fn into_configured_headless_assembly(
        self,
        poller: QueuedInputPoller,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        let renderer = self.try_renderer_selection()?;
        self.into_headless_assembly(poller, renderer)
    }

    pub fn into_live_runtime_assembly(
        self,
        poller: QueuedInputPoller,
    ) -> Option<LiveBackendRuntimeAssembly> {
        let renderer_status =
            self.renderer_runtime_status_for_preference(self.renderer_import_status());
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_configured_headless_assembly_with_gbm_device<D>(
        self,
        poller: QueuedInputPoller,
        discovery: &D,
    ) -> Option<HeadlessCompositorBackendAssembly>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        let renderer = self.renderer_selection_for_status(renderer_status)?;
        self.into_headless_assembly(poller, renderer)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_live_runtime_assembly_with_gbm_device<D>(
        self,
        poller: QueuedInputPoller,
        discovery: &D,
    ) -> Option<LiveBackendRuntimeAssembly>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    pub fn into_headless_assembly(
        self,
        poller: QueuedInputPoller,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly> {
        self.discovery.into_headless_assembly(poller, renderer)
    }

    fn into_live_runtime_assembly_with_status(
        self,
        poller: QueuedInputPoller,
        renderer_status: LiveRendererImportStartupStatus,
    ) -> Option<LiveBackendRuntimeAssembly> {
        let renderer_selection = self.renderer_selection_for_status(renderer_status)?;
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
}

#[cfg(feature = "gbm-probe")]
pub trait RenderDeviceDiscoveryBackend {
    type Device: AsFd;

    fn open_render_device(&self) -> io::Result<Self::Device>;
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendRendererProbeReport {
    pub render_device: LiveRenderDeviceDiscoveryReport,
    pub renderer_import: LiveRendererImportStartupStatus,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRenderDeviceDiscoveryReport {
    pub status: LiveRenderDeviceDiscoveryStatus,
}

#[cfg(feature = "gbm-probe")]
impl LiveRenderDeviceDiscoveryReport {
    fn from_open_result<T>(device: &io::Result<T>) -> Self {
        Self {
            status: if device.is_ok() {
                LiveRenderDeviceDiscoveryStatus::Opened
            } else {
                LiveRenderDeviceDiscoveryStatus::Unavailable
            },
        }
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRenderDeviceDiscoveryStatus {
    NotRequested,
    Opened,
    Unavailable,
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
        renderer_preference: config.renderer_preference,
    }
}

fn selection_from_native_status(
    status: LiveRendererImportStartupStatus,
) -> Option<RendererSelection> {
    if status.health != LiveRendererImportHealth::NativeImportCapable {
        return None;
    }

    Some(RendererSelection::ImportCapable {
        import_xpixmap: status.xpixmap == LiveRendererImportPathStatus::Enabled,
        import_dmabuf: status.dmabuf == LiveRendererImportPathStatus::Enabled,
    })
}

fn cpu_fallback_renderer_status() -> LiveRendererImportStartupStatus {
    LiveRendererImportBoundary::cpu_only().startup_status()
}

fn selection_observation(selection: RendererSelection) -> LiveRendererSelectionObservation {
    match selection {
        RendererSelection::CpuFallback => LiveRendererSelectionObservation::CpuFallback,
        RendererSelection::ImportCapable { .. } => {
            LiveRendererSelectionObservation::NativeImportCapable
        }
    }
}

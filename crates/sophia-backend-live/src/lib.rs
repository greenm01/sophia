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
    LiveCompositorBackendDiscoveryReport, LiveCompositorBackendDiscoveryStatus,
    PageFlipCommitOutcome, QueuedInputPoller, RendererSelection,
};
use sophia_engine::{
    StaticInputDiscoveryBackend, SysfsDrmKmsOutputBackend, discover_live_compositor_backend,
};
pub use sophia_protocol::{BufferSource, DeviceId, OutputId, SeatId, Size};
#[cfg(feature = "gbm-probe")]
use sophia_renderer_live::GbmCapabilityProbeStatus;
#[cfg(feature = "egl-probe")]
use sophia_renderer_live::{
    EglCapabilityProbeStatus, FakeEglCapabilityProbe, NativeEglCapabilityProbe, NativeEglDrawSmoke,
};
#[cfg(feature = "egl-probe")]
pub use sophia_renderer_live::{EglContextProbeStatus, EglPlatformStatus};
#[cfg(feature = "egl-probe")]
pub use sophia_renderer_live::{EglDrawSmokeReport, EglDrawSmokeStatus};
#[cfg(feature = "gbm-probe")]
pub use sophia_renderer_live::{GbmCapabilityProbeReport, NativeGbmCapabilityProbe};
pub use sophia_renderer_live::{
    LiveRendererImportBoundary, LiveRendererImportDecision, LiveRendererImportHealth,
    LiveRendererImportPathStatus, LiveRendererImportRejection, LiveRendererImportStartupStatus,
    LiveRendererPresentationReport, LiveRendererPresentationStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation,
};
#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
use sophia_renderer_live::{
    NativeGbmBackedEglDrawSmoke, NativeGbmBackedEglPlatformProbe,
    NativeGbmBackedEglPresentationSmoke,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveScanoutReadinessReport {
    pub status: LiveScanoutReadinessStatus,
}

impl LiveScanoutReadinessReport {
    fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        if backend.selected_output().is_none() {
            return Self {
                status: LiveScanoutReadinessStatus::OutputUnavailable,
            };
        }

        Self {
            status: match presentation.status {
                LiveRendererPresentationStatus::Ready => LiveScanoutReadinessStatus::Ready,
                LiveRendererPresentationStatus::Unavailable => {
                    LiveScanoutReadinessStatus::PresentationUnavailable
                }
                LiveRendererPresentationStatus::Degraded => LiveScanoutReadinessStatus::Degraded,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveScanoutReadinessStatus {
    Ready,
    OutputUnavailable,
    PresentationUnavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePageFlipEvent {
    pub status: LivePageFlipEventStatus,
    pub frame_serial: Option<u64>,
}

impl LivePageFlipEvent {
    pub const fn from_scanout_status(status: LiveScanoutReadinessStatus) -> Self {
        Self {
            status: match status {
                LiveScanoutReadinessStatus::Ready => LivePageFlipEventStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable => {
                    LivePageFlipEventStatus::OutputUnavailable
                }
                LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LivePageFlipEventStatus::PresentationUnavailable
                }
                LiveScanoutReadinessStatus::Degraded => LivePageFlipEventStatus::Degraded,
            },
            frame_serial: None,
        }
    }

    pub fn from_commit_outcome(outcome: &PageFlipCommitOutcome) -> Self {
        match outcome {
            PageFlipCommitOutcome::Idle => Self {
                status: LivePageFlipEventStatus::Idle,
                frame_serial: None,
            },
            PageFlipCommitOutcome::WaitingForOutput { .. } => Self {
                status: LivePageFlipEventStatus::WaitingForOutput,
                frame_serial: None,
            },
            PageFlipCommitOutcome::WaitingForTransactionReadiness { .. } => Self {
                status: LivePageFlipEventStatus::WaitingForTransactionReadiness,
                frame_serial: None,
            },
            PageFlipCommitOutcome::Committed { frame_serial, .. } => Self {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(*frame_serial),
            },
            PageFlipCommitOutcome::Rejected { frame_serial, .. } => Self {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(*frame_serial),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePageFlipEventStatus {
    Ready,
    Idle,
    WaitingForOutput,
    WaitingForTransactionReadiness,
    Presented,
    Rejected,
    OutputUnavailable,
    PresentationUnavailable,
    Degraded,
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

    pub fn scanout_readiness_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveScanoutReadinessReport {
        LiveScanoutReadinessReport::from_backend_and_presentation(self, presentation)
    }

    #[cfg(feature = "egl-probe")]
    pub fn egl_probe_report(
        &self,
        platform: EglPlatformStatus,
        context: EglContextProbeStatus,
    ) -> LiveEglStartupReport {
        LiveEglStartupReport::from_probe_status(
            FakeEglCapabilityProbe::new(platform, context)
                .probe_report()
                .status,
        )
    }

    #[cfg(feature = "egl-probe")]
    pub fn native_egl_probe_report(&self) -> LiveEglStartupReport {
        LiveEglStartupReport::from_probe_status(NativeEglCapabilityProbe::probe_report().status)
    }

    #[cfg(feature = "egl-probe")]
    pub fn native_egl_draw_smoke_report(&self) -> EglDrawSmokeReport {
        NativeEglDrawSmoke::smoke_report()
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn gbm_backed_egl_platform_report(
        &self,
        gpu_startup: LiveGpuStartupReport,
    ) -> LiveGbmBackedEglPlatformReport {
        LiveGbmBackedEglPlatformReport::from_gpu_startup(gpu_startup)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_platform_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveGbmBackedEglPlatformReport {
        LiveGbmBackedEglPlatformReport {
            status: NativeGbmBackedEglPlatformProbe::platform_status_from_backend_device_result(
                device,
            ),
        }
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_platform_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveGbmBackedEglPlatformReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_platform_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_draw_smoke_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> EglDrawSmokeReport {
        NativeGbmBackedEglDrawSmoke::smoke_report_from_backend_device_result(device)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_draw_smoke_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> EglDrawSmokeReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_draw_smoke_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_presentation_smoke_report_from_device_result<T: AsFd>(
        &self,
        device: io::Result<T>,
    ) -> LiveRendererPresentationReport {
        NativeGbmBackedEglPresentationSmoke::smoke_report_from_backend_device_result(device)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn native_gbm_backed_egl_presentation_smoke_report_with_gbm_device<D>(
        &self,
        discovery: &D,
    ) -> LiveRendererPresentationReport
    where
        D: RenderDeviceDiscoveryBackend,
    {
        self.native_gbm_backed_egl_presentation_smoke_report_from_device_result(
            discovery.open_render_device(),
        )
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn egl_probe_report_from_gbm_startup(
        &self,
        gpu_startup: LiveGpuStartupReport,
        context: EglContextProbeStatus,
    ) -> LiveEglStartupReport {
        self.egl_probe_report(
            self.gbm_backed_egl_platform_report(gpu_startup).status,
            context,
        )
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
                gpu_startup: LiveGpuStartupReport::not_requested(),
                renderer_import: self
                    .renderer_runtime_status_for_preference(self.renderer_import_status()),
            };
        }

        let device = discovery.open_render_device();
        let render_device = LiveRenderDeviceDiscoveryReport::from_open_result(&device);
        let probe_report =
            NativeGbmCapabilityProbe::probe_report_from_backend_device_result(device);
        let renderer_import = self.renderer_runtime_status_for_preference(
            self.renderer_import_status_from_gbm_probe(probe_report),
        );
        let gpu_startup =
            LiveGpuStartupReport::from_discovery_and_probe(render_device, probe_report.status);

        LiveBackendRendererProbeReport {
            render_device,
            gpu_startup,
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

        self.renderer_import_status_from_gbm_probe(
            NativeGbmCapabilityProbe::probe_report_from_backend_device_result(device),
        )
    }

    #[cfg(feature = "gbm-probe")]
    fn renderer_import_status_from_gbm_probe(
        &self,
        probe_report: GbmCapabilityProbeReport,
    ) -> LiveRendererImportStartupStatus {
        let configured = self.renderer_import_status();

        if !self.renderer_import.import_dmabuf {
            return configured;
        }

        LiveRendererImportStartupStatus::from_path_statuses(
            configured.xpixmap,
            probe_report.startup_status.dmabuf,
        )
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
    pub gpu_startup: LiveGpuStartupReport,
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

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGpuStartupReport {
    pub status: LiveGpuStartupStatus,
}

#[cfg(feature = "gbm-probe")]
impl LiveGpuStartupReport {
    fn not_requested() -> Self {
        Self {
            status: LiveGpuStartupStatus::NotRequested,
        }
    }

    fn from_discovery_and_probe(
        discovery: LiveRenderDeviceDiscoveryReport,
        probe_status: GbmCapabilityProbeStatus,
    ) -> Self {
        if discovery.status != LiveRenderDeviceDiscoveryStatus::Opened {
            return Self {
                status: LiveGpuStartupStatus::RenderDeviceUnavailable,
            };
        }

        Self {
            status: match probe_status {
                GbmCapabilityProbeStatus::NativeCapable => LiveGpuStartupStatus::NativeCapable,
                GbmCapabilityProbeStatus::ReducedDeviceUnavailable => {
                    LiveGpuStartupStatus::RenderDeviceUnavailable
                }
                GbmCapabilityProbeStatus::NativeDeviceRejected => {
                    LiveGpuStartupStatus::GbmDeviceRejected
                }
                GbmCapabilityProbeStatus::PrivateAllocationUnavailable => {
                    LiveGpuStartupStatus::PrivateAllocationUnavailable
                }
            },
        }
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGpuStartupStatus {
    NotRequested,
    NativeCapable,
    RenderDeviceUnavailable,
    GbmDeviceRejected,
    PrivateAllocationUnavailable,
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl From<LiveGpuStartupStatus> for EglPlatformStatus {
    fn from(status: LiveGpuStartupStatus) -> Self {
        match status {
            LiveGpuStartupStatus::NativeCapable => EglPlatformStatus::NativePlatformCapable,
            LiveGpuStartupStatus::NotRequested | LiveGpuStartupStatus::RenderDeviceUnavailable => {
                EglPlatformStatus::PlatformUnavailable
            }
            LiveGpuStartupStatus::GbmDeviceRejected
            | LiveGpuStartupStatus::PrivateAllocationUnavailable => {
                EglPlatformStatus::PlatformDegraded
            }
        }
    }
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmBackedEglPlatformReport {
    pub status: EglPlatformStatus,
}

#[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
impl LiveGbmBackedEglPlatformReport {
    pub fn from_gpu_startup(gpu_startup: LiveGpuStartupReport) -> Self {
        Self {
            status: EglPlatformStatus::from(gpu_startup.status),
        }
    }
}

#[cfg(feature = "egl-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveEglStartupReport {
    pub status: LiveEglStartupStatus,
}

#[cfg(feature = "egl-probe")]
impl LiveEglStartupReport {
    fn from_probe_status(status: EglCapabilityProbeStatus) -> Self {
        Self {
            status: match status {
                EglCapabilityProbeStatus::NativeDrawingCapable => {
                    LiveEglStartupStatus::NativeDrawingCapable
                }
                EglCapabilityProbeStatus::PlatformUnavailable => {
                    LiveEglStartupStatus::PlatformUnavailable
                }
                EglCapabilityProbeStatus::PlatformDegraded => {
                    LiveEglStartupStatus::PlatformDegraded
                }
                EglCapabilityProbeStatus::ContextUnavailable => {
                    LiveEglStartupStatus::ContextUnavailable
                }
            },
        }
    }
}

#[cfg(feature = "egl-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveEglStartupStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
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

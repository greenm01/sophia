use crate::prelude::*;

mod config;
#[cfg(feature = "egl-probe")]
mod egl;
#[cfg(any(feature = "gbm-probe", feature = "egl-probe"))]
mod probe;
mod renderer;

pub use config::*;
#[cfg(any(feature = "gbm-probe", feature = "egl-probe"))]
pub use probe::*;

use renderer::selection_observation;

impl LiveBackendStartupReport {
    pub fn status(&self) -> &LiveCompositorBackendDiscoveryStatus {
        &self.discovery.status
    }

    pub fn selected_output(&self) -> Option<HeadlessOutput> {
        self.discovery.selected_output
    }

    pub fn scanout_readiness_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveScanoutReadinessReport {
        LiveScanoutReadinessReport::from_backend_and_presentation(self, presentation)
    }

    pub fn kms_scanout_target_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveKmsScanoutTargetReport {
        LiveKmsScanoutTargetReport::from_backend_and_presentation(self, presentation)
    }

    pub fn selected_gbm_egl_frame_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.selected_output()
            .map(|output| LiveGbmEglFrameTargetRecord::new(output.size))
    }

    #[cfg(feature = "libdrm-events")]
    pub fn native_libdrm_output_routes(&self) -> Vec<LibdrmNativeOutputRoute> {
        self.discovery
            .outputs
            .outputs()
            .enumerate()
            .filter_map(|(index, output)| {
                LibdrmNativeOutputSlot::new(
                    u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX),
                )
                .map(|slot| LibdrmNativeOutputRoute {
                    slot,
                    output: output.output,
                })
            })
            .collect()
    }

    #[cfg(feature = "libdrm-events")]
    pub fn native_libdrm_poller_startup_report(&self) -> LiveLibdrmPollerStartupReport {
        if !self.discovery.is_ready() {
            return LiveLibdrmPollerStartupReport {
                status: if self.discovery.selected_output.is_none() {
                    LiveLibdrmPollerStartupStatus::NoOutputs
                } else {
                    LiveLibdrmPollerStartupStatus::BackendNotReady
                },
                route_count: 0,
            };
        }

        let route_count = self.native_libdrm_output_routes().len();
        LiveLibdrmPollerStartupReport {
            status: if route_count == 0 {
                LiveLibdrmPollerStartupStatus::NoOutputs
            } else {
                LiveLibdrmPollerStartupStatus::Ready
            },
            route_count,
        }
    }

    #[cfg(feature = "libdrm-events")]
    pub fn native_libdrm_poller_from_authority(
        &self,
        authority: LibdrmBackendFdAuthority,
    ) -> Option<NativeLibdrmPageFlipEventPoller> {
        if !self.discovery.is_ready() {
            return None;
        }

        Some(
            NativeLibdrmPageFlipEventPoller::new(LibdrmNativePageFlipSource::from_authority(
                authority,
            ))
            .with_routes(self.native_libdrm_output_routes()),
        )
    }

    pub fn into_configured_headless_assembly<P>(
        self,
        poller: P,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let renderer = self.try_renderer_selection()?;
        self.into_headless_assembly(poller, renderer)
    }

    pub fn into_live_runtime_assembly<P>(self, poller: P) -> Option<LiveBackendRuntimeAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let renderer_status =
            self.renderer_runtime_status_for_preference(self.renderer_import_status());
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_configured_headless_assembly_with_gbm_device<P, D>(
        self,
        poller: P,
        discovery: &D,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        let renderer = self.renderer_selection_for_status(renderer_status)?;
        self.into_headless_assembly(poller, renderer)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn into_live_runtime_assembly_with_gbm_device<P, D>(
        self,
        poller: P,
        discovery: &D,
    ) -> Option<LiveBackendRuntimeAssembly<P>>
    where
        P: NonBlockingInputPoller,
        D: RenderDeviceDiscoveryBackend,
    {
        let renderer_status = self.renderer_import_status_with_gbm_device(discovery);
        self.into_live_runtime_assembly_with_status(poller, renderer_status)
    }

    pub fn into_headless_assembly<P>(
        self,
        poller: P,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        self.discovery.into_headless_assembly(poller, renderer)
    }

    fn into_live_runtime_assembly_with_status<P>(
        self,
        poller: P,
        renderer_status: LiveRendererImportStartupStatus,
    ) -> Option<LiveBackendRuntimeAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let renderer_selection = self.renderer_selection_for_status(renderer_status)?;
        let selected_output = self.selected_output()?;
        let renderer_observation = LiveRendererRuntimeObservation::from_startup_status(
            renderer_status,
            selection_observation(renderer_selection),
        );
        let scanout_readiness = self.scanout_readiness_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        });
        let kms_scanout_target = self.kms_scanout_target_report(LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        });
        let page_flip_event =
            LivePageFlipEvent::from_kms_scanout_target_status(kms_scanout_target.status);
        let page_flip_callback_intake = LivePageFlipCallbackIntake::new(selected_output.id);
        let gbm_egl_frame_target = LiveGbmEglFrameTargetRecord::new(selected_output.size);
        self.into_headless_assembly(poller, renderer_selection)
            .map(|assembly| LiveBackendRuntimeAssembly {
                assembly,
                renderer_observation,
                output_size: Some(selected_output.size),
                scanout_readiness,
                kms_scanout_target,
                gbm_egl_frame_target: Some(gbm_egl_frame_target),
                gbm_egl_frame_target_lifecycle: Some(
                    LiveGbmEglFrameTargetLifecycleReport::created(gbm_egl_frame_target),
                ),
                gbm_egl_frame_target_allocation: None,
                page_flip_event,
                page_flip_callback_intake,
                page_flip_callback_queue: None,
                libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics::not_configured(),
                #[cfg(feature = "libdrm-events")]
                rendered_primary_plane_scanout_submission: None,
                #[cfg(feature = "libdrm-events")]
                rendered_primary_plane_scanout_cleanup: None,
                #[cfg(feature = "libdrm-events")]
                rendered_primary_plane_runtime_scanout_state: None,
                #[cfg(feature = "libdrm-events")]
                rendered_primary_plane_scanout_in_flight_ticks: 0,
                #[cfg(feature = "libdrm-events")]
                pending_runtime_scanout_states: VecDeque::new(),
            })
    }
}

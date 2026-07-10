use crate::prelude::*;

use super::renderer::selection_observation;

impl LiveBackendStartupReport {
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

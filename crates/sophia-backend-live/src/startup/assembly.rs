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
        self.into_headless_assembly(poller, renderer_selection)
            .map(|assembly| {
                LiveBackendRuntimeAssembly::from_ready_headless_scanout(
                    assembly,
                    selected_output,
                    renderer_observation,
                )
            })
    }
}

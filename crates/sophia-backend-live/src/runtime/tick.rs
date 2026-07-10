use crate::prelude::*;

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn run_tick(
        &mut self,
        mut input: CompositorBackendTickInput,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError> {
        let page_flip_callbacks = self.drain_page_flip_callback_queue();

        #[cfg(feature = "libdrm-events")]
        self.advance_rendered_primary_plane_scanout_age_if_in_flight();
        #[cfg(feature = "libdrm-events")]
        let runtime_scanout_states: Vec<RuntimeScanoutState> =
            self.pending_runtime_scanout_states.drain(..).collect();
        #[cfg(not(feature = "libdrm-events"))]
        let runtime_scanout_states: Vec<RuntimeScanoutState> = Vec::new();
        input
            .scanout_lifecycle_states
            .extend(runtime_scanout_states.iter().copied());

        let engine = self.assembly.run_tick(input)?;

        Ok(self.build_tick_report(LiveBackendRuntimeTickReportInput {
            engine,
            page_flip_callbacks,
            runtime_scanout_states,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_retry: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_submit: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_retire: None,
        }))
    }
}

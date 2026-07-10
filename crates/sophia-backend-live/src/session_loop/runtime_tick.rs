use crate::prelude::*;

impl<P> LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>
where
    P: NonBlockingInputPoller,
{
    #[allow(clippy::too_many_arguments)]
    pub fn run_session_loop_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with<
        D,
        E,
        R,
    >(
        &mut self,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        device: &D,
        exporter: &mut E,
        reader: &mut R,
        poller: &mut NativeLibdrmPageFlipEventPoller,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
        R: LibdrmNativePageFlipReader,
    {
        if readiness.input_ready {
            self.assembly_mut().input_mut().poller_mut().observe_ready();
        }

        let native_page_flip = if readiness.page_flip_ready {
            poller.read_and_poll_page_flip_events(
                reader,
                sender,
                page_flip_budget.max_read,
                page_flip_budget.max_emit,
            )
        } else {
            let poll = poller.poll_page_flip_events(sender, page_flip_budget.max_emit);
            LibdrmNativeReadAndPollReport {
                read_loop: poller.last_read_loop_report(),
                poll,
            }
        };
        self.observe_native_libdrm_poller_diagnostics(poller.diagnostics());
        let tick =
            self.run_tick_with_rendered_primary_plane_scanout_with(input, device, exporter)?;
        let input_gate = self.assembly().input().poller().last_gate_report();

        Ok(LiveBackendSessionLoopTickReport {
            input_gate,
            native_page_flip,
            tick,
        })
    }

    #[cfg(feature = "gbm-probe")]
    #[allow(clippy::too_many_arguments)]
    pub fn run_session_loop_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with<
        D,
        E,
        R,
    >(
        &mut self,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        reader: &mut R,
        poller: &mut NativeLibdrmPageFlipEventPoller,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: RenderDeviceDiscoveryBackend,
        R: LibdrmNativePageFlipReader,
    {
        self.run_session_loop_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            input, readiness, page_flip_budget, device, exporter, reader, poller, sender,
        )
    }
}

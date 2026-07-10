use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendSessionLoop {
    page_flip_poller: NativeLibdrmPageFlipEventPoller,
    page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
}

impl LiveBackendSessionLoop {
    pub const fn new(
        page_flip_poller: NativeLibdrmPageFlipEventPoller,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
    ) -> Self {
        Self {
            page_flip_poller,
            page_flip_budget,
        }
    }

    pub const fn page_flip_budget(&self) -> LiveBackendSessionLoopPageFlipBudget {
        self.page_flip_budget
    }

    pub const fn page_flip_poller(&self) -> &NativeLibdrmPageFlipEventPoller {
        &self.page_flip_poller
    }

    pub fn page_flip_poller_mut(&mut self) -> &mut NativeLibdrmPageFlipEventPoller {
        &mut self.page_flip_poller
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with<
        P,
        D,
        E,
        R,
    >(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        device: &D,
        exporter: &mut E,
        reader: &mut R,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: LiveRenderedScanoutBufferPrimeSource + 'static,
        R: LibdrmNativePageFlipReader,
    {
        runtime
            .run_session_loop_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
                input,
                readiness,
                self.page_flip_budget,
                device,
                exporter,
                reader,
                &mut self.page_flip_poller,
                sender,
            )
    }

    #[cfg(feature = "gbm-probe")]
    #[allow(clippy::too_many_arguments)]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with<
        P,
        D,
        E,
        R,
    >(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        reader: &mut R,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: RenderDeviceDiscoveryBackend,
        R: LibdrmNativePageFlipReader,
    {
        self.run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            runtime, input, readiness, device, exporter, reader, sender,
        )
    }
}

use crate::prelude::*;

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    #[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_with<D, R>(
        &mut self,
        input: CompositorBackendTickInput,
        device: &D,
        discovery: &R,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        R: RenderDeviceDiscoveryBackend,
    {
        let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery);
        self.run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
            input,
            device,
            &mut exporter,
        )
    }

    #[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with<D, R>(
        &mut self,
        input: CompositorBackendTickInput,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<R>,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        R: RenderDeviceDiscoveryBackend,
    {
        self.run_tick_with_rendered_primary_plane_scanout_with(input, device, exporter)
    }

    #[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
    #[allow(clippy::too_many_arguments)]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with<
        D,
        E,
        R,
    >(
        &mut self,
        input: CompositorBackendTickInput,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        reader: &mut R,
        poller: &mut NativeLibdrmPageFlipEventPoller,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> Result<LiveBackendRuntimeNativePageFlipTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: RenderDeviceDiscoveryBackend,
        R: LibdrmNativePageFlipReader,
    {
        self.run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            input, device, exporter, reader, poller, sender, max_read, max_emit,
        )
    }
}

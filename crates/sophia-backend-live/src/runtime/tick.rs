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

        Ok(LiveBackendRuntimeTickReport {
            engine,
            renderer: self.renderer_observation,
            scanout: self.scanout_readiness,
            kms_scanout_target: self.kms_scanout_target,
            gbm_egl_frame_target: self.gbm_egl_frame_target,
            gbm_egl_frame_target_lifecycle: self.gbm_egl_frame_target_lifecycle,
            gbm_egl_frame_target_allocation: self.gbm_egl_frame_target_allocation,
            page_flip: self.page_flip_event,
            page_flip_callbacks,
            runtime_scanout_states,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_pending: self
                .rendered_primary_plane_scanout_cleanup_pending(),
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_retry: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_in_flight_ticks: self
                .rendered_primary_plane_scanout_in_flight_ticks,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_backpressure: self
                .rendered_primary_plane_scanout_backpressure_report(
                    LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
                ),
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_submit: None,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_retire: None,
            libdrm_poller: self.libdrm_poller_diagnostics,
        })
    }

    #[cfg(feature = "libdrm-events")]
    pub fn run_tick_with_rendered_primary_plane_scanout_with<D, E>(
        &mut self,
        mut input: CompositorBackendTickInput,
        device: &D,
        exporter: &mut E,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
    {
        let rendered_primary_plane_scanout_cleanup_retry = self
            .rendered_primary_plane_scanout_cleanup_pending()
            .then(|| self.retry_tracked_rendered_primary_plane_scanout_cleanup(device));
        let page_flip_callbacks = self.drain_page_flip_callback_queue();
        let rendered_primary_plane_scanout_retire =
            page_flip_callbacks.last_accepted.map(|callback| {
                self.retire_tracked_rendered_primary_plane_scanout_after_page_flip(
                    device, &callback,
                )
            });
        self.advance_rendered_primary_plane_scanout_age_if_in_flight();
        let runtime_scanout_states: Vec<RuntimeScanoutState> =
            self.pending_runtime_scanout_states.drain(..).collect();
        input
            .scanout_lifecycle_states
            .extend(runtime_scanout_states.iter().copied());

        let target = self.gbm_egl_frame_target;
        let scanout_target = self.kms_scanout_target.status;
        let rendered_primary_plane_scanout_submission =
            &mut self.rendered_primary_plane_scanout_submission;
        let rendered_primary_plane_scanout_cleanup =
            &mut self.rendered_primary_plane_scanout_cleanup;
        let rendered_primary_plane_runtime_scanout_state =
            &mut self.rendered_primary_plane_runtime_scanout_state;
        let rendered_primary_plane_scanout_in_flight_ticks =
            &mut self.rendered_primary_plane_scanout_in_flight_ticks;
        let submitted_after_page_flip_serial = self.page_flip_callback_intake.last_frame_serial();
        let mut rendered_primary_plane_scanout_submit = None;

        let engine = self.assembly.run_tick_with_live_runtime_adapter(
            input,
            |engine: &HeadlessEngine, intake: LiveRuntimeDriverIntake| {
                let inner = LiveRuntimeDriverAdapter::from_authority_batches(engine, intake);
                LiveRenderedPrimaryPlaneRuntimeAdapter {
                    inner,
                    scanout_target,
                    target,
                    rendered_primary_plane_scanout_submission,
                    rendered_primary_plane_scanout_cleanup,
                    rendered_primary_plane_runtime_scanout_state,
                    rendered_primary_plane_scanout_in_flight_ticks,
                    submitted_after_page_flip_serial,
                    device,
                    exporter,
                    submit_report: &mut rendered_primary_plane_scanout_submit,
                }
            },
            |adapter| adapter.inner.renderer.committed_surfaces.clone(),
        )?;

        Ok(LiveBackendRuntimeTickReport {
            engine,
            renderer: self.renderer_observation,
            scanout: self.scanout_readiness,
            kms_scanout_target: self.kms_scanout_target,
            gbm_egl_frame_target: self.gbm_egl_frame_target,
            gbm_egl_frame_target_lifecycle: self.gbm_egl_frame_target_lifecycle,
            gbm_egl_frame_target_allocation: self.gbm_egl_frame_target_allocation,
            page_flip: self.page_flip_event,
            page_flip_callbacks,
            runtime_scanout_states,
            rendered_primary_plane_scanout_cleanup_pending: self
                .rendered_primary_plane_scanout_cleanup_pending(),
            rendered_primary_plane_scanout_cleanup_retry,
            rendered_primary_plane_scanout_in_flight_ticks: self
                .rendered_primary_plane_scanout_in_flight_ticks,
            rendered_primary_plane_scanout_backpressure: self
                .rendered_primary_plane_scanout_backpressure_report(
                    LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
                ),
            rendered_primary_plane_scanout_submit,
            rendered_primary_plane_scanout_retire,
            libdrm_poller: self.libdrm_poller_diagnostics,
        })
    }

    #[cfg(feature = "libdrm-events")]
    #[allow(clippy::too_many_arguments)]
    pub fn run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with<D, E, R>(
        &mut self,
        input: CompositorBackendTickInput,
        device: &D,
        exporter: &mut E,
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
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
        R: LibdrmNativePageFlipReader,
    {
        let native_page_flip =
            poller.read_and_poll_page_flip_events(reader, sender, max_read, max_emit);
        self.observe_native_libdrm_poller_diagnostics(poller.diagnostics());
        let tick =
            self.run_tick_with_rendered_primary_plane_scanout_with(input, device, exporter)?;

        Ok(LiveBackendRuntimeNativePageFlipTickReport {
            native_page_flip,
            tick,
        })
    }

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

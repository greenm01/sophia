use crate::prelude::*;

mod frame_target;
mod rendered_primary_plane;

pub struct LiveBackendRuntimeAssembly<P = QueuedInputPoller> {
    pub(crate) assembly: HeadlessCompositorBackendAssembly<P>,
    pub(crate) renderer_observation: LiveRendererRuntimeObservation,
    pub(crate) output_size: Option<Size>,
    pub(crate) scanout_readiness: LiveScanoutReadinessReport,
    pub(crate) kms_scanout_target: LiveKmsScanoutTargetReport,
    pub(crate) gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    pub(crate) gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub(crate) gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    pub(crate) page_flip_event: LivePageFlipEvent,
    pub(crate) page_flip_callback_intake: LivePageFlipCallbackIntake,
    pub(crate) page_flip_callback_queue: Option<LivePageFlipCallbackQueue>,
    pub(crate) libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_submission:
        Option<BoxedRenderedPrimaryPlaneScanoutSubmission>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_cleanup:
        Option<BoxedRenderedPrimaryPlaneScanoutCleanup>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_runtime_scanout_state: Option<RuntimeScanoutState>,
    #[cfg(feature = "libdrm-events")]
    pub(crate) rendered_primary_plane_scanout_in_flight_ticks: u64,
    #[cfg(feature = "libdrm-events")]
    pub(crate) pending_runtime_scanout_states: VecDeque<RuntimeScanoutState>,
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly<P> {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly<P> {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
    }

    pub fn with_page_flip_callback_queue(mut self, queue: LivePageFlipCallbackQueue) -> Self {
        self.page_flip_callback_queue = Some(queue);
        self
    }

    pub fn with_libdrm_poller_diagnostics(
        mut self,
        diagnostics: LiveLibdrmPollerDiagnostics,
    ) -> Self {
        self.libdrm_poller_diagnostics = diagnostics;
        self
    }

    #[cfg(feature = "libdrm-events")]
    pub fn with_native_libdrm_poller_diagnostics(
        self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) -> Self {
        self.with_libdrm_poller_diagnostics(diagnostics.into())
    }

    pub fn observe_libdrm_poller_diagnostics(&mut self, diagnostics: LiveLibdrmPollerDiagnostics) {
        self.libdrm_poller_diagnostics = diagnostics;
    }

    #[cfg(feature = "libdrm-events")]
    pub fn observe_native_libdrm_poller_diagnostics(
        &mut self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) {
        self.observe_libdrm_poller_diagnostics(diagnostics.into());
    }

    pub fn libdrm_poller_diagnostics(&self) -> LiveLibdrmPollerDiagnostics {
        self.libdrm_poller_diagnostics
    }

    pub fn page_flip_observation(&self) -> LivePageFlipEvent {
        self.page_flip_event
    }

    pub fn observe_page_flip_outcome(&mut self, outcome: &PageFlipCommitOutcome) {
        self.page_flip_event = LivePageFlipEvent::from_commit_outcome(outcome);
    }

    pub fn observe_atomic_scanout_commit(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report = LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_with<C>(
        &mut self,
        committer: &mut C,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let report = committer.commit_atomic_scanout(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_after_page_flip_with<C>(
        &mut self,
        committer: &mut C,
        callback: LivePageFlipCallback,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let callback_report = self.page_flip_callback_intake.observe(callback);
        let report = committer.commit_atomic_scanout_after_page_flip(&callback_report, outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn observe_page_flip_callback(
        &mut self,
        callback: LivePageFlipCallback,
    ) -> LivePageFlipCallbackReport {
        let report = self.page_flip_callback_intake.observe(callback);
        self.page_flip_event = report.event;
        report
    }

    pub fn run_tick(
        &mut self,
        mut input: CompositorBackendTickInput,
    ) -> Result<LiveBackendRuntimeTickReport, CompositorBackendAssemblyError> {
        let page_flip_callbacks = self
            .page_flip_callback_queue
            .as_ref()
            .map(|queue| {
                queue.drain_ready(
                    &mut self.page_flip_callback_intake,
                    &mut self.page_flip_event,
                )
            })
            .unwrap_or_default();

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
        let page_flip_callbacks = self
            .page_flip_callback_queue
            .as_ref()
            .map(|queue| {
                queue.drain_ready(
                    &mut self.page_flip_callback_intake,
                    &mut self.page_flip_event,
                )
            })
            .unwrap_or_default();
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
        let rendered_primary_plane_scanout_cleanup_pending =
            self.rendered_primary_plane_scanout_cleanup_pending();
        let rendered_primary_plane_scanout_submission =
            &mut self.rendered_primary_plane_scanout_submission;
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
                    rendered_primary_plane_runtime_scanout_state,
                    rendered_primary_plane_scanout_in_flight_ticks,
                    cleanup_pending: rendered_primary_plane_scanout_cleanup_pending,
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

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeTickReport {
    pub engine: CompositorBackendTickReport,
    pub renderer: LiveRendererRuntimeObservation,
    pub scanout: LiveScanoutReadinessReport,
    pub kms_scanout_target: LiveKmsScanoutTargetReport,
    pub gbm_egl_frame_target: Option<LiveGbmEglFrameTargetRecord>,
    pub gbm_egl_frame_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    pub gbm_egl_frame_target_allocation: Option<LiveGbmEglFrameTargetAllocationReport>,
    pub page_flip: LivePageFlipEvent,
    pub page_flip_callbacks: LivePageFlipCallbackQueueReport,
    pub runtime_scanout_states: Vec<RuntimeScanoutState>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_cleanup_pending: bool,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_cleanup_retry:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_in_flight_ticks: u64,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_submit:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_retire:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutRetireReport>,
    pub libdrm_poller: LiveLibdrmPollerDiagnostics,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeNativePageFlipTickReport {
    pub native_page_flip: LibdrmNativeReadAndPollReport,
    pub tick: LiveBackendRuntimeTickReport,
}

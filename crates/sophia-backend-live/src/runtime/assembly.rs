use crate::prelude::*;

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
    pub(crate) fn from_ready_headless_scanout(
        assembly: HeadlessCompositorBackendAssembly<P>,
        output: HeadlessOutput,
        renderer_observation: LiveRendererRuntimeObservation,
    ) -> Self {
        let presentation = LiveRendererPresentationReport {
            status: LiveRendererPresentationStatus::Ready,
        };
        let scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        let gbm_egl_frame_target = LiveGbmEglFrameTargetRecord::new(output.size);
        let kms_scanout_target = LiveKmsScanoutTargetReport::from_output_target_and_presentation(
            Some(output.size),
            Some(gbm_egl_frame_target),
            presentation,
        );
        let page_flip_event =
            LivePageFlipEvent::from_kms_scanout_target_status(kms_scanout_target.status);

        Self {
            assembly,
            renderer_observation,
            output_size: Some(output.size),
            scanout_readiness,
            kms_scanout_target,
            gbm_egl_frame_target: Some(gbm_egl_frame_target),
            gbm_egl_frame_target_lifecycle: Some(LiveGbmEglFrameTargetLifecycleReport::created(
                gbm_egl_frame_target,
            )),
            gbm_egl_frame_target_allocation: None,
            page_flip_event,
            page_flip_callback_intake: LivePageFlipCallbackIntake::new(output.id),
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
        }
    }

    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly<P> {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly<P> {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
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
}

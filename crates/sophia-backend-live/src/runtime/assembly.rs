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

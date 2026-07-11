use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendRuntimeTickReport {
    pub engine: CompositorBackendTickReport,
    pub renderer: LiveRendererRuntimeObservation,
    pub scanout: LiveScanoutReadinessReport,
    pub output_size: Option<Size>,
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
    pub rendered_primary_plane_scanout_backpressure:
        LiveRenderedPrimaryPlaneScanoutBackpressureReport,
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

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Debug, PartialEq)]
pub struct LiveRenderedPrimaryPlanePageFlipDrainReport {
    pub page_flip_callbacks: LivePageFlipCallbackQueueReport,
    pub rendered_primary_plane_scanout_retire:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutRetireReport>,
}

pub(crate) struct LiveBackendRuntimeTickReportInput {
    pub engine: CompositorBackendTickReport,
    pub page_flip_callbacks: LivePageFlipCallbackQueueReport,
    pub runtime_scanout_states: Vec<RuntimeScanoutState>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_cleanup_retry:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutCleanupReport>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_submit:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutSubmitReport>,
    #[cfg(feature = "libdrm-events")]
    pub rendered_primary_plane_scanout_retire:
        Option<LiveTrackedRenderedPrimaryPlaneScanoutRetireReport>,
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub(crate) fn build_tick_report(
        &self,
        input: LiveBackendRuntimeTickReportInput,
    ) -> LiveBackendRuntimeTickReport {
        let state = self.primary_output_state();
        LiveBackendRuntimeTickReport {
            engine: input.engine,
            renderer: self.renderer_observation,
            scanout: state.scanout_readiness,
            output_size: state.output_size,
            kms_scanout_target: state.kms_scanout_target,
            gbm_egl_frame_target: state.gbm_egl_frame_target,
            gbm_egl_frame_target_lifecycle: state.gbm_egl_frame_target_lifecycle,
            gbm_egl_frame_target_allocation: state.gbm_egl_frame_target_allocation,
            page_flip: state.page_flip_event,
            page_flip_callbacks: input.page_flip_callbacks,
            runtime_scanout_states: input.runtime_scanout_states,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_pending: self
                .rendered_primary_plane_scanout_cleanup_pending(),
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_cleanup_retry: input
                .rendered_primary_plane_scanout_cleanup_retry,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_in_flight_ticks: state
                .rendered_primary_plane_scanout_in_flight_ticks,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_backpressure: self
                .rendered_primary_plane_scanout_backpressure_report(
                    LIVE_RENDERED_PRIMARY_PLANE_SCANOUT_STALL_THRESHOLD_TICKS,
                ),
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_submit: input.rendered_primary_plane_scanout_submit,
            #[cfg(feature = "libdrm-events")]
            rendered_primary_plane_scanout_retire: input.rendered_primary_plane_scanout_retire,
            libdrm_poller: self.libdrm_poller_diagnostics,
        }
    }
}

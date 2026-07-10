use crate::prelude::*;

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

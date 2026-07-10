use super::*;

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn scanout_readiness_observation(&self) -> LiveScanoutReadinessReport {
        self.scanout_readiness
    }

    pub fn kms_scanout_target_observation(&self) -> LiveKmsScanoutTargetReport {
        self.kms_scanout_target
    }

    pub fn gbm_egl_frame_target_observation(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.gbm_egl_frame_target
    }

    pub fn gbm_egl_frame_target_lifecycle_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.gbm_egl_frame_target_lifecycle
    }

    pub fn gbm_egl_frame_target_allocation_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport> {
        self.gbm_egl_frame_target_allocation
    }

    pub fn observe_gbm_egl_frame_target_size(&mut self, size: Size) -> LiveGbmEglFrameTargetRecord {
        let previous = self.gbm_egl_frame_target;
        let record = LiveGbmEglFrameTargetRecord::new(size);
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::from_size_update(previous, record);
        self.gbm_egl_frame_target = Some(record);
        self.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        if lifecycle.status != LiveGbmEglFrameTargetLifecycleStatus::Retained {
            self.gbm_egl_frame_target_allocation = None;
        }
        self.refresh_kms_scanout_target(self.current_renderer_presentation_report());
        record
    }

    pub fn retire_gbm_egl_frame_target(&mut self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        let target = self.gbm_egl_frame_target.take()?;
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::retired(target);
        self.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        self.gbm_egl_frame_target_allocation = None;
        self.refresh_kms_scanout_target(self.current_renderer_presentation_report());
        Some(lifecycle)
    }

    pub fn allocate_gbm_egl_frame_target<A>(
        &mut self,
        allocator: &mut A,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        A: LiveGbmEglFrameTargetAllocator,
    {
        let target = self.gbm_egl_frame_target?;
        let report =
            allocator.allocate_frame_target(LiveGbmEglFrameTargetAllocationRequest { target });
        self.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    #[cfg(all(feature = "egl-probe", feature = "gbm-probe"))]
    pub fn allocate_native_gbm_egl_frame_target_with_gbm_device<D>(
        &mut self,
        discovery: &D,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        D: RenderDeviceDiscoveryBackend,
    {
        let target = self.gbm_egl_frame_target?;
        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
                discovery.open_render_device(),
                LiveGbmEglFrameTargetAllocationRequest { target },
            );
        self.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    pub fn observe_presentation_report(&mut self, presentation: LiveRendererPresentationReport) {
        self.scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        self.refresh_kms_scanout_target(presentation);
    }

    fn current_renderer_presentation_report(&self) -> LiveRendererPresentationReport {
        LiveRendererPresentationReport {
            status: match self.scanout_readiness.status {
                LiveScanoutReadinessStatus::Ready => LiveRendererPresentationStatus::Ready,
                LiveScanoutReadinessStatus::OutputUnavailable
                | LiveScanoutReadinessStatus::PresentationUnavailable => {
                    LiveRendererPresentationStatus::Unavailable
                }
                LiveScanoutReadinessStatus::Degraded => LiveRendererPresentationStatus::Degraded,
            },
        }
    }

    fn refresh_kms_scanout_target(&mut self, presentation: LiveRendererPresentationReport) {
        self.kms_scanout_target = LiveKmsScanoutTargetReport::from_output_target_and_presentation(
            self.output_size,
            self.gbm_egl_frame_target,
            presentation,
        );
        self.page_flip_event =
            LivePageFlipEvent::from_kms_scanout_target_status(self.kms_scanout_target.status);
    }
}

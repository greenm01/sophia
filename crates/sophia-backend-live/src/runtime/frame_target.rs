use super::*;

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn scanout_readiness_observation(&self) -> LiveScanoutReadinessReport {
        self.primary_output_state().scanout_readiness
    }

    pub fn kms_scanout_target_observation(&self) -> LiveKmsScanoutTargetReport {
        self.primary_output_state().kms_scanout_target
    }

    pub fn gbm_egl_frame_target_observation(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.primary_output_state().gbm_egl_frame_target
    }

    pub fn gbm_egl_frame_target_lifecycle_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.primary_output_state().gbm_egl_frame_target_lifecycle
    }

    pub fn gbm_egl_frame_target_allocation_observation(
        &self,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport> {
        self.primary_output_state().gbm_egl_frame_target_allocation
    }

    pub fn output_size_observation(&self) -> Option<Size> {
        self.primary_output_state().output_size
    }

    pub fn observe_output_size(&mut self, size: Size) {
        self.observe_output_size_for(self.primary_output, size);
    }

    pub fn observe_output_size_for(&mut self, output: OutputId, size: Size) -> bool {
        let Some(state) = self.outputs.get_mut(output) else {
            return false;
        };
        let previous = state.output_size;
        state.output_size = Some(size);
        if previous != state.output_size {
            state.gbm_egl_frame_target_allocation = None;
        }
        refresh_kms_scanout_target(state, current_renderer_presentation_report(state));
        true
    }

    pub fn observe_gbm_egl_frame_target_size(&mut self, size: Size) -> LiveGbmEglFrameTargetRecord {
        self.observe_gbm_egl_frame_target_size_for(self.primary_output, size)
            .expect("live runtime primary output must remain registered")
    }

    pub fn observe_gbm_egl_frame_target_size_for(
        &mut self,
        output: OutputId,
        size: Size,
    ) -> Option<LiveGbmEglFrameTargetRecord> {
        let state = self.outputs.get_mut(output)?;
        let previous = state.gbm_egl_frame_target;
        let record = LiveGbmEglFrameTargetRecord::new(size);
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::from_size_update(previous, record);
        state.gbm_egl_frame_target = Some(record);
        state.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        if lifecycle.status != LiveGbmEglFrameTargetLifecycleStatus::Retained {
            state.gbm_egl_frame_target_allocation = None;
        }
        refresh_kms_scanout_target(state, current_renderer_presentation_report(state));
        Some(record)
    }

    pub fn retire_gbm_egl_frame_target(&mut self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.retire_gbm_egl_frame_target_for(self.primary_output)
    }

    pub fn retire_gbm_egl_frame_target_for(
        &mut self,
        output: OutputId,
    ) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        let state = self.outputs.get_mut(output)?;
        let target = state.gbm_egl_frame_target.take()?;
        let lifecycle = LiveGbmEglFrameTargetLifecycleReport::retired(target);
        state.gbm_egl_frame_target_lifecycle = Some(lifecycle);
        state.gbm_egl_frame_target_allocation = None;
        refresh_kms_scanout_target(state, current_renderer_presentation_report(state));
        Some(lifecycle)
    }

    pub fn allocate_gbm_egl_frame_target<A>(
        &mut self,
        allocator: &mut A,
    ) -> Option<LiveGbmEglFrameTargetAllocationReport>
    where
        A: LiveGbmEglFrameTargetAllocator,
    {
        let state = self.primary_output_state_mut();
        let target = state.gbm_egl_frame_target?;
        let report =
            allocator.allocate_frame_target(LiveGbmEglFrameTargetAllocationRequest { target });
        state.gbm_egl_frame_target_allocation = Some(report);
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
        let state = self.primary_output_state_mut();
        let target = state.gbm_egl_frame_target?;
        let report =
            NativeGbmBackedEglFrameTargetAllocator::allocation_report_from_backend_device_result(
                discovery.open_render_device(),
                LiveGbmEglFrameTargetAllocationRequest { target },
            );
        state.gbm_egl_frame_target_allocation = Some(report);
        Some(report)
    }

    pub fn observe_presentation_report(&mut self, presentation: LiveRendererPresentationReport) {
        let state = self.primary_output_state_mut();
        state.scanout_readiness =
            LiveScanoutReadinessReport::from_output_and_presentation(true, presentation);
        refresh_kms_scanout_target(state, presentation);
    }
}

fn current_renderer_presentation_report(
    state: &LiveRenderedOutputState,
) -> LiveRendererPresentationReport {
    LiveRendererPresentationReport {
        status: match state.scanout_readiness.status {
            LiveScanoutReadinessStatus::Ready => LiveRendererPresentationStatus::Ready,
            LiveScanoutReadinessStatus::OutputUnavailable
            | LiveScanoutReadinessStatus::PresentationUnavailable => {
                LiveRendererPresentationStatus::Unavailable
            }
            LiveScanoutReadinessStatus::Degraded => LiveRendererPresentationStatus::Degraded,
        },
    }
}

fn refresh_kms_scanout_target(
    state: &mut LiveRenderedOutputState,
    presentation: LiveRendererPresentationReport,
) {
    state.kms_scanout_target = LiveKmsScanoutTargetReport::from_output_target_and_presentation(
        state.output_size,
        state.gbm_egl_frame_target,
        presentation,
    );
    state.page_flip_event =
        LivePageFlipEvent::from_kms_scanout_target_status(state.kms_scanout_target.status);
}

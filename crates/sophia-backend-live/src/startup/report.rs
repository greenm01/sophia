use crate::prelude::*;

impl LiveBackendStartupReport {
    pub fn status(&self) -> &LiveCompositorBackendDiscoveryStatus {
        &self.discovery.status
    }

    pub fn selected_output(&self) -> Option<HeadlessOutput> {
        self.discovery.selected_output
    }

    pub fn scanout_readiness_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveScanoutReadinessReport {
        LiveScanoutReadinessReport::from_backend_and_presentation(self, presentation)
    }

    pub fn kms_scanout_target_report(
        &self,
        presentation: LiveRendererPresentationReport,
    ) -> LiveKmsScanoutTargetReport {
        LiveKmsScanoutTargetReport::from_backend_and_presentation(self, presentation)
    }

    pub fn selected_gbm_egl_frame_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.selected_output()
            .map(|output| LiveGbmEglFrameTargetRecord::new(output.size))
    }
}

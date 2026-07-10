use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRealGbmSmokeEvidence {
    pub status: LiveRealGbmSmokeEvidenceStatus,
    pub draw: EglDrawSmokeStatus,
    pub presentation: LiveRendererPresentationStatus,
    pub frame_target_allocation: LiveGbmEglFrameTargetAllocationStatus,
}

impl LiveRealGbmSmokeEvidence {
    pub const fn from_reports(
        draw: EglDrawSmokeReport,
        presentation: LiveRendererPresentationReport,
        frame_target_allocation: LiveGbmEglFrameTargetAllocationReport,
    ) -> Self {
        let status = match (
            draw.status,
            presentation.status,
            frame_target_allocation.status,
        ) {
            (
                EglDrawSmokeStatus::ClearColorReady,
                LiveRendererPresentationStatus::Ready,
                LiveGbmEglFrameTargetAllocationStatus::Ready,
            ) => LiveRealGbmSmokeEvidenceStatus::Passed,
            _ => LiveRealGbmSmokeEvidenceStatus::Failed,
        };

        Self {
            status,
            draw: draw.status,
            presentation: presentation.status,
            frame_target_allocation: frame_target_allocation.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRealGbmSmokeEvidenceStatus {
    Passed,
    Failed,
}

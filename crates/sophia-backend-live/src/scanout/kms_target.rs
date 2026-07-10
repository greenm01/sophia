use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveKmsScanoutTargetReport {
    pub status: LiveKmsScanoutTargetStatus,
    pub size: Option<Size>,
}

impl LiveKmsScanoutTargetReport {
    pub(crate) fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        Self::from_output_target_and_presentation(
            backend.selected_output().map(|output| output.size),
            backend.selected_gbm_egl_frame_target(),
            presentation,
        )
    }

    pub(crate) fn from_output_target_and_presentation(
        output_size: Option<Size>,
        frame_target: Option<LiveGbmEglFrameTargetRecord>,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        let Some(output_size) = output_size else {
            return Self {
                status: LiveKmsScanoutTargetStatus::OutputUnavailable,
                size: None,
            };
        };

        let Some(frame_target) = frame_target else {
            return Self {
                status: LiveKmsScanoutTargetStatus::FrameTargetUnavailable,
                size: Some(output_size),
            };
        };

        if !frame_target.is_valid_scanout_target() {
            return Self {
                status: LiveKmsScanoutTargetStatus::InvalidFrameTarget,
                size: Some(frame_target.size),
            };
        }

        if frame_target.size != output_size {
            return Self {
                status: LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
                size: Some(frame_target.size),
            };
        }

        Self {
            status: match presentation.status {
                LiveRendererPresentationStatus::Ready => LiveKmsScanoutTargetStatus::Ready,
                LiveRendererPresentationStatus::Unavailable => {
                    LiveKmsScanoutTargetStatus::PresentationUnavailable
                }
                LiveRendererPresentationStatus::Degraded => LiveKmsScanoutTargetStatus::Degraded,
            },
            size: Some(frame_target.size),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKmsScanoutTargetStatus {
    Ready,
    OutputUnavailable,
    FrameTargetUnavailable,
    InvalidFrameTarget,
    FrameTargetSizeMismatch,
    PresentationUnavailable,
    Degraded,
}

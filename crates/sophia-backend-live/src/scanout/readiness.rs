use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveScanoutReadinessReport {
    pub status: LiveScanoutReadinessStatus,
}

impl LiveScanoutReadinessReport {
    pub(crate) fn from_backend_and_presentation(
        backend: &LiveBackendStartupReport,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        Self::from_output_and_presentation(backend.selected_output().is_some(), presentation)
    }

    pub(crate) fn from_output_and_presentation(
        output_available: bool,
        presentation: LiveRendererPresentationReport,
    ) -> Self {
        if !output_available {
            return Self {
                status: LiveScanoutReadinessStatus::OutputUnavailable,
            };
        }

        Self {
            status: match presentation.status {
                LiveRendererPresentationStatus::Ready => LiveScanoutReadinessStatus::Ready,
                LiveRendererPresentationStatus::Unavailable => {
                    LiveScanoutReadinessStatus::PresentationUnavailable
                }
                LiveRendererPresentationStatus::Degraded => LiveScanoutReadinessStatus::Degraded,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveScanoutReadinessStatus {
    Ready,
    OutputUnavailable,
    PresentationUnavailable,
    Degraded,
}

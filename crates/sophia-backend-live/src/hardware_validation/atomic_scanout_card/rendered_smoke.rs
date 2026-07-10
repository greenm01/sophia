use super::{RealAtomicScanoutPageFlipSession, RealAtomicScanoutPageFlipWaitPolicy};
use crate::prelude::*;
use sophia_renderer_live::NativeGbmRenderedScanoutContextStatus;

impl RealAtomicScanoutPageFlipSession {
    pub fn run_native_gbm_rendered_primary_plane_smoke_phase<R>(
        &mut self,
        phase: LibdrmNativeAtomicScanoutSmokePhase,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<R>,
        intake: &mut LivePageFlipCallbackIntake,
        wait_policy: RealAtomicScanoutPageFlipWaitPolicy,
    ) -> LibdrmNativeAtomicScanoutSmokeEvidence
    where
        R: RenderDeviceDiscoveryBackend,
    {
        let selected = self.selection();
        let target = LiveGbmEglFrameTargetRecord::new(selected.size());
        let scanout_target = reduced_scanout_target_status_from_native_selection(
            LiveKmsScanoutTargetStatus::Ready,
            target,
            &LibdrmNativePrimaryPlaneSelectionResult {
                status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                selection: Some(selected),
            },
        );
        if scanout_target != LiveKmsScanoutTargetStatus::Ready {
            return reduced_smoke_evidence_for_phase(
                phase,
                scanout_target,
                None,
                LiveRendererScanoutBufferExportStatus::Unavailable,
                None,
                None,
                None,
                None,
            );
        }

        let export = exporter.export_rendered_scanout_buffer(target).normalized();
        let rendered_context =
            reduced_rendered_context_status_from_native(exporter.context_status());
        if export.status != LiveRendererScanoutBufferExportStatus::Exported {
            return reduced_smoke_evidence_for_phase(
                phase,
                scanout_target,
                rendered_context,
                export.status,
                None,
                None,
                None,
                None,
            );
        }

        let (Some(descriptor), Some(_owned_buffer)) = (export.descriptor, export.owner) else {
            let mut evidence = reduced_smoke_evidence_for_phase(
                phase,
                scanout_target,
                rendered_context,
                export.status,
                None,
                None,
                None,
                None,
            );
            evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
            return evidence;
        };

        let mut submit =
            submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor_with_policy(
                self.card(),
                LibdrmNativePrimaryPlaneSelectionResult {
                    status: LibdrmNativePrimaryPlaneSelectionStatus::Selected,
                    selection: Some(selected),
                },
                descriptor,
                submit_policy_for_smoke_phase(phase),
            );
        if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
        {
            return reduced_smoke_evidence_for_phase(
                phase,
                scanout_target,
                rendered_context,
                export.status,
                Some(&submit),
                None,
                None,
                None,
            );
        }
        let Some(submission) = submit.submission.take() else {
            let mut evidence = reduced_smoke_evidence_for_phase(
                phase,
                scanout_target,
                rendered_context,
                export.status,
                Some(&submit),
                None,
                None,
                None,
            );
            evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
            return evidence;
        };

        let page_flip =
            self.wait_for_submitted_page_flip_retirement(intake, submission, wait_policy);
        reduced_smoke_evidence_for_phase(
            phase,
            scanout_target,
            rendered_context,
            export.status,
            Some(&submit),
            Some(&page_flip.poll),
            page_flip.callback_report.as_ref(),
            page_flip.retired.as_ref(),
        )
    }
}

fn reduced_smoke_evidence_for_phase(
    phase: LibdrmNativeAtomicScanoutSmokePhase,
    scanout_target: LiveKmsScanoutTargetStatus,
    rendered_context: Option<LibdrmNativeRenderedScanoutContextStatus>,
    gbm_export: LiveRendererScanoutBufferExportStatus,
    submit: Option<&LibdrmNativePrimaryPlaneScanoutSubmitResult>,
    poll: Option<&LibdrmPageFlipEventPollReport>,
    callback: Option<&LivePageFlipCallbackReport>,
    retire: Option<&LibdrmNativePrimaryPlaneScanoutRetireResult>,
) -> LibdrmNativeAtomicScanoutSmokeEvidence {
    match phase {
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset => {
            LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                rendered_context,
                gbm_export,
                submit,
                poll,
                callback,
                retire,
            )
        }
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip => {
            LibdrmNativeAtomicScanoutSmokeEvidence::from_page_flip_pipeline_reports(
                scanout_target,
                rendered_context,
                gbm_export,
                submit,
                poll,
                callback,
                retire,
            )
        }
    }
}

fn submit_policy_for_smoke_phase(
    phase: LibdrmNativeAtomicScanoutSmokePhase,
) -> LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    match phase {
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset => {
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset()
        }
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip => {
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip()
        }
    }
}

fn reduced_rendered_context_status_from_native(
    status: Option<NativeGbmRenderedScanoutContextStatus>,
) -> Option<LibdrmNativeRenderedScanoutContextStatus> {
    status.map(|status| match status {
        NativeGbmRenderedScanoutContextStatus::Ready => {
            LibdrmNativeRenderedScanoutContextStatus::Ready
        }
        NativeGbmRenderedScanoutContextStatus::Unavailable => {
            LibdrmNativeRenderedScanoutContextStatus::Unavailable
        }
        NativeGbmRenderedScanoutContextStatus::Degraded => {
            LibdrmNativeRenderedScanoutContextStatus::Degraded
        }
    })
}

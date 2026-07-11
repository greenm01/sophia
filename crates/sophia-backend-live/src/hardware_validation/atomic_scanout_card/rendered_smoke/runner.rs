use super::super::select_real_atomic_scanout_card;
use super::RealAtomicScanoutSmokeConfig;
use crate::prelude::*;
use sophia_renderer_live::NativeGbmOwnedScanoutBuffer;

pub fn run_real_atomic_scanout_smoke_phases() -> Vec<LibdrmNativeAtomicScanoutSmokeEvidence> {
    let Some(config) = RealAtomicScanoutSmokeConfig::default_primary_output() else {
        let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed();
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::PageFlipReaderUnavailable;
        return vec![evidence];
    };

    run_real_atomic_scanout_smoke_phases_with(config)
}

pub fn run_real_atomic_scanout_smoke_phases_with(
    config: RealAtomicScanoutSmokeConfig,
) -> Vec<LibdrmNativeAtomicScanoutSmokeEvidence> {
    run_real_atomic_scanout_smoke_phases_with_policy(config, false)
}

pub fn run_real_atomic_vrr_smoke_phases_with(
    config: RealAtomicScanoutSmokeConfig,
) -> Vec<LibdrmNativeAtomicScanoutSmokeEvidence> {
    run_real_atomic_scanout_smoke_phases_with_policy(config, true)
}

fn run_real_atomic_scanout_smoke_phases_with_policy(
    config: RealAtomicScanoutSmokeConfig,
    prove_vrr: bool,
) -> Vec<LibdrmNativeAtomicScanoutSmokeEvidence> {
    let mut session_result = select_real_atomic_scanout_card().into_page_flip_session(
        config.slot,
        config.output,
        config.authority,
    );
    let Some(mut session) = session_result.session.take() else {
        return vec![
            session_result
                .failure_evidence()
                .unwrap_or_else(LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed),
        ];
    };
    let (initial_policy, steady_policy) = if prove_vrr {
        let discovery = session.vrr_properties_for_selection(session.selection());
        if discovery.status != LibdrmNativeVrrPropertyDiscoveryStatus::Discovered
            || !discovery.capable
            || discovery.enable_property.is_none()
        {
            return vec![LibdrmNativeAtomicScanoutSmokeEvidence::property_discovery_failed()];
        }
        let capability = OutputVrrCapability { capable: true };
        let activation = decide_output_vrr(
            true,
            capability,
            OutputVrrEligibility {
                opaque_fullscreen_surface_count: 1,
                unoccluded: true,
                overlays_present: false,
                composition_required: false,
            },
        );
        let fallback = decide_output_vrr(
            true,
            capability,
            OutputVrrEligibility {
                opaque_fullscreen_surface_count: 1,
                unoccluded: true,
                overlays_present: true,
                composition_required: false,
            },
        );
        if activation != OutputVrrDecision::Enabled || fallback != OutputVrrDecision::Ineligible {
            return vec![LibdrmNativeAtomicScanoutSmokeEvidence::property_discovery_failed()];
        }
        (
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset().with_vrr_enabled(true),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip().with_vrr_enabled(false),
        )
    } else {
        (
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::modeset(),
            LibdrmNativePrimaryPlaneScanoutSubmitPolicy::page_flip(),
        )
    };
    let discovery = match session.render_device_discovery() {
        Ok(discovery) => discovery,
        Err(_) => {
            return vec![
                LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                    LiveKmsScanoutTargetStatus::Ready,
                    Some(LibdrmNativeRenderedScanoutContextStatus::Unavailable),
                    LiveRendererScanoutBufferExportStatus::Unavailable,
                    None,
                    None,
                    None,
                    None,
                ),
            ];
        }
    };
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery)
        .with_preferred_modifiers(session.preferred_xrgb8888_scanout_modifiers());
    let mut intake = LivePageFlipCallbackIntake::new(config.output);

    let mut initial = match session
        .submit_native_gbm_rendered_primary_plane_smoke_phase_with_policy(
            LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
            &mut exporter,
            initial_policy,
            session.selection(),
        ) {
        Ok(initial) => initial,
        Err(evidence) => return vec![evidence],
    };
    let Some(initial_submission) = initial.submission.take() else {
        let mut evidence = initial.evidence(None, None, None);
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        return vec![evidence];
    };
    let initial_presentation = session.wait_for_rendered_submitted_page_flip_presentation(
        &mut intake,
        initial_submission,
        config.wait_policy,
    );
    let initial_poll = initial_presentation.poll;
    let initial_callback = initial_presentation.callback_report;
    let Some(initial_callback_report) = initial_callback else {
        let initial_waiting = initial_presentation
            .submission
            .map(waiting_retire_from_rendered_submission);
        return vec![initial.evidence(Some(&initial_poll), None, initial_waiting.as_ref())];
    };
    let Some(initial_submission) = initial_presentation.submission else {
        let mut evidence =
            initial.evidence(Some(&initial_poll), Some(&initial_callback_report), None);
        evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        return vec![evidence];
    };
    if !is_accepted_presented_page_flip(&initial_callback_report) {
        let initial_waiting = waiting_retire_from_rendered_submission(initial_submission);
        return vec![initial.evidence(
            Some(&initial_poll),
            Some(&initial_callback_report),
            Some(&initial_waiting),
        )];
    }

    let mut steady = match session.submit_native_gbm_rendered_primary_plane_smoke_phase_with_policy(
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip,
        &mut exporter,
        steady_policy,
        session.selection(),
    ) {
        Ok(steady) => steady,
        Err(evidence) => {
            let initial_retire = retire_rendered_submission_after_page_flip(
                session.card(),
                initial_submission,
                &initial_callback_report,
            );
            let initial_evidence = initial.evidence(
                Some(&initial_poll),
                Some(&initial_callback_report),
                Some(&initial_retire),
            );
            return vec![initial_evidence, evidence];
        }
    };
    let Some(steady_submission) = steady.submission.take() else {
        let initial_retire = retire_rendered_submission_after_page_flip(
            session.card(),
            initial_submission,
            &initial_callback_report,
        );
        let initial_evidence = initial.evidence(
            Some(&initial_poll),
            Some(&initial_callback_report),
            Some(&initial_retire),
        );
        let mut steady_evidence = steady.evidence(None, None, None);
        steady_evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        return vec![initial_evidence, steady_evidence];
    };
    let steady_presentation = session.wait_for_rendered_submitted_page_flip_presentation(
        &mut intake,
        steady_submission,
        config.wait_policy,
    );
    let steady_poll = steady_presentation.poll;
    let steady_callback = steady_presentation.callback_report;
    let Some(steady_callback_report) = steady_callback else {
        let initial_retire = retire_rendered_submission_after_page_flip(
            session.card(),
            initial_submission,
            &initial_callback_report,
        );
        let initial_evidence = initial.evidence(
            Some(&initial_poll),
            Some(&initial_callback_report),
            Some(&initial_retire),
        );
        let steady_waiting = steady_presentation
            .submission
            .map(waiting_retire_from_rendered_submission);
        let steady_evidence = steady.evidence(Some(&steady_poll), None, steady_waiting.as_ref());
        return vec![initial_evidence, steady_evidence];
    };
    let Some(steady_submission) = steady_presentation.submission else {
        let initial_retire = retire_rendered_submission_after_page_flip(
            session.card(),
            initial_submission,
            &initial_callback_report,
        );
        let initial_evidence = initial.evidence(
            Some(&initial_poll),
            Some(&initial_callback_report),
            Some(&initial_retire),
        );
        let mut steady_evidence =
            steady.evidence(Some(&steady_poll), Some(&steady_callback_report), None);
        steady_evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::RetainedResourceMissing;
        return vec![initial_evidence, steady_evidence];
    };

    let initial_retire_callback = if is_accepted_presented_page_flip(&steady_callback_report) {
        &steady_callback_report
    } else {
        &initial_callback_report
    };
    let initial_retire = retire_rendered_submission_after_page_flip(
        session.card(),
        initial_submission,
        initial_retire_callback,
    );
    let steady_retire = if is_accepted_presented_page_flip(&steady_callback_report) {
        retire_rendered_submission_after_page_flip(
            session.card(),
            steady_submission,
            &steady_callback_report,
        )
    } else {
        waiting_retire_from_rendered_submission(steady_submission)
    };
    let initial = initial.evidence(
        Some(&initial_poll),
        Some(&initial_callback_report),
        Some(&initial_retire),
    );
    let steady = steady.evidence(
        Some(&steady_poll),
        Some(&steady_callback_report),
        Some(&steady_retire),
    );
    vec![initial, steady]
}

fn is_accepted_presented_page_flip(callback: &LivePageFlipCallbackReport) -> bool {
    callback.decision == LivePageFlipCallbackDecision::Accepted
        && callback.event.status == LivePageFlipEventStatus::Presented
}

fn waiting_retire_from_rendered_submission(
    submission: LiveRenderedPrimaryPlaneScanoutSubmission<NativeGbmOwnedScanoutBuffer>,
) -> LibdrmNativePrimaryPlaneScanoutRetireResult {
    let LiveRenderedPrimaryPlaneScanoutSubmission {
        scanout_buffer,
        primary_plane,
        submitted_after_page_flip_serial: _,
    } = submission;
    drop(scanout_buffer);
    LibdrmNativePrimaryPlaneScanoutRetireResult {
        status: LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip,
        destroy: None,
        submission: Some(primary_plane),
        cleanup: None,
    }
}

fn retire_rendered_submission_after_page_flip(
    card: &RealAtomicScanoutCard,
    submission: LiveRenderedPrimaryPlaneScanoutSubmission<NativeGbmOwnedScanoutBuffer>,
    callback: &LivePageFlipCallbackReport,
) -> LibdrmNativePrimaryPlaneScanoutRetireResult {
    let LiveRenderedPrimaryPlaneScanoutSubmission {
        scanout_buffer,
        primary_plane,
        submitted_after_page_flip_serial: _,
    } = submission;
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(card, primary_plane, callback);
    drop(scanout_buffer);
    retired
}

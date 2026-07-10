use super::super::select_real_atomic_scanout_card;
use super::RealAtomicScanoutSmokeConfig;
use crate::prelude::*;

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
    let initial = session.run_native_gbm_rendered_primary_plane_smoke_phase(
        LibdrmNativeAtomicScanoutSmokePhase::InitialModeset,
        &mut exporter,
        &mut intake,
        config.wait_policy,
    );
    if initial.status != LibdrmNativeAtomicScanoutSmokeStatus::Passed {
        return vec![initial];
    }

    let steady = session.run_native_gbm_rendered_primary_plane_smoke_phase(
        LibdrmNativeAtomicScanoutSmokePhase::SteadyPageFlip,
        &mut exporter,
        &mut intake,
        config.wait_policy,
    );
    vec![initial, steady]
}

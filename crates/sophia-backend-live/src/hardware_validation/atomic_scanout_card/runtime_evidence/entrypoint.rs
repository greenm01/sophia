use super::super::RealAtomicScanoutSmokeConfig;
use crate::prelude::*;

pub fn run_real_atomic_runtime_rendered_scanout_evidence_with(
    config: RealAtomicScanoutSmokeConfig,
) -> Vec<String> {
    let mut session_result = select_real_atomic_scanout_card().into_page_flip_session(
        config.slot,
        config.output,
        config.authority,
    );
    let Some(mut session) = session_result.session.take() else {
        return vec![
            session_result
                .failure_evidence()
                .unwrap_or_else(LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed)
                .reduced_log_line(),
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
                )
                .reduced_log_line(),
            ];
        }
    };
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery);
    session.run_runtime_rendered_scanout_evidence_lines(
        config.output,
        &mut exporter,
        config.wait_policy,
    )
}

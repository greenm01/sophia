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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealAtomicCpuFrameScanoutEvidence {
    pub lines: Vec<String>,
    pub requested_checksum: u64,
    pub requested_nonzero_pixel_bytes: usize,
    pub export_attempts: usize,
    pub exported_checksum: Option<u64>,
    pub export_status: Option<LiveRendererScanoutBufferExportStatus>,
    pub frame_pending: bool,
}

impl RealAtomicCpuFrameScanoutEvidence {
    pub fn frame_exported(&self) -> bool {
        self.export_attempts == 1
            && self.exported_checksum == Some(self.requested_checksum)
            && self.export_status == Some(LiveRendererScanoutBufferExportStatus::Exported)
            && !self.frame_pending
            && self.requested_nonzero_pixel_bytes > 0
    }
}

pub fn run_real_atomic_runtime_rendered_scanout_evidence_with_cpu_frame(
    config: RealAtomicScanoutSmokeConfig,
    frame: LiveCpuComposedFrame,
) -> RealAtomicCpuFrameScanoutEvidence {
    let requested_checksum = frame
        .bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        });
    let requested_nonzero_pixel_bytes = frame.bytes.iter().filter(|byte| **byte != 0).count();
    let mut session_result = select_real_atomic_scanout_card().into_page_flip_session(
        config.slot,
        config.output,
        config.authority,
    );
    let Some(mut session) = session_result.session.take() else {
        return RealAtomicCpuFrameScanoutEvidence {
            lines: vec![
                session_result
                    .failure_evidence()
                    .unwrap_or_else(LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed)
                    .reduced_log_line(),
            ],
            requested_checksum,
            requested_nonzero_pixel_bytes,
            export_attempts: 0,
            exported_checksum: None,
            export_status: None,
            frame_pending: true,
        };
    };
    let discovery = match session.render_device_discovery() {
        Ok(discovery) => discovery,
        Err(_) => {
            return RealAtomicCpuFrameScanoutEvidence {
                lines: vec![
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
                ],
                requested_checksum,
                requested_nonzero_pixel_bytes,
                export_attempts: 0,
                exported_checksum: None,
                export_status: None,
                frame_pending: true,
            };
        }
    };
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(discovery)
        .with_preferred_modifiers(session.preferred_xrgb8888_scanout_modifiers());
    exporter.set_pending_cpu_frame(frame);
    let lines = session.run_runtime_rendered_scanout_evidence_lines(
        config.output,
        &mut exporter,
        config.wait_policy,
    );
    RealAtomicCpuFrameScanoutEvidence {
        lines,
        requested_checksum,
        requested_nonzero_pixel_bytes,
        export_attempts: exporter.cpu_frame_export_attempts(),
        exported_checksum: exporter.last_cpu_frame_checksum(),
        export_status: exporter.last_cpu_frame_export_status(),
        frame_pending: exporter.pending_cpu_frame(),
    }
}

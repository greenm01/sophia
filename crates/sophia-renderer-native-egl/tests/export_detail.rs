#![cfg(feature = "gbm-platform")]

use sophia_renderer_native_egl::{
    NativeGbmScanoutBufferExportDetail, NativeGbmScanoutBufferExportStatus,
};

#[test]
fn mixed_composition_failure_stages_remain_reduced_and_degraded() {
    for detail in [
        NativeGbmScanoutBufferExportDetail::CpuLayerUploadFailed,
        NativeGbmScanoutBufferExportDetail::DmaBufImageCreateFailed,
        NativeGbmScanoutBufferExportDetail::DmaBufImageBindFailed,
        NativeGbmScanoutBufferExportDetail::CompositionDrawFailed,
        NativeGbmScanoutBufferExportDetail::CompositionFinishFailed,
        NativeGbmScanoutBufferExportDetail::EglImageDestroyFailed,
    ] {
        assert_eq!(
            detail.status(),
            NativeGbmScanoutBufferExportStatus::Degraded
        );
    }
}

#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
pub(crate) fn reduced_scanout_target_status_from_native_selection(
    current_status: LiveKmsScanoutTargetStatus,
    target: LiveGbmEglFrameTargetRecord,
    selection: &LibdrmNativePrimaryPlaneSelectionResult,
) -> LiveKmsScanoutTargetStatus {
    if current_status != LiveKmsScanoutTargetStatus::Ready {
        return current_status;
    }

    if target.status != LiveGbmEglFrameTargetStatus::Ready {
        return LiveKmsScanoutTargetStatus::InvalidFrameTarget;
    }

    let Some(selected) = selection.selection else {
        return LiveKmsScanoutTargetStatus::OutputUnavailable;
    };

    if selected.size() != target.size {
        return LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch;
    }

    LiveKmsScanoutTargetStatus::Ready
}

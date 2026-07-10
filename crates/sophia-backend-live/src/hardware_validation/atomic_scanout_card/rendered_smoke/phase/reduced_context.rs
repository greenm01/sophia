use crate::prelude::*;
use sophia_renderer_live::NativeGbmRenderedScanoutContextStatus;

pub(super) fn reduced_rendered_context_status_from_native(
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

#[cfg(feature = "libdrm-events")]
use super::*;
#[cfg(feature = "libdrm-events")]
use crate::prelude::*;
#[cfg(feature = "libdrm-events")]
use std::sync::mpsc::{SyncSender, TrySendError};

#[cfg(feature = "libdrm-events")]
pub fn decode_native_page_flip_batch(
    callbacks: &[LibdrmNativePageFlipCallback],
    routes: &[LibdrmNativeOutputRoute],
    sender: &SyncSender<LivePageFlipCallback>,
    max_decode: usize,
) -> LibdrmNativePageFlipBatchReport {
    let mut source_report = LivePageFlipCallbackSourceReport::default();
    let mut decoded_callbacks = 0usize;
    let mut rejected_callbacks = 0usize;
    let mut stopped_at = None;

    for (index, native) in callbacks.iter().take(max_decode).copied().enumerate() {
        let decode = native.decode(routes);
        let Some(callback) = decode.callback else {
            rejected_callbacks = rejected_callbacks.saturating_add(1);
            continue;
        };
        decoded_callbacks = decoded_callbacks.saturating_add(1);

        match sender.try_send(callback) {
            Ok(()) => {
                source_report.emitted = source_report.emitted.saturating_add(1);
            }
            Err(TrySendError::Full(_)) => {
                source_report.backpressure = true;
                stopped_at = Some(index);
                break;
            }
            Err(TrySendError::Disconnected(_)) => {
                source_report.disconnected = true;
                stopped_at = Some(index);
                break;
            }
        }
    }

    if let Some(index) = stopped_at {
        source_report.queued_remaining = callbacks.len().saturating_sub(index);
    }

    if callbacks.len() > max_decode {
        source_report.max_reached = true;
        source_report.queued_remaining = source_report
            .queued_remaining
            .max(callbacks.len() - max_decode);
    }

    LibdrmNativePageFlipBatchReport {
        read_loop: LibdrmNativeReadLoopReport::callbacks_decoded(
            decoded_callbacks,
            rejected_callbacks,
        )
        .unwrap_or_else(LibdrmNativeReadLoopReport::idle),
        poll: LibdrmPageFlipEventPollReport::from_source_report(source_report),
    }
}

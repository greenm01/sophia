use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutPageFlipWaitStatus {
    Retired,
    CallbackMissing,
    CallbackRejected,
    PollBackpressure,
    PollDisconnected,
    RetireMissing,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

impl LibdrmNativeAtomicScanoutPageFlipWaitStatus {
    pub(crate) fn from_reduced_reports(
        page_flip_poll: Option<LibdrmPageFlipEventPollStatus>,
        callback: Option<&LivePageFlipCallbackReport>,
        retire: Option<LibdrmNativePrimaryPlaneScanoutRetireStatus>,
        retire_destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
        retire_cleanup_pending: bool,
    ) -> Self {
        if page_flip_poll == Some(LibdrmPageFlipEventPollStatus::Disconnected) {
            return Self::PollDisconnected;
        }
        if page_flip_poll == Some(LibdrmPageFlipEventPollStatus::Backpressure) {
            return Self::PollBackpressure;
        }

        let Some(callback) = callback else {
            return Self::CallbackMissing;
        };
        if callback.decision != LivePageFlipCallbackDecision::Accepted
            || callback.event.status != LivePageFlipEventStatus::Presented
        {
            return Self::CallbackRejected;
        }

        match retire {
            Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip)
                if retire_destroy
                    == Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
                    && !retire_cleanup_pending =>
            {
                Self::Retired
            }
            Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip) => {
                Self::WaitingForAcceptedPageFlip
            }
            Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed) => {
                Self::ResourceRetireFailed
            }
            Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip) | None => {
                Self::RetireMissing
            }
        }
    }
}

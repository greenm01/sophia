#[cfg(feature = "libdrm-events")]
use crate::prelude::*;

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmPageFlipEventPollReport {
    pub status: LibdrmPageFlipEventPollStatus,
    pub callbacks: LivePageFlipCallbackSourceReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmPageFlipEventPollStatus {
    Idle,
    Emitted,
    Backpressure,
    Disconnected,
    EmitLimitReached,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmPageFlipEventPollReport {
    pub fn from_source_report(callbacks: LivePageFlipCallbackSourceReport) -> Self {
        let status = if callbacks.disconnected {
            LibdrmPageFlipEventPollStatus::Disconnected
        } else if callbacks.backpressure {
            LibdrmPageFlipEventPollStatus::Backpressure
        } else if callbacks.max_reached {
            LibdrmPageFlipEventPollStatus::EmitLimitReached
        } else if callbacks.emitted > 0 {
            LibdrmPageFlipEventPollStatus::Emitted
        } else {
            LibdrmPageFlipEventPollStatus::Idle
        };

        Self { status, callbacks }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeReadAndPollReport {
    pub read_loop: LibdrmNativeReadLoopReport,
    pub poll: LibdrmPageFlipEventPollReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePollerDiagnostics {
    pub route_count: usize,
    pub pending_callbacks: usize,
    pub last_read_loop: LibdrmNativeReadLoopReport,
}

#[cfg(feature = "libdrm-events")]
impl From<LibdrmNativePollerDiagnostics> for LiveLibdrmPollerDiagnostics {
    fn from(diagnostics: LibdrmNativePollerDiagnostics) -> Self {
        Self {
            status: diagnostics.last_read_loop.status.into(),
            route_count: diagnostics.route_count,
            pending_callbacks: diagnostics.pending_callbacks,
            decoded_callbacks: diagnostics.last_read_loop.decoded_callbacks,
            rejected_callbacks: diagnostics.last_read_loop.rejected_callbacks,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeReadLoopReport {
    pub status: LibdrmNativeReadLoopStatus,
    pub decoded_callbacks: usize,
    pub rejected_callbacks: usize,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativeReadLoopReport {
    pub const fn idle() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::Idle,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub const fn would_block() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::WouldBlock,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub const fn callbacks_decoded(
        decoded_callbacks: usize,
        rejected_callbacks: usize,
    ) -> Option<Self> {
        if decoded_callbacks == 0 && rejected_callbacks == 0 {
            return None;
        }

        Some(Self {
            status: if decoded_callbacks > 0 {
                LibdrmNativeReadLoopStatus::CallbackDecoded
            } else {
                LibdrmNativeReadLoopStatus::CallbackRejected
            },
            decoded_callbacks,
            rejected_callbacks,
        })
    }

    pub const fn callback_decoded(decoded_callbacks: usize) -> Option<Self> {
        Self::callbacks_decoded(decoded_callbacks, 0)
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LibdrmNativeReadLoopStatus::ReadFailed,
            decoded_callbacks: 0,
            rejected_callbacks: 0,
        }
    }

    pub fn into_poll_report(self) -> LibdrmPageFlipEventPollReport {
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: if matches!(self.status, LibdrmNativeReadLoopStatus::CallbackDecoded) {
                self.decoded_callbacks
            } else {
                0
            },
            queued_remaining: 0,
            backpressure: false,
            disconnected: matches!(self.status, LibdrmNativeReadLoopStatus::ReadFailed),
            max_reached: false,
        })
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeReadLoopStatus {
    Idle,
    WouldBlock,
    CallbackDecoded,
    CallbackRejected,
    ReadFailed,
}

#[cfg(feature = "libdrm-events")]
impl From<LibdrmNativeReadLoopStatus> for LiveLibdrmPollerDiagnosticsStatus {
    fn from(status: LibdrmNativeReadLoopStatus) -> Self {
        match status {
            LibdrmNativeReadLoopStatus::Idle => Self::Idle,
            LibdrmNativeReadLoopStatus::WouldBlock => Self::WouldBlock,
            LibdrmNativeReadLoopStatus::CallbackDecoded => Self::CallbackDecoded,
            LibdrmNativeReadLoopStatus::CallbackRejected => Self::CallbackRejected,
            LibdrmNativeReadLoopStatus::ReadFailed => Self::ReadFailed,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipBatchReport {
    pub read_loop: LibdrmNativeReadLoopReport,
    pub poll: LibdrmPageFlipEventPollReport,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipDecodeReport {
    pub status: LibdrmNativePageFlipDecodeStatus,
    pub callback: Option<LivePageFlipCallback>,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePageFlipDecodeStatus {
    Decoded,
    UnknownOutputSlot,
    InvalidFrameSerial,
}

use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokePhase {
    InitialModeset,
    SteadyPageFlip,
}

impl LibdrmNativeAtomicScanoutSmokePhase {
    pub const fn required_request_scope(self) -> LibdrmNativeAtomicCommitRequestScope {
        match self {
            Self::InitialModeset => LibdrmNativeAtomicCommitRequestScope::Modeset,
            Self::SteadyPageFlip => LibdrmNativeAtomicCommitRequestScope::PageFlip,
        }
    }

    pub const fn required_commit_flags(self) -> LibdrmNativeAtomicCommitFlagsReport {
        match self {
            Self::InitialModeset => LibdrmNativeAtomicCommitFlagsReport {
                page_flip_event: true,
                nonblocking: true,
                allow_modeset: true,
                test_only: false,
            },
            Self::SteadyPageFlip => LibdrmNativeAtomicCommitFlagsReport {
                page_flip_event: true,
                nonblocking: true,
                allow_modeset: false,
                test_only: false,
            },
        }
    }
}

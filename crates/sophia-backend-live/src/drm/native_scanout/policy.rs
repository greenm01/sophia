use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub allow_modeset: bool,
    pub page_flip_event: bool,
    pub nonblocking: bool,
}

impl LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub const fn page_flip() -> Self {
        Self {
            allow_modeset: false,
            page_flip_event: true,
            nonblocking: true,
        }
    }

    pub const fn modeset() -> Self {
        Self {
            allow_modeset: true,
            page_flip_event: true,
            nonblocking: true,
        }
    }

    pub const fn blocking_modeset() -> Self {
        Self {
            allow_modeset: true,
            page_flip_event: false,
            nonblocking: false,
        }
    }

    pub const fn expected_request_scope(self) -> LibdrmNativeAtomicCommitRequestScope {
        if self.allow_modeset {
            LibdrmNativeAtomicCommitRequestScope::Modeset
        } else {
            LibdrmNativeAtomicCommitRequestScope::PageFlip
        }
    }
}

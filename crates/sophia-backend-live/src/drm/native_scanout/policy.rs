use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub allow_modeset: bool,
}

impl LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub const fn page_flip() -> Self {
        Self {
            allow_modeset: false,
        }
    }

    pub const fn modeset() -> Self {
        Self {
            allow_modeset: true,
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

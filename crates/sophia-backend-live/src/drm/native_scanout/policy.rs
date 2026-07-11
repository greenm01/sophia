use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub allow_modeset: bool,
    pub page_flip_event: bool,
    pub nonblocking: bool,
    pub vrr_enabled: Option<bool>,
}

impl LibdrmNativePrimaryPlaneScanoutSubmitPolicy {
    pub const fn page_flip() -> Self {
        Self {
            allow_modeset: false,
            page_flip_event: true,
            nonblocking: true,
            vrr_enabled: None,
        }
    }

    pub const fn modeset() -> Self {
        Self {
            allow_modeset: true,
            page_flip_event: true,
            nonblocking: true,
            vrr_enabled: None,
        }
    }

    pub const fn blocking_modeset() -> Self {
        Self {
            allow_modeset: true,
            page_flip_event: false,
            nonblocking: false,
            vrr_enabled: None,
        }
    }

    pub const fn with_vrr_enabled(mut self, enabled: bool) -> Self {
        self.vrr_enabled = Some(enabled);
        self
    }

    pub const fn expected_request_scope(self) -> LibdrmNativeAtomicCommitRequestScope {
        if self.allow_modeset {
            LibdrmNativeAtomicCommitRequestScope::Modeset
        } else {
            LibdrmNativeAtomicCommitRequestScope::PageFlip
        }
    }
}

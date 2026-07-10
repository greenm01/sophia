#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmDependencyAdmissionReport {
    pub status: LibdrmDependencyAdmissionStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmDependencyAdmissionStatus {
    TypedPageFlipEventAvailable,
}

#[cfg(feature = "libdrm-events")]
pub fn libdrm_dependency_admission_report() -> LibdrmDependencyAdmissionReport {
    native_drm_admission::dependency_admission_report()
}

#[cfg(feature = "libdrm-events")]
pub fn native_libdrm_event_adapter_report() -> LibdrmNativeEventAdapterReport {
    native_libdrm_events::adapter_report()
}

#[cfg(feature = "libdrm-events")]
pub fn native_libdrm_event_adapter_report_for_authority(
    authority: LibdrmBackendFdAuthority,
) -> LibdrmNativeEventAdapterReport {
    native_libdrm_events::adapter_report_for_authority(authority)
}

#[cfg(feature = "libdrm-events")]
pub fn libdrm_fd_authority_report(
    authority: LibdrmBackendFdAuthority,
) -> LibdrmBackendFdAuthorityReport {
    native_libdrm_events::fd_authority_report(authority)
}

#[cfg(feature = "libdrm-events")]
mod native_drm_admission {
    use super::{LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus};

    pub(super) fn dependency_admission_report() -> LibdrmDependencyAdmissionReport {
        let _ = core::mem::size_of::<drm::control::PageFlipEvent>();
        LibdrmDependencyAdmissionReport {
            status: LibdrmDependencyAdmissionStatus::TypedPageFlipEventAvailable,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativeEventAdapterReport {
    pub status: LibdrmNativeEventAdapterStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeEventAdapterStatus {
    SkeletonReady,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipSource {
    _private: (),
}

#[cfg(feature = "libdrm-events")]
impl LibdrmNativePageFlipSource {
    pub fn from_authority(authority: LibdrmBackendFdAuthority) -> Self {
        native_libdrm_events::page_flip_source_from_authority(authority)
    }

    pub const fn report(&self) -> LibdrmNativePageFlipSourceReport {
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePageFlipSourceReport {
    pub status: LibdrmNativePageFlipSourceStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePageFlipSourceStatus {
    ConstructedWithoutPolling,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmBackendFdAuthority {
    generation: u64,
}

#[cfg(feature = "libdrm-events")]
impl LibdrmBackendFdAuthority {
    pub const fn new(generation: u64) -> Option<Self> {
        if generation == 0 {
            return None;
        }

        Some(Self { generation })
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmBackendFdAuthorityReport {
    pub status: LibdrmBackendFdAuthorityStatus,
}

#[cfg(feature = "libdrm-events")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmBackendFdAuthorityStatus {
    BackendOwned,
}

#[cfg(feature = "libdrm-events")]
mod native_libdrm_events {
    use super::{
        LibdrmBackendFdAuthority, LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
        LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus, LibdrmNativePageFlipSource,
    };

    pub(super) fn adapter_report() -> LibdrmNativeEventAdapterReport {
        let _ = core::mem::align_of::<drm::control::PageFlipEvent>();
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    }

    pub(super) fn adapter_report_for_authority(
        authority: LibdrmBackendFdAuthority,
    ) -> LibdrmNativeEventAdapterReport {
        let _ = fd_authority_report(authority);
        adapter_report()
    }

    pub(super) fn page_flip_source_from_authority(
        authority: LibdrmBackendFdAuthority,
    ) -> LibdrmNativePageFlipSource {
        let _ = fd_authority_report(authority);
        LibdrmNativePageFlipSource { _private: () }
    }

    pub(super) fn fd_authority_report(
        _authority: LibdrmBackendFdAuthority,
    ) -> LibdrmBackendFdAuthorityReport {
        LibdrmBackendFdAuthorityReport {
            status: LibdrmBackendFdAuthorityStatus::BackendOwned,
        }
    }
}

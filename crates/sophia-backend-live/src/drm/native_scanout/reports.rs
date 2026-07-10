use crate::prelude::*;

#[derive(Debug)]
pub struct LibdrmNativePrimaryPlaneScanoutSubmitResult {
    pub status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
    pub selection: LibdrmNativePrimaryPlaneSelectionStatus,
    pub scanout_buffer: LiveRendererScanoutBufferStatus,
    pub properties: Option<LibdrmNativePrimaryPlanePropertyDiscoveryStatus>,
    pub resources: Option<LibdrmNativePrimaryPlaneResourceCreateStatus>,
    pub request: Option<LibdrmNativeAtomicRequestBuildStatus>,
    pub request_scope: Option<LibdrmNativeAtomicCommitRequestScope>,
    pub commit_flags: Option<LibdrmNativeAtomicCommitFlagsReport>,
    pub submit: Option<LibdrmNativeAtomicCommitSubmitStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

impl LibdrmNativePrimaryPlaneScanoutSubmitResult {
    pub(crate) const fn new(
        status: LibdrmNativePrimaryPlaneScanoutSubmitStatus,
        selection: LibdrmNativePrimaryPlaneSelectionStatus,
        scanout_buffer: LiveRendererScanoutBufferStatus,
    ) -> Self {
        Self {
            status,
            selection,
            scanout_buffer,
            properties: None,
            resources: None,
            request: None,
            request_scope: None,
            commit_flags: None,
            submit: None,
            submission: None,
            cleanup: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutSubmitStatus {
    SubmittedWaitingForPageFlip,
    KmsTargetUnavailable,
    ScanoutBufferUnavailable,
    PropertyDiscoveryUnavailable,
    ResourceCreationUnavailable,
    AtomicRequestBuildFailed,
    AtomicSubmitFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibdrmNativePrimaryPlaneScanoutRetireResult {
    pub status: LibdrmNativePrimaryPlaneScanoutRetireStatus,
    pub destroy: Option<LibdrmNativePrimaryPlaneResourceDestroyStatus>,
    pub submission: Option<LibdrmNativePrimaryPlaneScanoutSubmission>,
    pub cleanup: Option<LibdrmNativePrimaryPlaneResourceCleanup>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePrimaryPlaneScanoutRetireStatus {
    RetiredAfterPageFlip,
    WaitingForAcceptedPageFlip,
    ResourceRetireFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativeAtomicScanoutSmokeStatus {
    Passed,
    SmokeChildTimeout,
    NoPrimaryCard,
    PrimaryCardOpenFailed,
    ClientCapabilityFailed,
    KmsSelectionFailed,
    KmsTargetUnavailable,
    RenderedContextUnavailable,
    GbmExportFailed,
    ScanoutBufferUnavailable,
    RetainedResourceMissing,
    PropertyDiscoveryFailed,
    ResourceCreationFailed,
    RequestBuildFailed,
    AtomicSubmitFailed,
    RequestShapeMismatch,
    PageFlipReaderUnavailable,
    PageFlipMissing,
    RetireFailed,
}

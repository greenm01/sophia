#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEglProbeStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEglDrawSmokeStatus {
    ClearColorReady,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
    SurfaceUnavailable,
    MakeCurrentUnavailable,
    GlUnavailable,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmBackedEglPlatformStatus {
    NativePlatformCapable,
    PlatformUnavailable,
    PlatformDegraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePresentationSmokeStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmEglFrameTargetAllocationStatus {
    Ready,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmScanoutBufferExportStatus {
    Exported,
    InvalidTarget,
    Unavailable,
    Degraded,
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmScanoutBufferExportDetail {
    Exported,
    InvalidTarget,
    BackendDeviceUnavailable,
    GbmDeviceUnavailable,
    EglUnavailable,
    EglDisplayUnavailable,
    EglInitializeFailed,
    EglBindApiFailed,
    EglConfigUnavailable,
    GbmSurfaceUnavailable,
    EglSurfaceUnavailable,
    EglContextUnavailable,
    EglMakeCurrentFailed,
    GlSmokeFailed,
    DmaBufImportFailed,
    EglSwapBuffersFailed,
    FrontBufferLockFailed,
    InvalidBufferDescriptor,
}

#[cfg(feature = "gbm-platform")]
impl NativeGbmScanoutBufferExportDetail {
    pub const fn status(self) -> NativeGbmScanoutBufferExportStatus {
        match self {
            Self::Exported => NativeGbmScanoutBufferExportStatus::Exported,
            Self::InvalidTarget => NativeGbmScanoutBufferExportStatus::InvalidTarget,
            Self::BackendDeviceUnavailable
            | Self::GbmDeviceUnavailable
            | Self::EglUnavailable
            | Self::EglDisplayUnavailable
            | Self::GbmSurfaceUnavailable => NativeGbmScanoutBufferExportStatus::Unavailable,
            Self::EglInitializeFailed
            | Self::EglBindApiFailed
            | Self::EglConfigUnavailable
            | Self::EglSurfaceUnavailable
            | Self::EglContextUnavailable
            | Self::EglMakeCurrentFailed
            | Self::GlSmokeFailed
            | Self::DmaBufImportFailed
            | Self::EglSwapBuffersFailed
            | Self::FrontBufferLockFailed
            | Self::InvalidBufferDescriptor => NativeGbmScanoutBufferExportStatus::Degraded,
        }
    }
}

#[cfg(feature = "gbm-platform")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGbmRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

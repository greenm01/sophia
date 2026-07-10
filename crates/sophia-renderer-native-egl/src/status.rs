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
pub enum NativeGbmRenderedScanoutContextStatus {
    Ready,
    Unavailable,
    Degraded,
}

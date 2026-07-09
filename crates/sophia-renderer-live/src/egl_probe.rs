#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeEglCapabilityProbe {
    pub platform: EglPlatformStatus,
    pub context: EglContextProbeStatus,
}

impl FakeEglCapabilityProbe {
    pub const fn new(platform: EglPlatformStatus, context: EglContextProbeStatus) -> Self {
        Self { platform, context }
    }

    pub const fn probe_report(self) -> EglCapabilityProbeReport {
        EglCapabilityProbeReport {
            status: match (self.platform, self.context) {
                (EglPlatformStatus::NativePlatformCapable, EglContextProbeStatus::Available) => {
                    EglCapabilityProbeStatus::NativeDrawingCapable
                }
                (EglPlatformStatus::NativePlatformCapable, EglContextProbeStatus::Unavailable) => {
                    EglCapabilityProbeStatus::ContextUnavailable
                }
                (EglPlatformStatus::PlatformUnavailable, _) => {
                    EglCapabilityProbeStatus::PlatformUnavailable
                }
                (EglPlatformStatus::PlatformDegraded, _) => {
                    EglCapabilityProbeStatus::PlatformDegraded
                }
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeEglCapabilityProbe;

impl NativeEglCapabilityProbe {
    pub fn probe_report() -> EglCapabilityProbeReport {
        report_from_probe_result(native::probe())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EglCapabilityProbeReport {
    pub status: EglCapabilityProbeStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglCapabilityProbeStatus {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglPlatformStatus {
    NativePlatformCapable,
    PlatformUnavailable,
    PlatformDegraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EglContextProbeStatus {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EglProbeResult {
    NativeDrawingCapable,
    PlatformUnavailable,
    PlatformDegraded,
    ContextUnavailable,
}

fn report_from_probe_result(result: EglProbeResult) -> EglCapabilityProbeReport {
    EglCapabilityProbeReport {
        status: match result {
            EglProbeResult::NativeDrawingCapable => EglCapabilityProbeStatus::NativeDrawingCapable,
            EglProbeResult::PlatformUnavailable => EglCapabilityProbeStatus::PlatformUnavailable,
            EglProbeResult::PlatformDegraded => EglCapabilityProbeStatus::PlatformDegraded,
            EglProbeResult::ContextUnavailable => EglCapabilityProbeStatus::ContextUnavailable,
        },
    }
}

mod native {
    use super::EglProbeResult;

    pub(super) fn probe() -> EglProbeResult {
        match sophia_renderer_native_egl::probe_default_display_context() {
            sophia_renderer_native_egl::NativeEglProbeStatus::NativeDrawingCapable => {
                EglProbeResult::NativeDrawingCapable
            }
            sophia_renderer_native_egl::NativeEglProbeStatus::PlatformUnavailable => {
                EglProbeResult::PlatformUnavailable
            }
            sophia_renderer_native_egl::NativeEglProbeStatus::PlatformDegraded => {
                EglProbeResult::PlatformDegraded
            }
            sophia_renderer_native_egl::NativeEglProbeStatus::ContextUnavailable => {
                EglProbeResult::ContextUnavailable
            }
        }
    }
}

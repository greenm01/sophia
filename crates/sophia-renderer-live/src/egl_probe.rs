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

//! Live renderer boundary.
//!
//! This crate is the future home for renderer-private resources such as GBM,
//! EGL, DMA-BUF import, explicit sync fences, and upload caches. The current
//! implementation is dependency-free beyond Sophia's own data crates and only
//! models reduced import admission.

pub use sophia_engine::BufferImportPath;
pub use sophia_protocol::BufferSource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererImportBoundary {
    pub import_xpixmap: bool,
    pub import_dmabuf: bool,
}

impl LiveRendererImportBoundary {
    pub const fn cpu_only() -> Self {
        Self {
            import_xpixmap: false,
            import_dmabuf: false,
        }
    }

    pub const fn with_native_imports(import_xpixmap: bool, import_dmabuf: bool) -> Self {
        Self {
            import_xpixmap,
            import_dmabuf,
        }
    }

    pub fn decide(self, source: BufferSource) -> LiveRendererImportDecision {
        match source {
            BufferSource::None => LiveRendererImportDecision::Rejected {
                reason: LiveRendererImportRejection::EmptySource,
            },
            BufferSource::CpuBuffer { .. } => LiveRendererImportDecision::Accepted {
                path: BufferImportPath::CpuReadback,
            },
            BufferSource::XPixmap { .. } if self.import_xpixmap => {
                LiveRendererImportDecision::Accepted {
                    path: BufferImportPath::XPixmap,
                }
            }
            BufferSource::DmaBuf { .. } if self.import_dmabuf => {
                LiveRendererImportDecision::Accepted {
                    path: BufferImportPath::DmaBuf,
                }
            }
            BufferSource::XPixmap { .. } => LiveRendererImportDecision::Deferred {
                requested: BufferImportPath::XPixmap,
                required_boundary: "live XPixmap renderer import",
            },
            BufferSource::DmaBuf { .. } => LiveRendererImportDecision::Deferred {
                requested: BufferImportPath::DmaBuf,
                required_boundary: "live DMA-BUF renderer import",
            },
        }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        LiveRendererImportStartupStatus::from_path_statuses(
            if self.import_xpixmap {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Disabled
            },
            if self.import_dmabuf {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Disabled
            },
        )
    }
}

impl Default for LiveRendererImportBoundary {
    fn default() -> Self {
        Self::cpu_only()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportDecision {
    Accepted {
        path: BufferImportPath,
    },
    Deferred {
        requested: BufferImportPath,
        required_boundary: &'static str,
    },
    Rejected {
        reason: LiveRendererImportRejection,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportRejection {
    EmptySource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererImportStartupStatus {
    pub health: LiveRendererImportHealth,
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
}

impl LiveRendererImportStartupStatus {
    pub fn from_path_statuses(
        xpixmap: LiveRendererImportPathStatus,
        dmabuf: LiveRendererImportPathStatus,
    ) -> Self {
        Self {
            health: renderer_health_from_path_statuses(xpixmap, dmabuf),
            xpixmap,
            dmabuf,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportHealth {
    CpuFallback,
    NativeImportCapable,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererImportPathStatus {
    Disabled,
    Enabled,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeLiveRendererCapabilityProbe {
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
}

impl FakeLiveRendererCapabilityProbe {
    pub const fn new(
        xpixmap: LiveRendererImportPathStatus,
        dmabuf: LiveRendererImportPathStatus,
    ) -> Self {
        Self { xpixmap, dmabuf }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        LiveRendererImportStartupStatus::from_path_statuses(self.xpixmap, self.dmabuf)
    }
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeGbmCapabilityProbe {
    pub available: bool,
}

#[cfg(feature = "gbm-probe")]
impl FakeGbmCapabilityProbe {
    pub const fn new(available: bool) -> Self {
        Self { available }
    }

    pub fn startup_status(self) -> LiveRendererImportStartupStatus {
        LiveRendererImportStartupStatus::from_path_statuses(
            LiveRendererImportPathStatus::Disabled,
            if self.available {
                LiveRendererImportPathStatus::Enabled
            } else {
                LiveRendererImportPathStatus::Degraded
            },
        )
    }
}

fn renderer_health_from_path_statuses(
    xpixmap: LiveRendererImportPathStatus,
    dmabuf: LiveRendererImportPathStatus,
) -> LiveRendererImportHealth {
    match (xpixmap, dmabuf) {
        (LiveRendererImportPathStatus::Degraded, _)
        | (_, LiveRendererImportPathStatus::Degraded) => LiveRendererImportHealth::Degraded,
        (LiveRendererImportPathStatus::Enabled, _) | (_, LiveRendererImportPathStatus::Enabled) => {
            LiveRendererImportHealth::NativeImportCapable
        }
        (LiveRendererImportPathStatus::Disabled, LiveRendererImportPathStatus::Disabled) => {
            LiveRendererImportHealth::CpuFallback
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererRuntimeObservation {
    pub health: LiveRendererImportHealth,
    pub xpixmap: LiveRendererImportPathStatus,
    pub dmabuf: LiveRendererImportPathStatus,
    pub selection: LiveRendererSelectionObservation,
}

impl LiveRendererRuntimeObservation {
    pub fn from_startup_status(
        status: LiveRendererImportStartupStatus,
        selection: LiveRendererSelectionObservation,
    ) -> Self {
        Self {
            health: status.health,
            xpixmap: status.xpixmap,
            dmabuf: status.dmabuf,
            selection,
        }
    }

    pub fn degraded_by_failed_import(self, requested: BufferImportPath) -> Self {
        let mut degraded = Self {
            health: LiveRendererImportHealth::Degraded,
            ..self
        };

        match requested {
            BufferImportPath::XPixmap => {
                degraded.xpixmap = LiveRendererImportPathStatus::Degraded;
            }
            BufferImportPath::DmaBuf => {
                degraded.dmabuf = LiveRendererImportPathStatus::Degraded;
            }
            BufferImportPath::CpuReadback => {}
        }

        degraded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererSelectionObservation {
    CpuFallback,
    NativeImportCapable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyKind {
    LibDrm,
    LibInput,
    Gbm,
    Egl,
    DmaBuf,
    Wgpu,
    MitShm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyUse {
    Discovery,
    RuntimePolling,
    RendererImport,
    SharedMemoryImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBackendDependencyDecision {
    Allowed,
    Deferred { required_boundary: &'static str },
}

impl LiveBackendDependencyDecision {
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

pub fn live_backend_dependency_decision(
    kind: LiveBackendDependencyKind,
    use_case: LiveBackendDependencyUse,
) -> LiveBackendDependencyDecision {
    use LiveBackendDependencyDecision::{Allowed, Deferred};
    use LiveBackendDependencyKind::{DmaBuf, Egl, Gbm, LibDrm, LibInput, MitShm, Wgpu};
    use LiveBackendDependencyUse::{Discovery, RendererImport, RuntimePolling, SharedMemoryImport};

    match (kind, use_case) {
        (LibDrm | LibInput, Discovery | RuntimePolling) => Allowed,
        (MitShm, _) | (_, SharedMemoryImport) => Deferred {
            required_boundary: "bounded shared-memory import boundary",
        },
        (Wgpu, _) => Deferred {
            required_boundary: "validated GBM/EGL startup, drawing, and presentation seams",
        },
        (Gbm | Egl | DmaBuf, _) | (_, RendererImport) => Deferred {
            required_boundary: "live renderer import boundary",
        },
    }
}

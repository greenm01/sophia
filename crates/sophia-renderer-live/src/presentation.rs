#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakePresentationSmoke {
    pub status: LiveRendererPresentationStatus,
}

impl FakePresentationSmoke {
    pub const fn new(status: LiveRendererPresentationStatus) -> Self {
        Self { status }
    }

    pub const fn smoke_report(self) -> LiveRendererPresentationReport {
        LiveRendererPresentationReport {
            status: self.status,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRendererPresentationReport {
    pub status: LiveRendererPresentationStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRendererPresentationStatus {
    Ready,
    Unavailable,
    Degraded,
}

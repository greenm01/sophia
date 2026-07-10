#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveRuntimeRenderedScanoutEvidenceFailureReport {
    pub status: LiveRuntimeRenderedScanoutEvidenceFailureStatus,
    pub submit_seen: bool,
    pub retire_seen: bool,
}

impl LiveRuntimeRenderedScanoutEvidenceFailureReport {
    pub const fn new(
        status: LiveRuntimeRenderedScanoutEvidenceFailureStatus,
        submit_seen: bool,
        retire_seen: bool,
    ) -> Self {
        Self {
            status,
            submit_seen,
            retire_seen,
        }
    }

    pub fn reduced_log_line(&self) -> String {
        format!(
            "sophia_runtime_rendered_scanout_failure schema=1 status={:?} submit_seen={} retire_seen={}",
            self.status, self.submit_seen, self.retire_seen,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveRuntimeRenderedScanoutEvidenceFailureStatus {
    InitialTickFailed,
    SubmitReportMissing,
    RetireTickFailed,
    RetireTimedOut,
}

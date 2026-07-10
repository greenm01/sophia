use crate::prelude::*;

pub trait LibdrmNativeAtomicCommitDevice {
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()>;
}

impl<D> LibdrmNativeAtomicCommitDevice for D
where
    D: drm::control::Device,
{
    fn submit_atomic_commit(
        &self,
        flags: drm::control::AtomicCommitFlags,
        request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.atomic_commit(flags, request)
    }
}

#[derive(Debug)]
pub struct NativeLibdrmAtomicScanoutCommitter<D> {
    device: D,
    submitted: usize,
    rejected: usize,
}

impl<D> NativeLibdrmAtomicScanoutCommitter<D> {
    pub const fn new(device: D) -> Self {
        Self {
            device,
            submitted: 0,
            rejected: 0,
        }
    }

    pub const fn submitted_count(&self) -> usize {
        self.submitted
    }

    pub const fn rejected_count(&self) -> usize {
        self.rejected
    }
}

impl<D> NativeLibdrmAtomicScanoutCommitter<D>
where
    D: LibdrmNativeAtomicCommitDevice,
{
    pub fn submit_native_atomic_commit(
        &mut self,
        request: LibdrmNativeAtomicCommitRequest,
    ) -> LibdrmNativeAtomicCommitSubmitReport {
        let (flags, request) = request.into_native();
        match self.device.submit_atomic_commit(flags, request) {
            Ok(()) => {
                self.submitted = self.submitted.saturating_add(1);
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::WouldBlock,
                }
            }
            Err(_) => {
                self.rejected = self.rejected.saturating_add(1);
                LibdrmNativeAtomicCommitSubmitReport {
                    status: LibdrmNativeAtomicCommitSubmitStatus::Rejected,
                }
            }
        }
    }
}

impl<D> LiveAtomicScanoutCommitter for NativeLibdrmAtomicScanoutCommitter<D> {
    fn commit_atomic_scanout(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome)
    }

    fn commit_atomic_scanout_after_page_flip(
        &mut self,
        callback: &LivePageFlipCallbackReport,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        LiveAtomicScanoutCommitReport::from_page_flip_callback_and_outcome(callback, outcome)
    }
}

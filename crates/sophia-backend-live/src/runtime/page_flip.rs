use super::*;

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn with_page_flip_callback_queue(mut self, queue: LivePageFlipCallbackQueue) -> Self {
        self.page_flip_callback_queue = Some(queue);
        self
    }

    pub fn page_flip_observation(&self) -> LivePageFlipEvent {
        self.page_flip_event
    }

    pub fn observe_page_flip_outcome(&mut self, outcome: &PageFlipCommitOutcome) {
        self.page_flip_event = LivePageFlipEvent::from_commit_outcome(outcome);
    }

    pub fn observe_atomic_scanout_commit(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report = LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_with<C>(
        &mut self,
        committer: &mut C,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let report = committer.commit_atomic_scanout(outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn commit_atomic_scanout_after_page_flip_with<C>(
        &mut self,
        committer: &mut C,
        callback: LivePageFlipCallback,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport
    where
        C: LiveAtomicScanoutCommitter,
    {
        let callback_report = self.page_flip_callback_intake.observe(callback);
        let report = committer.commit_atomic_scanout_after_page_flip(&callback_report, outcome);
        self.page_flip_event = report.page_flip;
        report
    }

    pub fn observe_page_flip_callback(
        &mut self,
        callback: LivePageFlipCallback,
    ) -> LivePageFlipCallbackReport {
        let report = self.page_flip_callback_intake.observe(callback);
        self.page_flip_event = report.event;
        report
    }

    pub(crate) fn drain_page_flip_callback_queue(&mut self) -> LivePageFlipCallbackQueueReport {
        self.page_flip_callback_queue
            .as_ref()
            .map(|queue| {
                queue.drain_ready(
                    &mut self.page_flip_callback_intake,
                    &mut self.page_flip_event,
                )
            })
            .unwrap_or_default()
    }
}

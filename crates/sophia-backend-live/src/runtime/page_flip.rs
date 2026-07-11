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
        self.primary_output_state().page_flip_event
    }

    pub fn observe_page_flip_outcome(&mut self, outcome: &PageFlipCommitOutcome) {
        self.primary_output_state_mut().page_flip_event =
            LivePageFlipEvent::from_commit_outcome(outcome);
    }

    pub fn observe_atomic_scanout_commit(
        &mut self,
        outcome: &PageFlipCommitOutcome,
    ) -> LiveAtomicScanoutCommitReport {
        let report = LiveAtomicScanoutCommitReport::from_page_flip_outcome(outcome);
        self.primary_output_state_mut().page_flip_event = report.page_flip;
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
        self.primary_output_state_mut().page_flip_event = report.page_flip;
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
        let callback_report = self.observe_page_flip_callback(callback);
        let report = committer.commit_atomic_scanout_after_page_flip(&callback_report, outcome);
        if let Some(state) = self.outputs.get_mut(callback.output) {
            state.page_flip_event = report.page_flip;
        }
        report
    }

    pub fn observe_page_flip_callback(
        &mut self,
        callback: LivePageFlipCallback,
    ) -> LivePageFlipCallbackReport {
        let Some(state) = self.outputs.get_mut(callback.output) else {
            return LivePageFlipCallbackReport {
                decision: LivePageFlipCallbackDecision::RejectedUnexpectedOutput,
                event: LivePageFlipEvent {
                    status: LivePageFlipEventStatus::WaitingForOutput,
                    frame_serial: None,
                },
            };
        };
        let report = state.page_flip_callback_intake.observe(callback);
        state.page_flip_event = report.event;
        report
    }

    pub(crate) fn drain_page_flip_callback_queue(&mut self) -> LivePageFlipCallbackQueueReport {
        let Some(queue) = self.page_flip_callback_queue.take() else {
            return LivePageFlipCallbackQueueReport::default();
        };
        let mut last_accepted_output = None;
        let report = queue.drain_ready_with(|callback| {
            let output = callback.output;
            let report = self.observe_page_flip_callback(callback);
            if report.decision == LivePageFlipCallbackDecision::Accepted {
                last_accepted_output = Some(output);
            }
            report
        });
        if let (Some(output), Some(accepted)) = (last_accepted_output, report.last_accepted)
            && let Some(state) = self.outputs.get_mut(output)
        {
            state.page_flip_event = accepted.event;
        }
        self.page_flip_callback_queue = Some(queue);
        report
    }
}

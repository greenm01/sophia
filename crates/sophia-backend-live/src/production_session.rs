use sophia_engine::{
    DrmKmsOutputRegistry, OutputPresentationFeedback, OutputPresentationRegistry,
    OutputPresentationRetire, OutputPresentationSchedule, ProductionPresentationAdapter,
    ProductionRetirement,
};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_protocol::TransactionId;
use sophia_protocol::{CommittedSurfaceState, OutputId};
use std::collections::{BTreeMap, VecDeque};

pub struct LiveProductionPresentationAdapter<Compose, Submit, Retire, Feedback> {
    compose: Compose,
    submit: Submit,
    retire: Retire,
    feedback: Feedback,
}

impl<Compose, Submit, Retire, Feedback>
    LiveProductionPresentationAdapter<Compose, Submit, Retire, Feedback>
{
    pub const fn new(compose: Compose, submit: Submit, retire: Retire, feedback: Feedback) -> Self {
        Self {
            compose,
            submit,
            retire,
            feedback,
        }
    }
}

impl<Compose, Submit, Retire, Feedback, Frame, Submission, Retirement, Evidence, Error>
    ProductionPresentationAdapter
    for LiveProductionPresentationAdapter<Compose, Submit, Retire, Feedback>
where
    Compose: FnMut(u64, &[CommittedSurfaceState]) -> Result<Frame, Error>,
    Submit: FnMut(u64, Frame) -> Result<Submission, Error>,
    Retire: FnMut() -> Result<Vec<ProductionRetirement<Retirement>>, Error>,
    Feedback: FnMut(u64, Retirement) -> Result<Evidence, Error>,
{
    type Frame = Frame;
    type Submission = Submission;
    type Retirement = Retirement;
    type Evidence = Evidence;
    type Error = Error;

    fn compose(
        &mut self,
        cycle: u64,
        committed: &[CommittedSurfaceState],
    ) -> Result<Self::Frame, Self::Error> {
        (self.compose)(cycle, committed)
    }

    fn submit_frame(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Submission, Self::Error> {
        (self.submit)(cycle, frame)
    }

    fn poll_retirements(
        &mut self,
    ) -> Result<Vec<ProductionRetirement<Self::Retirement>>, Self::Error> {
        (self.retire)()
    }

    fn route_protocol_feedback(
        &mut self,
        cycle: u64,
        retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error> {
        (self.feedback)(cycle, retirement)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionPageFlipRetirement {
    pub output: OutputId,
    pub ust: u64,
    pub msc: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveProductionPageFlipTrackerError {
    Schedule(OutputPresentationSchedule),
    Feedback(OutputPresentationFeedback),
    Retirement(OutputPresentationRetire),
    MissingCycle { output: OutputId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProductionPageFlipTracker {
    presentation: OutputPresentationRegistry,
    pending: BTreeMap<OutputId, (u64, u64)>,
    retirements: VecDeque<ProductionRetirement<LiveProductionPageFlipRetirement>>,
}

impl LiveProductionPageFlipTracker {
    pub fn from_outputs(outputs: &DrmKmsOutputRegistry) -> Self {
        Self {
            presentation: OutputPresentationRegistry::from_outputs(outputs),
            pending: BTreeMap::new(),
            retirements: VecDeque::new(),
        }
    }

    pub fn submit(
        &mut self,
        output: OutputId,
        cycle: u64,
    ) -> Result<u64, LiveProductionPageFlipTrackerError> {
        let _ = self.presentation.mark_damage(output);
        match self.presentation.schedule(output) {
            OutputPresentationSchedule::Scheduled(frame) => {
                self.pending.insert(output, (cycle, frame.frame_serial));
                Ok(frame.frame_serial)
            }
            outcome => Err(LiveProductionPageFlipTrackerError::Schedule(outcome)),
        }
    }

    pub fn observe_page_flip(
        &mut self,
        output: OutputId,
        sequence: u64,
        presentation_msec: u64,
        ust: u64,
    ) -> Result<(), LiveProductionPageFlipTrackerError> {
        let (cycle, frame_serial) = self
            .pending
            .get(&output)
            .copied()
            .ok_or(LiveProductionPageFlipTrackerError::MissingCycle { output })?;
        match self
            .presentation
            .observe_page_flip(output, sequence, presentation_msec)
        {
            OutputPresentationFeedback::Accepted { .. } => {}
            outcome => return Err(LiveProductionPageFlipTrackerError::Feedback(outcome)),
        }
        match self.presentation.retire(output, frame_serial) {
            OutputPresentationRetire::Retired { .. } => {}
            outcome => return Err(LiveProductionPageFlipTrackerError::Retirement(outcome)),
        }
        self.pending.remove(&output);
        self.retirements.push_back(ProductionRetirement {
            cycle,
            retirement: LiveProductionPageFlipRetirement {
                output,
                ust,
                msc: sequence,
            },
        });
        Ok(())
    }

    pub fn drain_retirements(
        &mut self,
    ) -> Vec<ProductionRetirement<LiveProductionPageFlipRetirement>> {
        self.retirements.drain(..).collect()
    }

    pub fn take_retirement(
        &mut self,
        output: OutputId,
    ) -> Option<ProductionRetirement<LiveProductionPageFlipRetirement>> {
        let index = self
            .retirements
            .iter()
            .position(|retirement| retirement.retirement.output == output)?;
        self.retirements.remove(index)
    }

    pub fn discard_retirements(&mut self, output: Option<OutputId>) {
        match output {
            Some(output) => self
                .retirements
                .retain(|retirement| retirement.retirement.output != output),
            None => self.retirements.clear(),
        }
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePresentCompletionMode {
    Flip,
    Skip,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePresentProtocolFeedback {
    Complete {
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: LivePresentCompletionMode,
    },
    Idle {
        transaction: TransactionId,
    },
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePresentFeedbackOutcome {
    pub feedback: [LivePresentProtocolFeedback; 2],
    pub idle_fence_triggered: bool,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LivePresentFeedbackError {
    UnknownPresentation { transaction: TransactionId },
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
#[derive(Debug, Default)]
pub struct LiveProductionPresentFeedbackCoordinator {
    resources: crate::LivePresentationResourceSession,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl LiveProductionPresentFeedbackCoordinator {
    pub fn resources(&self) -> &crate::LivePresentationResourceSession {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut crate::LivePresentationResourceSession {
        &mut self.resources
    }

    pub fn complete_flip(
        &mut self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
    ) -> Result<LivePresentFeedbackOutcome, LivePresentFeedbackError> {
        let retirement = self
            .resources
            .retire_page_flip(transaction)
            .ok_or(LivePresentFeedbackError::UnknownPresentation { transaction })?;
        Ok(Self::outcome(
            transaction,
            ust,
            msc,
            LivePresentCompletionMode::Flip,
            retirement.idle_fence == sophia_renderer_live::LiveIdleFenceStatus::Triggered,
        ))
    }

    pub fn reject_skip(
        &mut self,
        transaction: TransactionId,
        ust: u64,
        msc: u64,
    ) -> Result<LivePresentFeedbackOutcome, LivePresentFeedbackError> {
        let retirement = self
            .resources
            .reject(transaction)
            .ok_or(LivePresentFeedbackError::UnknownPresentation { transaction })?;
        Ok(Self::outcome(
            transaction,
            ust,
            msc,
            LivePresentCompletionMode::Skip,
            retirement.idle_fence == sophia_renderer_live::LiveIdleFenceStatus::Triggered,
        ))
    }

    pub fn disconnect(&mut self) -> sophia_renderer_live::LivePresentationDisconnectReport {
        self.resources.disconnect()
    }

    fn outcome(
        transaction: TransactionId,
        ust: u64,
        msc: u64,
        mode: LivePresentCompletionMode,
        idle_fence_triggered: bool,
    ) -> LivePresentFeedbackOutcome {
        LivePresentFeedbackOutcome {
            feedback: [
                LivePresentProtocolFeedback::Complete {
                    transaction,
                    ust,
                    msc,
                    mode,
                },
                LivePresentProtocolFeedback::Idle { transaction },
            ],
            idle_fence_triggered,
        }
    }
}

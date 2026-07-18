use sophia_engine::{ProductionPresentationAdapter, ProductionRetirement};
use sophia_protocol::CommittedSurfaceState;

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

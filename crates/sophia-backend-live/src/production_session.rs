use sophia_engine::ProductionPresentationAdapter;
use sophia_protocol::CommittedSurfaceState;

pub struct LiveProductionPresentationAdapter<Compose, SubmitRetire, Feedback> {
    compose: Compose,
    submit_retire: SubmitRetire,
    feedback: Feedback,
}

impl<Compose, SubmitRetire, Feedback>
    LiveProductionPresentationAdapter<Compose, SubmitRetire, Feedback>
{
    pub const fn new(compose: Compose, submit_retire: SubmitRetire, feedback: Feedback) -> Self {
        Self {
            compose,
            submit_retire,
            feedback,
        }
    }
}

impl<Compose, SubmitRetire, Feedback, Frame, Retirement, Evidence, Error>
    ProductionPresentationAdapter
    for LiveProductionPresentationAdapter<Compose, SubmitRetire, Feedback>
where
    Compose: FnMut(u64, &[CommittedSurfaceState]) -> Result<Frame, Error>,
    SubmitRetire: FnMut(u64, Frame) -> Result<Retirement, Error>,
    Feedback: FnMut(u64, Retirement) -> Result<Evidence, Error>,
{
    type Frame = Frame;
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

    fn submit_and_retire(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Retirement, Self::Error> {
        (self.submit_retire)(cycle, frame)
    }

    fn route_protocol_feedback(
        &mut self,
        cycle: u64,
        retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error> {
        (self.feedback)(cycle, retirement)
    }
}

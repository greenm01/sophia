use crate::{AuthorityTransactionIntake, HeadlessEngine};
use sophia_protocol::{CommittedSurfaceState, TransactionCommit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSessionPhase {
    AuthorityIntake,
    EngineCommitPreparation,
    FrameComposition,
    KmsSubmitRetire,
    ProtocolFeedback,
}

pub trait ProductionPresentationAdapter {
    type Frame;
    type Retirement;
    type Evidence;
    type Error;

    fn compose(
        &mut self,
        cycle: u64,
        committed: &[CommittedSurfaceState],
    ) -> Result<Self::Frame, Self::Error>;

    fn submit_and_retire(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Retirement, Self::Error>;

    fn route_protocol_feedback(
        &mut self,
        cycle: u64,
        retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionSessionCycleReport<Evidence> {
    pub cycle: u64,
    pub authority_commits: Vec<TransactionCommit>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
    pub evidence: Evidence,
}

#[derive(Debug, PartialEq)]
pub struct ProductionSessionCycleError<Error> {
    pub cycle: u64,
    pub phase: ProductionSessionPhase,
    pub source: Error,
}

#[derive(Clone, Debug)]
pub struct ProductionSessionCoordinator {
    engine: HeadlessEngine,
    committed_surfaces: Vec<CommittedSurfaceState>,
    next_cycle: u64,
}

impl ProductionSessionCoordinator {
    pub fn new(engine: HeadlessEngine) -> Self {
        Self {
            engine,
            committed_surfaces: Vec::new(),
            next_cycle: 1,
        }
    }

    pub fn with_committed_surfaces(
        mut self,
        committed_surfaces: Vec<CommittedSurfaceState>,
    ) -> Self {
        self.committed_surfaces = committed_surfaces;
        self
    }

    pub fn engine(&self) -> &HeadlessEngine {
        &self.engine
    }

    pub fn committed_surfaces(&self) -> &[CommittedSurfaceState] {
        &self.committed_surfaces
    }

    pub fn replace_committed_surfaces(&mut self, committed_surfaces: Vec<CommittedSurfaceState>) {
        self.committed_surfaces = committed_surfaces;
    }

    /// Commits one bounded authority intake phase and retains the resulting
    /// immutable visual snapshot for composition and per-output projection.
    pub fn commit_authority_batches(
        &mut self,
        authority_batches: &[AuthorityTransactionIntake],
    ) -> Vec<TransactionCommit> {
        authority_batches
            .iter()
            .map(|batch| batch.commit(&self.engine, &mut self.committed_surfaces))
            .collect()
    }

    pub(crate) fn engine_and_committed_surfaces_mut(
        &mut self,
    ) -> (&HeadlessEngine, &mut Vec<CommittedSurfaceState>) {
        (&self.engine, &mut self.committed_surfaces)
    }

    pub fn run_cycle<A>(
        &mut self,
        authority_batches: &[AuthorityTransactionIntake],
        adapter: &mut A,
    ) -> Result<ProductionSessionCycleReport<A::Evidence>, ProductionSessionCycleError<A::Error>>
    where
        A: ProductionPresentationAdapter,
    {
        let cycle = self.next_cycle;
        self.next_cycle = self.next_cycle.saturating_add(1);

        let authority_commits = self.commit_authority_batches(authority_batches);

        let frame = adapter
            .compose(cycle, &self.committed_surfaces)
            .map_err(|source| ProductionSessionCycleError {
                cycle,
                phase: ProductionSessionPhase::FrameComposition,
                source,
            })?;
        let retirement = adapter.submit_and_retire(cycle, frame).map_err(|source| {
            ProductionSessionCycleError {
                cycle,
                phase: ProductionSessionPhase::KmsSubmitRetire,
                source,
            }
        })?;
        let evidence = adapter
            .route_protocol_feedback(cycle, retirement)
            .map_err(|source| ProductionSessionCycleError {
                cycle,
                phase: ProductionSessionPhase::ProtocolFeedback,
                source,
            })?;

        Ok(ProductionSessionCycleReport {
            cycle,
            authority_commits,
            committed_surfaces: self.committed_surfaces.clone(),
            evidence,
        })
    }
}

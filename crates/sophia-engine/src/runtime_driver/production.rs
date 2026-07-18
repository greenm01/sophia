use crate::{AuthorityTransactionIntake, HeadlessEngine, PreparedSurfaceCommit};
use sophia_protocol::{
    CommittedSurfaceState, SurfaceTransaction, TransactionCommit, TransactionOutcome,
};

/// Rebase a complete presentation snapshot onto the Engine visual generation.
///
/// Skipped asynchronous presentations deliberately do not advance Engine state. A later
/// full-state frame may therefore carry an authority-local generation ahead of the last
/// visible generation; only its causal generation is rebased before normal validation.
pub fn rebase_full_state_present_transactions(
    transactions: &[SurfaceTransaction],
    committed: &[CommittedSurfaceState],
) -> Vec<SurfaceTransaction> {
    transactions
        .iter()
        .cloned()
        .map(|mut transaction| {
            transaction.previous_committed_generation = committed
                .iter()
                .find(|state| state.surface == transaction.surface)
                .map_or(0, |state| state.committed_generation);
            transaction
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSessionPhase {
    AuthorityIntake,
    EngineCommitPreparation,
    FrameComposition,
    KmsSubmit,
    KmsRetire,
    ProtocolFeedback,
}

pub trait ProductionOutputRuntimeAdapter {
    type Report;
    type Error;

    fn output_count(&self) -> usize;

    fn run_output(
        &mut self,
        output_index: usize,
        committed: &[CommittedSurfaceState],
    ) -> Result<Self::Report, Self::Error>;
}

pub trait ProductionPresentationAdapter {
    type Frame;
    type Submission;
    type Retirement;
    type Evidence;
    type Error;

    fn compose(
        &mut self,
        cycle: u64,
        committed: &[CommittedSurfaceState],
        authority_commits: &[TransactionCommit],
    ) -> Result<Self::Frame, Self::Error>;

    fn submit_frame(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Submission, Self::Error>;

    fn poll_retirements(
        &mut self,
    ) -> Result<Vec<ProductionRetirement<Self::Retirement>>, Self::Error>;

    fn route_protocol_feedback(
        &mut self,
        cycle: u64,
        retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionRetirement<Retirement> {
    pub cycle: u64,
    pub retirement: Retirement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionPreparedRetirementReport<Evidence> {
    pub commit: TransactionCommit,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
    pub evidence: Evidence,
}

#[derive(Debug, PartialEq)]
pub enum ProductionPreparedRetirementError<Error> {
    EngineCommit(TransactionCommit),
    ProtocolFeedback(Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProductionSessionCycleReport<Submission, Evidence> {
    pub cycle: u64,
    pub authority_commits: Vec<TransactionCommit>,
    pub committed_surfaces: Vec<CommittedSurfaceState>,
    pub submission: Submission,
    pub evidence: Vec<Evidence>,
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

    pub fn apply_prepared_surface_commit(
        &mut self,
        prepared: PreparedSurfaceCommit,
    ) -> TransactionCommit {
        self.engine
            .apply_prepared_surface_commit(prepared, &mut self.committed_surfaces)
    }

    /// Applies the Engine state prepared for an already-matched KMS retirement,
    /// then retires backend resources and produces reduced protocol feedback.
    /// Feedback is never invoked when the prepared baseline is stale or invalid.
    pub fn complete_prepared_retirement<Evidence, Error>(
        &mut self,
        prepared: PreparedSurfaceCommit,
        complete_feedback: impl FnOnce() -> Result<Evidence, Error>,
    ) -> Result<
        ProductionPreparedRetirementReport<Evidence>,
        ProductionPreparedRetirementError<Error>,
    > {
        let commit = self.apply_prepared_surface_commit(prepared);
        if commit.outcome != TransactionOutcome::Committed {
            return Err(ProductionPreparedRetirementError::EngineCommit(commit));
        }
        let evidence =
            complete_feedback().map_err(ProductionPreparedRetirementError::ProtocolFeedback)?;
        Ok(ProductionPreparedRetirementReport {
            commit,
            committed_surfaces: self.committed_surfaces.clone(),
            evidence,
        })
    }

    /// Projects the one committed snapshot to every output and delegates the
    /// backend-private runtime/scanout decision to the production output adapter.
    pub fn run_outputs<A>(&self, adapter: &mut A) -> Result<Vec<A::Report>, A::Error>
    where
        A: ProductionOutputRuntimeAdapter,
    {
        let mut reports = Vec::with_capacity(adapter.output_count());
        for output_index in 0..adapter.output_count() {
            reports.push(adapter.run_output(output_index, &self.committed_surfaces)?);
        }
        Ok(reports)
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
    ) -> Result<
        ProductionSessionCycleReport<A::Submission, A::Evidence>,
        ProductionSessionCycleError<A::Error>,
    >
    where
        A: ProductionPresentationAdapter,
    {
        let cycle = self.next_cycle;
        self.next_cycle = self.next_cycle.saturating_add(1);

        let authority_commits = self.commit_authority_batches(authority_batches);

        let frame = adapter
            .compose(cycle, &self.committed_surfaces, &authority_commits)
            .map_err(|source| ProductionSessionCycleError {
                cycle,
                phase: ProductionSessionPhase::FrameComposition,
                source,
            })?;
        let submission =
            adapter
                .submit_frame(cycle, frame)
                .map_err(|source| ProductionSessionCycleError {
                    cycle,
                    phase: ProductionSessionPhase::KmsSubmit,
                    source,
                })?;
        let retirements =
            adapter
                .poll_retirements()
                .map_err(|source| ProductionSessionCycleError {
                    cycle,
                    phase: ProductionSessionPhase::KmsRetire,
                    source,
                })?;
        let mut evidence = Vec::with_capacity(retirements.len());
        for retirement in retirements {
            evidence.push(
                adapter
                    .route_protocol_feedback(retirement.cycle, retirement.retirement)
                    .map_err(|source| ProductionSessionCycleError {
                        cycle: retirement.cycle,
                        phase: ProductionSessionPhase::ProtocolFeedback,
                        source,
                    })?,
            );
        }

        Ok(ProductionSessionCycleReport {
            cycle,
            authority_commits,
            committed_surfaces: self.committed_surfaces.clone(),
            submission,
            evidence,
        })
    }
}

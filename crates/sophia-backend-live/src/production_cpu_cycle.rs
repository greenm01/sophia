use crate::{LiveCpuBufferUpdate, LiveCpuCompositionReport, LiveProductionComposedFrame};
use sophia_engine::{HeadlessOutput, ProductionPresentationAdapter, ProductionRetirement};
use sophia_protocol::{CommittedSurfaceState, Point, SurfaceId, TransactionCommit};
use sophia_renderer_live::LiveProductionCpuScene;
use std::error::Error;
use std::time::{Duration, Instant};

pub type LiveProductionCycleError = Box<dyn Error>;

pub struct LiveProductionCpuCycleFrame {
    committed_surfaces: Vec<CommittedSurfaceState>,
    authority_commits: Vec<TransactionCommit>,
    composition: LiveCpuCompositionReport,
    native_frames: Option<Vec<LiveProductionComposedFrame>>,
    composed: bool,
    compose_elapsed: Duration,
}

#[derive(Clone, Debug)]
pub struct LiveProductionCpuCycleSubmission<Tick> {
    pub tick: Tick,
    pub composition: LiveCpuCompositionReport,
    pub composed: bool,
    pub compose_elapsed: Duration,
}

pub struct LiveProductionCpuCycleAdapter<'a, Submit> {
    scene: &'a mut LiveProductionCpuScene,
    updates: Option<Vec<LiveCpuBufferUpdate>>,
    raised_surface: Option<SurfaceId>,
    cursor_position: Option<Point>,
    defer_frame: bool,
    create_native_frames: bool,
    output_descriptors: &'a [HeadlessOutput],
    submit: Submit,
}

impl<'a, Submit> LiveProductionCpuCycleAdapter<'a, Submit> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scene: &'a mut LiveProductionCpuScene,
        updates: Vec<LiveCpuBufferUpdate>,
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
        defer_frame: bool,
        create_native_frames: bool,
        output_descriptors: &'a [HeadlessOutput],
        submit: Submit,
    ) -> Self {
        Self {
            scene,
            updates: Some(updates),
            raised_surface,
            cursor_position,
            defer_frame,
            create_native_frames,
            output_descriptors,
            submit,
        }
    }
}

impl<Submit, Tick> ProductionPresentationAdapter for LiveProductionCpuCycleAdapter<'_, Submit>
where
    Submit: FnMut(
        u64,
        &[CommittedSurfaceState],
        &[TransactionCommit],
        Option<Vec<LiveProductionComposedFrame>>,
    ) -> Result<Tick, LiveProductionCycleError>,
{
    type Frame = LiveProductionCpuCycleFrame;
    type Submission = LiveProductionCpuCycleSubmission<Tick>;
    type Retirement = ();
    type Evidence = ();
    type Error = LiveProductionCycleError;

    fn compose(
        &mut self,
        _cycle: u64,
        committed: &[CommittedSurfaceState],
        authority_commits: &[TransactionCommit],
    ) -> Result<Self::Frame, Self::Error> {
        self.scene
            .apply_updates(self.updates.take().unwrap_or_default(), committed)?;
        let compose_started = Instant::now();
        let composition = if self.defer_frame {
            self.scene
                .last_report()
                .cloned()
                .ok_or("software redraw coalescing has no prior composed frame")?
        } else {
            self.scene
                .compose(committed, self.raised_surface, self.cursor_position)?
                .clone()
        };
        let native_frames = if self.defer_frame || !self.create_native_frames {
            None
        } else {
            Some(self.scene.frames_for_outputs(self.output_descriptors)?)
        };
        Ok(LiveProductionCpuCycleFrame {
            committed_surfaces: committed.to_vec(),
            authority_commits: authority_commits.to_vec(),
            composition,
            native_frames,
            composed: !self.defer_frame,
            compose_elapsed: if self.defer_frame {
                Duration::ZERO
            } else {
                compose_started.elapsed()
            },
        })
    }

    fn submit_frame(
        &mut self,
        cycle: u64,
        frame: Self::Frame,
    ) -> Result<Self::Submission, Self::Error> {
        let tick = (self.submit)(
            cycle,
            &frame.committed_surfaces,
            &frame.authority_commits,
            frame.native_frames,
        )?;
        Ok(LiveProductionCpuCycleSubmission {
            tick,
            composition: frame.composition,
            composed: frame.composed,
            compose_elapsed: frame.compose_elapsed,
        })
    }

    fn poll_retirements(
        &mut self,
    ) -> Result<Vec<ProductionRetirement<Self::Retirement>>, Self::Error> {
        Ok(Vec::new())
    }

    fn route_protocol_feedback(
        &mut self,
        _cycle: u64,
        _retirement: Self::Retirement,
    ) -> Result<Self::Evidence, Self::Error> {
        Ok(())
    }
}

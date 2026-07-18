use crate::{
    LIVE_RENDERED_OUTPUT_CAPACITY, LiveBackendRuntimeAssembly, LivePageFlipCallbackQueue,
    LiveProductionComposedFrame, LiveProductionNativeScanout, LiveRendererImportPathStatus,
    LiveRendererImportStartupStatus, LiveRendererRuntimeObservation,
    LiveRendererSelectionObservation,
};
use sophia_engine::{HeadlessCompositorBackendAssembly, HeadlessOutput};
use sophia_protocol::{CommittedSurfaceState, OutputId};
use std::collections::BTreeMap;
use std::error::Error;

pub struct LiveProductionOutputRuntime {
    pub runtime: LiveBackendRuntimeAssembly,
    native_initialized: bool,
}

pub struct LiveProductionOutputRuntimeSet {
    outputs: BTreeMap<OutputId, LiveProductionOutputRuntime>,
}

impl LiveProductionOutputRuntimeSet {
    pub fn new(
        outputs: &[HeadlessOutput],
        committed_surfaces: &[CommittedSurfaceState],
        mut native_scanout: Option<&mut LiveProductionNativeScanout>,
        initial_native_frames: Option<Vec<LiveProductionComposedFrame>>,
    ) -> Result<Self, Box<dyn Error>> {
        if outputs.is_empty() || outputs.len() > LIVE_RENDERED_OUTPUT_CAPACITY {
            return Err("production output runtime requires 1-16 outputs".into());
        }
        let mut initial_native_frames = initial_native_frames.unwrap_or_default().into_iter();
        let mut output_runtimes = BTreeMap::new();
        for (index, output) in outputs.iter().copied().enumerate() {
            let assembly = HeadlessCompositorBackendAssembly::new(output)
                .with_committed_surfaces(committed_surfaces.to_vec());
            let renderer = LiveRendererRuntimeObservation::from_startup_status(
                LiveRendererImportStartupStatus::from_path_statuses(
                    LiveRendererImportPathStatus::Disabled,
                    LiveRendererImportPathStatus::Disabled,
                ),
                LiveRendererSelectionObservation::CpuFallback,
            );
            let mut runtime =
                LiveBackendRuntimeAssembly::from_ready_headless_scanout(assembly, output, renderer)
                    .with_persistent_rendered_primary_plane_scanout();
            let mut native_initialized = native_scanout.is_none();
            if let Some(native_scanout) = native_scanout.as_deref_mut() {
                runtime = runtime.with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(
                    native_scanout.take_receiver(index),
                    64,
                ));
                let selection = native_scanout.selection(index);
                if !runtime.configure_native_output_selection(output.id, selection) {
                    return Err("production native output selection was not registered".into());
                }
                if let Some(initial_frame) = initial_native_frames.next() {
                    native_scanout.initialize(index, &mut runtime, initial_frame)?;
                    native_initialized = true;
                }
            }
            output_runtimes.insert(
                output.id,
                LiveProductionOutputRuntime {
                    runtime,
                    native_initialized,
                },
            );
        }
        Ok(Self {
            outputs: output_runtimes,
        })
    }

    pub fn initialize_native_scanout(
        &mut self,
        native_scanout: &mut LiveProductionNativeScanout,
        frames: &[LiveProductionComposedFrame],
    ) -> Result<(), Box<dyn Error>> {
        if frames.len() != self.outputs.len() {
            return Err("production native initialization frame count mismatch".into());
        }
        for (index, output) in self.outputs.values_mut().enumerate() {
            if !output.native_initialized {
                native_scanout.initialize(index, &mut output.runtime, frames[index].clone())?;
                output.native_initialized = true;
            }
        }
        Ok(())
    }

    pub fn run_output<R>(
        &mut self,
        index: usize,
        committed: &[CommittedSurfaceState],
        run: impl FnOnce(&mut LiveBackendRuntimeAssembly) -> Result<R, Box<dyn Error>>,
    ) -> Result<R, Box<dyn Error>> {
        let output = self
            .outputs
            .values_mut()
            .nth(index)
            .ok_or("production output index was not registered")?;
        output
            .runtime
            .assembly_mut()
            .replace_committed_surfaces(committed.to_vec());
        run(&mut output.runtime)
    }

    pub fn project_committed(&mut self, committed: &[CommittedSurfaceState]) {
        for output in self.outputs.values_mut() {
            output
                .runtime
                .assembly_mut()
                .replace_committed_surfaces(committed.to_vec());
        }
    }

    pub fn replace_output_projection(
        &mut self,
        index: usize,
        committed: Vec<CommittedSurfaceState>,
    ) -> bool {
        let Some(output) = self.outputs.values_mut().nth(index) else {
            return false;
        };
        output
            .runtime
            .assembly_mut()
            .replace_committed_surfaces(committed);
        true
    }

    pub fn output_committed(&self, index: usize) -> Option<&[CommittedSurfaceState]> {
        self.outputs
            .values()
            .nth(index)
            .map(|output| output.runtime.assembly().committed_surfaces())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut LiveProductionOutputRuntime> {
        self.outputs.values_mut()
    }

    pub fn values(&self) -> impl Iterator<Item = &LiveProductionOutputRuntime> {
        self.outputs.values()
    }

    pub fn primary_output(&self) -> Option<OutputId> {
        self.outputs.keys().next().copied()
    }

    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    pub fn native_scanout_in_flight(&self) -> bool {
        self.outputs
            .values()
            .any(|output| output.runtime.rendered_primary_plane_scanout_in_flight())
    }

    pub fn native_cleanup_pending(&self) -> bool {
        self.outputs.values().any(|output| {
            output
                .runtime
                .rendered_primary_plane_scanout_cleanup_pending()
        })
    }

    pub fn diagnostic(&self) -> String {
        self.outputs
            .iter()
            .map(|(output, state)| {
                format!(
                    "{}:in_flight={}:ticks={}:cleanup={}",
                    output.raw(),
                    state.runtime.rendered_primary_plane_scanout_in_flight(),
                    state
                        .runtime
                        .rendered_primary_plane_scanout_in_flight_ticks(),
                    state
                        .runtime
                        .rendered_primary_plane_scanout_cleanup_pending(),
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

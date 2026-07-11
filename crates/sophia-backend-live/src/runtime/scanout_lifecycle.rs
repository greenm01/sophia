use crate::prelude::*;

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub(crate) fn drain_pending_runtime_scanout_states_into(
        &mut self,
        input: &mut CompositorBackendTickInput,
    ) -> Vec<RuntimeScanoutState> {
        let runtime_scanout_states = self.drain_pending_runtime_scanout_states();
        input
            .scanout_lifecycle_states
            .extend(runtime_scanout_states.iter().copied());
        runtime_scanout_states
    }

    #[cfg(feature = "libdrm-events")]
    fn drain_pending_runtime_scanout_states(&mut self) -> Vec<RuntimeScanoutState> {
        self.primary_output_state_mut()
            .pending_runtime_scanout_states
            .drain(..)
            .collect()
    }

    #[cfg(not(feature = "libdrm-events"))]
    fn drain_pending_runtime_scanout_states(&mut self) -> Vec<RuntimeScanoutState> {
        Vec::new()
    }
}

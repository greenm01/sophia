use crate::prelude::*;

pub struct LiveBackendRuntimeAssembly<P = QueuedInputPoller> {
    pub(crate) assembly: HeadlessCompositorBackendAssembly<P>,
    pub(crate) renderer_observation: LiveRendererRuntimeObservation,
    pub(crate) primary_output: OutputId,
    pub(crate) outputs: LiveRenderedOutputTable,
    pub(crate) page_flip_callback_queue: Option<LivePageFlipCallbackQueue>,
    pub(crate) libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics,
}

impl<P> LiveBackendRuntimeAssembly<P>
where
    P: NonBlockingInputPoller,
{
    pub fn from_ready_headless_scanout(
        assembly: HeadlessCompositorBackendAssembly<P>,
        output: HeadlessOutput,
        renderer_observation: LiveRendererRuntimeObservation,
    ) -> Self {
        let mut outputs = LiveRenderedOutputTable::new();
        let _ = outputs.insert(LiveRenderedOutputState::ready(output));

        Self {
            assembly,
            renderer_observation,
            primary_output: output.id,
            outputs,
            page_flip_callback_queue: None,
            libdrm_poller_diagnostics: LiveLibdrmPollerDiagnostics::not_configured(),
        }
    }

    pub fn assembly(&self) -> &HeadlessCompositorBackendAssembly<P> {
        &self.assembly
    }

    pub fn assembly_mut(&mut self) -> &mut HeadlessCompositorBackendAssembly<P> {
        &mut self.assembly
    }

    pub fn renderer_observation(&self) -> LiveRendererRuntimeObservation {
        self.renderer_observation
    }

    pub fn rendered_outputs(&self) -> &LiveRenderedOutputTable {
        &self.outputs
    }

    pub fn add_ready_output(&mut self, output: HeadlessOutput) -> LiveRenderedOutputTableUpdate {
        self.outputs.insert(LiveRenderedOutputState::ready(output))
    }

    pub fn observe_output_vrr_eligibility(
        &mut self,
        output: OutputId,
        policy_enabled: bool,
        capability: OutputVrrCapability,
        eligibility: OutputVrrEligibility,
    ) -> Option<OutputVrrDecision> {
        let state = self.outputs.get_mut(output)?;
        let decision = decide_output_vrr(policy_enabled, capability, eligibility);
        state.vrr_decision = decision;
        state.vrr_property_request = (policy_enabled && capability.capable)
            .then_some(decision == OutputVrrDecision::Enabled);
        Some(decision)
    }

    #[cfg(feature = "libdrm-events")]
    pub fn configure_native_output_selection(
        &mut self,
        output: OutputId,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> bool {
        let Some(state) = self.outputs.get_mut(output) else {
            return false;
        };
        state.native_selection = Some(selection);
        true
    }

    pub(crate) fn primary_output_state(&self) -> &LiveRenderedOutputState {
        self.outputs
            .get(self.primary_output)
            .expect("live runtime primary output must remain registered")
    }

    pub(crate) fn primary_output_state_mut(&mut self) -> &mut LiveRenderedOutputState {
        self.outputs
            .get_mut(self.primary_output)
            .expect("live runtime primary output must remain registered")
    }

    pub fn with_libdrm_poller_diagnostics(
        mut self,
        diagnostics: LiveLibdrmPollerDiagnostics,
    ) -> Self {
        self.libdrm_poller_diagnostics = diagnostics;
        self
    }

    #[cfg(feature = "libdrm-events")]
    pub fn with_native_libdrm_poller_diagnostics(
        self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) -> Self {
        self.with_libdrm_poller_diagnostics(diagnostics.into())
    }

    pub fn observe_libdrm_poller_diagnostics(&mut self, diagnostics: LiveLibdrmPollerDiagnostics) {
        self.libdrm_poller_diagnostics = diagnostics;
    }

    #[cfg(feature = "libdrm-events")]
    pub fn observe_native_libdrm_poller_diagnostics(
        &mut self,
        diagnostics: LibdrmNativePollerDiagnostics,
    ) {
        self.observe_libdrm_poller_diagnostics(diagnostics.into());
    }

    pub fn libdrm_poller_diagnostics(&self) -> LiveLibdrmPollerDiagnostics {
        self.libdrm_poller_diagnostics
    }
}

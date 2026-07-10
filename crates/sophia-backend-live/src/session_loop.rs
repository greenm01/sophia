#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
use crate::prelude::*;

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendSessionLoopReadiness {
    pub input_ready: bool,
    pub page_flip_ready: bool,
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
impl LiveBackendSessionLoopReadiness {
    pub const fn new(input_ready: bool, page_flip_ready: bool) -> Self {
        Self {
            input_ready,
            page_flip_ready,
        }
    }

    pub const fn idle() -> Self {
        Self::new(false, false)
    }

    pub const fn input_ready() -> Self {
        Self::new(true, false)
    }

    pub const fn page_flip_ready() -> Self {
        Self::new(false, true)
    }

    pub const fn all_ready() -> Self {
        Self::new(true, true)
    }
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveBackendReadinessCollector {
    input_ready: bool,
    page_flip_ready: bool,
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
impl LiveBackendReadinessCollector {
    pub const fn new() -> Self {
        Self {
            input_ready: false,
            page_flip_ready: false,
        }
    }

    pub fn observe_input_ready(&mut self) {
        self.input_ready = true;
    }

    pub fn observe_page_flip_ready(&mut self) {
        self.page_flip_ready = true;
    }

    pub const fn snapshot(&self) -> LiveBackendSessionLoopReadiness {
        LiveBackendSessionLoopReadiness::new(self.input_ready, self.page_flip_ready)
    }

    pub fn drain(&mut self) -> LiveBackendSessionLoopReadiness {
        let readiness = self.snapshot();
        self.input_ready = false;
        self.page_flip_ready = false;
        readiness
    }
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveBackendSessionLoopPageFlipBudget {
    pub max_read: usize,
    pub max_emit: usize,
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
impl LiveBackendSessionLoopPageFlipBudget {
    pub const fn new(max_read: usize, max_emit: usize) -> Self {
        Self { max_read, max_emit }
    }
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendSessionLoopTickReport {
    pub input_gate: LiveInputReadinessGateReport,
    pub native_page_flip: LibdrmNativeReadAndPollReport,
    pub tick: LiveBackendRuntimeTickReport,
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
#[derive(Clone, Debug, PartialEq)]
pub struct LiveBackendSessionLoop {
    page_flip_poller: NativeLibdrmPageFlipEventPoller,
    page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
impl LiveBackendSessionLoop {
    pub const fn new(
        page_flip_poller: NativeLibdrmPageFlipEventPoller,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
    ) -> Self {
        Self {
            page_flip_poller,
            page_flip_budget,
        }
    }

    pub const fn page_flip_budget(&self) -> LiveBackendSessionLoopPageFlipBudget {
        self.page_flip_budget
    }

    pub const fn page_flip_poller(&self) -> &NativeLibdrmPageFlipEventPoller {
        &self.page_flip_poller
    }

    pub fn page_flip_poller_mut(&mut self) -> &mut NativeLibdrmPageFlipEventPoller {
        &mut self.page_flip_poller
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with<
        P,
        D,
        E,
        R,
    >(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        device: &D,
        exporter: &mut E,
        reader: &mut R,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
        R: LibdrmNativePageFlipReader,
    {
        runtime
            .run_session_loop_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
                input,
                readiness,
                self.page_flip_budget,
                device,
                exporter,
                reader,
                &mut self.page_flip_poller,
                sender,
            )
    }

    #[cfg(feature = "gbm-probe")]
    #[allow(clippy::too_many_arguments)]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with<
        P,
        D,
        E,
        R,
    >(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        reader: &mut R,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: RenderDeviceDiscoveryBackend,
        R: LibdrmNativePageFlipReader,
    {
        self.run_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            runtime, input, readiness, device, exporter, reader, sender,
        )
    }
}

#[cfg(all(feature = "libdrm-events", feature = "libinput-events"))]
impl<P> LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>
where
    P: NonBlockingInputPoller,
{
    #[allow(clippy::too_many_arguments)]
    pub fn run_session_loop_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with<
        D,
        E,
        R,
    >(
        &mut self,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        device: &D,
        exporter: &mut E,
        reader: &mut R,
        poller: &mut NativeLibdrmPageFlipEventPoller,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: LiveRenderedScanoutBufferExporter,
        E::Owner: 'static,
        R: LibdrmNativePageFlipReader,
    {
        if readiness.input_ready {
            self.assembly_mut().input_mut().poller_mut().observe_ready();
        }

        let native_page_flip = if readiness.page_flip_ready {
            poller.read_and_poll_page_flip_events(
                reader,
                sender,
                page_flip_budget.max_read,
                page_flip_budget.max_emit,
            )
        } else {
            let poll = poller.poll_page_flip_events(sender, page_flip_budget.max_emit);
            LibdrmNativeReadAndPollReport {
                read_loop: poller.last_read_loop_report(),
                poll,
            }
        };
        self.observe_native_libdrm_poller_diagnostics(poller.diagnostics());
        let tick =
            self.run_tick_with_rendered_primary_plane_scanout_with(input, device, exporter)?;
        let input_gate = self.assembly().input().poller().last_gate_report();

        Ok(LiveBackendSessionLoopTickReport {
            input_gate,
            native_page_flip,
            tick,
        })
    }

    #[cfg(feature = "gbm-probe")]
    #[allow(clippy::too_many_arguments)]
    pub fn run_session_loop_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with<
        D,
        E,
        R,
    >(
        &mut self,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        device: &D,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        reader: &mut R,
        poller: &mut NativeLibdrmPageFlipEventPoller,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        D: LibdrmNativeKmsSelectionDevice
            + LibdrmNativePropertyLookupDevice
            + LibdrmNativePrimaryPlaneResourceDevice
            + LibdrmNativeAtomicCommitDevice,
        E: RenderDeviceDiscoveryBackend,
        R: LibdrmNativePageFlipReader,
    {
        self.run_session_loop_tick_with_rendered_primary_plane_scanout_and_native_page_flip_events_with(
            input, readiness, page_flip_budget, device, exporter, reader, poller, sender,
        )
    }
}

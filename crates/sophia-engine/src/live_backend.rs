use crate::prelude::*;
use crate::{
    DeterministicFrameClock, DrmKmsOutputRegistry, DrmKmsSysfsDiscovery,
    HeadlessCompositorBackendAssembly, HeadlessOutput, LibinputDeviceDescriptor,
    LibinputEventSource, LibinputPhysicalInputAdapter, NonBlockingInputPoller, RendererSelection,
};

pub trait OutputDiscoveryBackend {
    fn discover_outputs(&self) -> io::Result<DrmKmsOutputRegistry>;
}

pub trait InputDiscoveryBackend {
    fn discover_input_source(&self) -> io::Result<LibinputEventSource>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveCompositorBackendDiscoveryStatus {
    Ready,
    NoOutputs,
    OutputDiscoveryFailed { message: String },
    InputDiscoveryFailed { message: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveCompositorBackendDiscoveryReport {
    pub status: LiveCompositorBackendDiscoveryStatus,
    pub outputs: DrmKmsOutputRegistry,
    pub selected_output: Option<HeadlessOutput>,
    pub input_source: LibinputEventSource,
}

impl LiveCompositorBackendDiscoveryReport {
    pub fn is_ready(&self) -> bool {
        self.status == LiveCompositorBackendDiscoveryStatus::Ready
    }

    pub fn into_headless_assembly<P>(
        self,
        poller: P,
        renderer: RendererSelection,
    ) -> Option<HeadlessCompositorBackendAssembly<P>>
    where
        P: NonBlockingInputPoller,
    {
        let output = self.selected_output?;
        if !self.is_ready() {
            return None;
        }

        Some(HeadlessCompositorBackendAssembly::from_parts(
            output,
            self.outputs,
            DeterministicFrameClock::default(),
            LibinputPhysicalInputAdapter::new(poller, self.input_source),
            renderer,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SysfsDrmKmsOutputBackend {
    root: PathBuf,
    discovery: DrmKmsSysfsDiscovery,
}

impl SysfsDrmKmsOutputBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            discovery: DrmKmsSysfsDiscovery::default(),
        }
    }

    pub fn with_discovery(root: impl Into<PathBuf>, discovery: DrmKmsSysfsDiscovery) -> Self {
        Self {
            root: root.into(),
            discovery,
        }
    }
}

impl OutputDiscoveryBackend for SysfsDrmKmsOutputBackend {
    fn discover_outputs(&self) -> io::Result<DrmKmsOutputRegistry> {
        self.discovery.discover_outputs(&self.root)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticInputDiscoveryBackend {
    devices: Vec<LibinputDeviceDescriptor>,
}

impl StaticInputDiscoveryBackend {
    pub fn new(devices: Vec<LibinputDeviceDescriptor>) -> Self {
        Self { devices }
    }
}

impl InputDiscoveryBackend for StaticInputDiscoveryBackend {
    fn discover_input_source(&self) -> io::Result<LibinputEventSource> {
        let mut source = LibinputEventSource::new();
        for device in &self.devices {
            source.register_device(*device);
        }
        Ok(source)
    }
}

pub fn discover_live_compositor_backend(
    outputs: &impl OutputDiscoveryBackend,
    input: &impl InputDiscoveryBackend,
) -> LiveCompositorBackendDiscoveryReport {
    let outputs = match outputs.discover_outputs() {
        Ok(outputs) => outputs,
        Err(error) => {
            return LiveCompositorBackendDiscoveryReport {
                status: LiveCompositorBackendDiscoveryStatus::OutputDiscoveryFailed {
                    message: error.to_string(),
                },
                outputs: DrmKmsOutputRegistry::new(),
                selected_output: None,
                input_source: LibinputEventSource::new(),
            };
        }
    };
    let Some(selected_output) = outputs.primary_engine_output() else {
        return LiveCompositorBackendDiscoveryReport {
            status: LiveCompositorBackendDiscoveryStatus::NoOutputs,
            outputs,
            selected_output: None,
            input_source: LibinputEventSource::new(),
        };
    };
    let input_source = match input.discover_input_source() {
        Ok(source) => source,
        Err(error) => {
            return LiveCompositorBackendDiscoveryReport {
                status: LiveCompositorBackendDiscoveryStatus::InputDiscoveryFailed {
                    message: error.to_string(),
                },
                outputs,
                selected_output: Some(selected_output),
                input_source: LibinputEventSource::new(),
            };
        }
    };

    LiveCompositorBackendDiscoveryReport {
        status: LiveCompositorBackendDiscoveryStatus::Ready,
        outputs,
        selected_output: Some(selected_output),
        input_source,
    }
}

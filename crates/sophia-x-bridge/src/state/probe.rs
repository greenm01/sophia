use super::*;
use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceRecord {
    pub namespace: NamespaceId,
    pub label: String,
    pub source: NamespaceSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespaceSource {
    StaticConfig,
    XServer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticNamespaceConfig {
    namespaces: Vec<NamespaceRecord>,
}

impl StaticNamespaceConfig {
    pub fn new(namespaces: Vec<NamespaceRecord>) -> Self {
        Self { namespaces }
    }

    pub fn namespaces(&self) -> &[NamespaceRecord] {
        &self.namespaces
    }

    pub fn record_namespace(&mut self, record: NamespaceRecord) {
        if let Some(existing) = self
            .namespaces
            .iter_mut()
            .find(|existing| existing.namespace == record.namespace)
        {
            *existing = record;
            return;
        }

        self.namespaces.push(record);
    }

    pub fn with_discovered(mut self, records: impl IntoIterator<Item = NamespaceRecord>) -> Self {
        for record in records {
            self.record_namespace(record);
        }

        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceOwnership {
    pub window: XWindowId,
    pub namespace: NamespaceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XConnectionProbe {
    pub display_name: Option<String>,
    pub screen_num: usize,
    pub required_extensions: Vec<ExtensionStatus>,
    pub namespaces: StaticNamespaceConfig,
}

impl XConnectionProbe {
    pub fn missing_extensions(&self) -> Vec<RequiredExtension> {
        self.required_extensions
            .iter()
            .filter(|status| !status.present)
            .map(|status| status.extension)
            .collect()
    }

    pub fn has_required_extensions(&self) -> bool {
        self.missing_extensions().is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRootImport {
    pub probe: XConnectionProbe,
    pub mirror: XMirrorState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestClientConfig {
    pub display_name: Option<String>,
    pub size: Size,
    pub hold_millis: u64,
}

impl Default for TestClientConfig {
    fn default() -> Self {
        Self {
            display_name: None,
            size: Size {
                width: 320,
                height: 200,
            },
            hold_millis: 5_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestClientWindow {
    pub window: XWindowId,
    pub size: Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeReadbackReport {
    pub display_name: Option<String>,
    pub mirrored_windows: usize,
    pub surfaces: usize,
    pub renderable_layers: usize,
    pub redirect_targets: usize,
    pub readbacks: usize,
    pub total_bytes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmokeReadbackCapture {
    pub report: SmokeReadbackReport,
    pub surfaces: Vec<SurfaceSnapshot>,
    pub layers: Vec<LayerSnapshot>,
    pub readbacks: Vec<CpuBufferSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAtoms {
    pub wm_state: Atom,
    pub net_client_list: Atom,
    pub wm_protocols: Atom,
    pub wm_delete_window: Atom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoliteCloseOutcome {
    SentDeleteWindow { window: XWindowId },
    UnsupportedProtocol { window: XWindowId },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XClientHints {
    pub ewmh_clients: Vec<XWindowId>,
    pub icccm_clients: Vec<XWindowId>,
}

use sophia_protocol::{LayerSnapshot, XWindowMirror};

#[derive(Default)]
pub struct XMirrorState {
    windows: Vec<XWindowMirror>,
}

impl XMirrorState {
    pub fn ingest_window(&mut self, mirror: XWindowMirror) {
        self.windows.push(mirror);
    }

    pub fn windows(&self) -> &[XWindowMirror] {
        &self.windows
    }

    pub fn emit_layers(&self) -> Vec<LayerSnapshot> {
        Vec::new()
    }
}

use crate::EngineError;
use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayStep {
    pub command_index: usize,
    pub kind: RenderCommandKind,
    pub source: Option<SurfaceId>,
    pub target: Region,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayReport {
    pub output: OutputId,
    pub output_size: Size,
    pub output_scale: u32,
    pub frame_serial: u64,
    pub steps: Vec<ReplayStep>,
    pub damage: Region,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BufferImportPath {
    CpuReadback,
    XPixmap,
    DmaBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedBufferHandle {
    CpuReadback { source: BufferSource },
    XPixmap { pixmap: u32 },
    DmaBuf { handle: u64 },
}

impl ImportedBufferHandle {
    pub const fn path(self) -> BufferImportPath {
        match self {
            Self::CpuReadback { .. } => BufferImportPath::CpuReadback,
            Self::XPixmap { .. } => BufferImportPath::XPixmap,
            Self::DmaBuf { .. } => BufferImportPath::DmaBuf,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferImportReport {
    pub surface: SurfaceId,
    pub source: BufferSource,
    pub requested: BufferImportPath,
    pub used: BufferImportPath,
    pub handle: ImportedBufferHandle,
    pub used_fallback: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderFrameReport {
    pub replay: ReplayReport,
    pub imports: Vec<BufferImportReport>,
}

pub trait FrameRenderer {
    fn render_frame(
        &self,
        frame: &FrameSnapshot,
        replay: ReplayReport,
    ) -> Result<RenderFrameReport, EngineError>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuFallbackRenderer;

impl FrameRenderer for CpuFallbackRenderer {
    fn render_frame(
        &self,
        frame: &FrameSnapshot,
        replay: ReplayReport,
    ) -> Result<RenderFrameReport, EngineError> {
        let imports = collect_buffer_imports(frame, &|source| ImportedBufferHandle::CpuReadback {
            source,
        });
        trace!(
            output = frame.output.raw(),
            frame_serial = frame.frame_serial,
            import_count = imports.len(),
            "rendered frame with CPU fallback renderer"
        );

        Ok(RenderFrameReport { replay, imports })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImportCapableRenderer {
    pub import_xpixmap: bool,
    pub import_dmabuf: bool,
}

impl ImportCapableRenderer {
    pub const fn new(import_xpixmap: bool, import_dmabuf: bool) -> Self {
        Self {
            import_xpixmap,
            import_dmabuf,
        }
    }

    fn import_source(&self, source: BufferSource) -> ImportedBufferHandle {
        match source {
            BufferSource::XPixmap { pixmap } if self.import_xpixmap => {
                ImportedBufferHandle::XPixmap { pixmap }
            }
            BufferSource::DmaBuf { handle } if self.import_dmabuf => {
                ImportedBufferHandle::DmaBuf { handle }
            }
            _ => ImportedBufferHandle::CpuReadback { source },
        }
    }
}

impl FrameRenderer for ImportCapableRenderer {
    fn render_frame(
        &self,
        frame: &FrameSnapshot,
        replay: ReplayReport,
    ) -> Result<RenderFrameReport, EngineError> {
        let imports = collect_buffer_imports(frame, &|source| self.import_source(source));
        trace!(
            output = frame.output.raw(),
            frame_serial = frame.frame_serial,
            import_count = imports.len(),
            import_xpixmap = self.import_xpixmap,
            import_dmabuf = self.import_dmabuf,
            "rendered frame with import-capable renderer"
        );

        Ok(RenderFrameReport { replay, imports })
    }
}

pub(crate) fn should_render(layer: &LayerSnapshot) -> bool {
    layer.opacity > 0.0 && !layer.geometry.is_empty() && layer.source != BufferSource::None
}

fn collect_buffer_imports(
    frame: &FrameSnapshot,
    import_source: &impl Fn(BufferSource) -> ImportedBufferHandle,
) -> Vec<BufferImportReport> {
    let layers_by_surface = frame
        .layers
        .iter()
        .map(|layer| (layer.surface, layer))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut imports = Vec::new();

    for command in &frame.commands {
        let Some(surface) = command.source else {
            continue;
        };
        if !seen.insert(surface) {
            continue;
        }
        if let Some(layer) = layers_by_surface.get(&surface) {
            if let Some(import) = buffer_import_report(layer, import_source) {
                imports.push(import);
            }
        }
    }

    imports
}

fn buffer_import_report(
    layer: &LayerSnapshot,
    import_source: &impl Fn(BufferSource) -> ImportedBufferHandle,
) -> Option<BufferImportReport> {
    let requested = match layer.source {
        BufferSource::None => return None,
        BufferSource::CpuBuffer { .. } => BufferImportPath::CpuReadback,
        BufferSource::XPixmap { .. } => BufferImportPath::XPixmap,
        BufferSource::DmaBuf { .. } => BufferImportPath::DmaBuf,
    };
    let handle = import_source(layer.source);
    let used = handle.path();

    Some(BufferImportReport {
        surface: layer.surface,
        source: layer.source,
        requested,
        used,
        handle,
        used_fallback: requested != used,
    })
}

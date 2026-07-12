#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use super::{LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter};
#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use crate::api::*;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
use sophia_renderer_live::{
    LiveCpuComposedFrame, LiveRendererScanoutBufferExportDetail,
    LiveRendererScanoutBufferExportStatus, NativeGbmOwnedScanoutBuffer,
    NativeGbmRenderedScanoutContext, NativeGbmRenderedScanoutContextStatus,
};

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
pub struct NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    discovery: R,
    context: Option<NativeGbmRenderedScanoutContext<R::Device>>,
    context_status: Option<NativeGbmRenderedScanoutContextStatus>,
    context_open_attempts: usize,
    export_attempts: usize,
    preferred_modifiers: Vec<u64>,
    last_target: Option<LiveGbmEglFrameTargetRecord>,
    last_target_lifecycle: Option<LiveGbmEglFrameTargetLifecycleReport>,
    last_export_status: Option<LiveRendererScanoutBufferExportStatus>,
    pending_cpu_frame: Option<LiveCpuComposedFrame>,
    pending_dmabuf_frame: Option<sophia_renderer_live::LiveOwnedDmaBufFrame>,
    pending_cpu_frame_checksum: Option<u64>,
    cpu_frame_export_attempts: usize,
    last_cpu_frame_checksum: Option<u64>,
    last_cpu_frame_export_status: Option<LiveRendererScanoutBufferExportStatus>,
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    pub fn new(discovery: R) -> Self {
        Self {
            discovery,
            context: None,
            context_status: None,
            context_open_attempts: 0,
            export_attempts: 0,
            preferred_modifiers: Vec::new(),
            last_target: None,
            last_target_lifecycle: None,
            last_export_status: None,
            pending_cpu_frame: None,
            pending_dmabuf_frame: None,
            pending_cpu_frame_checksum: None,
            cpu_frame_export_attempts: 0,
            last_cpu_frame_checksum: None,
            last_cpu_frame_export_status: None,
        }
    }

    pub fn with_preferred_modifiers(mut self, preferred_modifiers: impl Into<Vec<u64>>) -> Self {
        self.preferred_modifiers = reduced_preferred_scanout_modifiers(preferred_modifiers.into());
        self
    }

    pub const fn context_open_attempts(&self) -> usize {
        self.context_open_attempts
    }

    pub const fn export_attempts(&self) -> usize {
        self.export_attempts
    }

    pub const fn last_export_status(&self) -> Option<LiveRendererScanoutBufferExportStatus> {
        self.last_export_status
    }

    pub const fn last_target(&self) -> Option<LiveGbmEglFrameTargetRecord> {
        self.last_target
    }

    pub const fn last_target_lifecycle(&self) -> Option<LiveGbmEglFrameTargetLifecycleReport> {
        self.last_target_lifecycle
    }

    pub const fn context_status(&self) -> Option<NativeGbmRenderedScanoutContextStatus> {
        self.context_status
    }

    pub const fn context_ready(&self) -> bool {
        self.context.is_some()
    }

    pub fn persistent_render_stats(&self) -> sophia_renderer_live::LiveNativePersistentRenderStats {
        self.context.as_ref().map_or_else(
            sophia_renderer_live::LiveNativePersistentRenderStats::default,
            NativeGbmRenderedScanoutContext::persistent_render_stats,
        )
    }

    pub fn discovery(&self) -> &R {
        &self.discovery
    }

    pub fn discovery_mut(&mut self) -> &mut R {
        &mut self.discovery
    }

    pub fn set_pending_cpu_frame(&mut self, frame: LiveCpuComposedFrame) {
        let checksum = cpu_frame_checksum(&frame);
        self.set_pending_cpu_frame_with_checksum(frame, checksum);
    }

    pub fn set_pending_cpu_frame_with_checksum(
        &mut self,
        frame: LiveCpuComposedFrame,
        checksum: u64,
    ) {
        self.pending_cpu_frame_checksum = Some(checksum);
        self.pending_cpu_frame = Some(frame);
    }

    pub const fn pending_cpu_frame(&self) -> bool {
        self.pending_cpu_frame.is_some()
    }

    pub fn set_pending_dmabuf_frame(&mut self, frame: sophia_renderer_live::LiveOwnedDmaBufFrame) {
        self.pending_dmabuf_frame = Some(frame);
    }

    pub const fn pending_dmabuf_frame(&self) -> bool {
        self.pending_dmabuf_frame.is_some()
    }

    pub const fn cpu_frame_export_attempts(&self) -> usize {
        self.cpu_frame_export_attempts
    }

    pub const fn last_cpu_frame_checksum(&self) -> Option<u64> {
        self.last_cpu_frame_checksum
    }

    pub const fn last_cpu_frame_export_status(
        &self,
    ) -> Option<LiveRendererScanoutBufferExportStatus> {
        self.last_cpu_frame_export_status
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
impl<R> LiveRenderedScanoutBufferExporter for NativeGbmRenderedScanoutBufferDiscoveryExporter<R>
where
    R: RenderDeviceDiscoveryBackend,
{
    type Owner = NativeGbmOwnedScanoutBuffer;

    fn export_rendered_scanout_buffer(
        &mut self,
        target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        let target_lifecycle =
            LiveGbmEglFrameTargetLifecycleReport::from_size_update(self.last_target, target);
        self.last_target = Some(target);
        self.last_target_lifecycle = Some(target_lifecycle);

        if !target.is_valid_scanout_target() {
            self.last_export_status = Some(LiveRendererScanoutBufferExportStatus::InvalidTarget);
            return LiveRenderedScanoutBufferExport::new(
                LiveRendererScanoutBufferExportStatus::InvalidTarget,
                LiveRendererScanoutBufferExportDetail::InvalidTarget,
                None,
                None,
            );
        }

        if self.context.is_none() {
            self.context_open_attempts = self.context_open_attempts.saturating_add(1);
            let report = NativeGbmRenderedScanoutContext::from_backend_device_result(
                self.discovery.open_render_device(),
            );
            self.context_status = Some(report.status);
            self.context = report.context;
        }

        let Some(context) = &mut self.context else {
            let status = match self.context_status {
                Some(NativeGbmRenderedScanoutContextStatus::Degraded) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Ready) => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                Some(NativeGbmRenderedScanoutContextStatus::Unavailable) | None => {
                    LiveRendererScanoutBufferExportStatus::Unavailable
                }
            };
            self.last_export_status = Some(status);
            return LiveRenderedScanoutBufferExport::new(
                status,
                LiveRendererScanoutBufferExportDetail::from_status(status),
                None,
                None,
            );
        };

        let report = match self.pending_dmabuf_frame.take() {
            Some(frame) => context.export_dmabuf_owned_scanout_buffer_with_modifiers(
                target,
                frame.as_frame(),
                &self.preferred_modifiers,
            ),
            None => match self.pending_cpu_frame.take() {
                Some(frame) => {
                    self.cpu_frame_export_attempts =
                        self.cpu_frame_export_attempts.saturating_add(1);
                    self.last_cpu_frame_checksum = self.pending_cpu_frame_checksum.take();
                    let report = context.export_xrgb8888_owned_scanout_buffer_with_modifiers(
                        target,
                        &frame,
                        &self.preferred_modifiers,
                    );
                    self.last_cpu_frame_export_status = Some(report.status);
                    report
                }
                None => context.export_rendered_owned_scanout_buffer_with_modifiers(
                    target,
                    &self.preferred_modifiers,
                ),
            },
        };
        let descriptor = report.buffer.as_ref().map(|buffer| buffer.descriptor());
        self.last_export_status = Some(report.status);
        LiveRenderedScanoutBufferExport::new(
            report.status,
            report.detail,
            descriptor,
            report.buffer,
        )
    }
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
fn reduced_preferred_scanout_modifiers(mut modifiers: Vec<u64>) -> Vec<u64> {
    let mut reduced = Vec::new();
    for modifier in modifiers.drain(..) {
        if modifier == u64::MAX || reduced.contains(&modifier) {
            continue;
        }
        reduced.push(modifier);
        if reduced.len() >= MAX_PREFERRED_SCANOUT_MODIFIERS {
            break;
        }
    }
    reduced
}

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
const MAX_PREFERRED_SCANOUT_MODIFIERS: usize = 16;

#[cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]
fn cpu_frame_checksum(frame: &LiveCpuComposedFrame) -> u64 {
    frame
        .bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        })
}

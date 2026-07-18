use std::collections::BTreeSet;

use sophia_engine::HeadlessOutput;
use sophia_protocol::{BufferSource, CommittedSurfaceState, Point, Rect, Size, SurfaceId};

use crate::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferRegistry, LiveCpuBufferSource,
    LiveCpuBufferSourceRef, LiveCpuBufferUpdate, LiveCpuComposedFrame, LiveCpuCompositionLayer,
    LiveCpuCompositionLayerRef, LiveCpuCompositionReport, compose_live_cpu_frame,
    compose_live_cpu_frame_ref_with_cursor,
};

#[derive(Clone)]
pub struct LiveProductionComposedFrame {
    pub frame: LiveCpuComposedFrame,
    pub checksum: u64,
    pub nonzero_pixel_bytes: usize,
}

pub struct LiveProductionCpuScene {
    output_size: Size,
    buffers: LiveCpuBufferRegistry,
    last_report: Option<LiveCpuCompositionReport>,
    max_nonzero_pixel_bytes: usize,
    nonzero_frames: usize,
}

impl LiveProductionCpuScene {
    pub fn new(output_size: Size) -> Self {
        Self {
            output_size,
            buffers: LiveCpuBufferRegistry::new(),
            last_report: None,
            max_nonzero_pixel_bytes: 0,
            nonzero_frames: 0,
        }
    }

    pub fn apply_updates(
        &mut self,
        updates: impl IntoIterator<Item = LiveCpuBufferUpdate>,
        committed_surfaces: &[CommittedSurfaceState],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for update in updates {
            self.buffers
                .apply(update)
                .map_err(|error| format!("renderer CPU buffer update failed: {error:?}"))?;
        }
        let retained_handles = committed_surfaces
            .iter()
            .filter_map(|surface| match surface.buffer {
                BufferSource::CpuBuffer { handle } => Some(handle),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        self.buffers
            .retain_handles(|handle| retained_handles.contains(&handle));
        Ok(())
    }

    pub fn compose(
        &mut self,
        committed_surfaces: &[CommittedSurfaceState],
        raised_surface: Option<SurfaceId>,
        cursor_position: Option<Point>,
    ) -> Result<&LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        let mut surface_order = committed_surfaces
            .iter()
            .filter(|surface| Some(surface.surface) != raised_surface)
            .collect::<Vec<_>>();
        if let Some(raised) = raised_surface
            && let Some(surface) = committed_surfaces
                .iter()
                .find(|surface| surface.surface == raised)
        {
            surface_order.push(surface);
        }
        let layers = surface_order
            .iter()
            .filter_map(|surface| {
                let BufferSource::CpuBuffer { handle } = surface.buffer else {
                    return None;
                };
                let buffer = self.buffers.get(handle)?;
                Some(LiveCpuCompositionLayerRef {
                    geometry: surface.geometry,
                    buffer: LiveCpuBufferSourceRef {
                        handle: buffer.handle,
                        size: buffer.size,
                        stride: buffer.stride,
                        format: buffer.format,
                        generation: buffer.generation,
                        bytes: &buffer.bytes,
                    },
                })
            })
            .collect::<Vec<_>>();
        self.last_report = Some(
            compose_live_cpu_frame_ref_with_cursor(self.output_size, &layers, cursor_position)
                .map_err(|error| format!("persistent CPU composition failed: {error:?}"))?,
        );
        let nonzero_pixel_bytes = self
            .last_report
            .as_ref()
            .expect("assigned above")
            .nonzero_pixel_bytes;
        self.max_nonzero_pixel_bytes = self.max_nonzero_pixel_bytes.max(nonzero_pixel_bytes);
        self.nonzero_frames = self
            .nonzero_frames
            .saturating_add(usize::from(nonzero_pixel_bytes > 0));
        Ok(self.last_report.as_ref().expect("assigned above"))
    }

    pub fn last_report(&self) -> Option<&LiveCpuCompositionReport> {
        self.last_report.as_ref()
    }

    pub fn max_nonzero_pixel_bytes(&self) -> usize {
        self.max_nonzero_pixel_bytes
    }

    pub fn nonzero_frames(&self) -> usize {
        self.nonzero_frames
    }

    pub fn buffer_checksum(&self) -> u64 {
        self.buffers.checksum()
    }

    pub fn surface_buffer_generation(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        surface: SurfaceId,
    ) -> Option<u64> {
        let committed = committed_surfaces
            .iter()
            .find(|committed| committed.surface == surface)?;
        let BufferSource::CpuBuffer { handle } = committed.buffer else {
            return None;
        };
        Some(self.buffers.get(handle)?.generation)
    }

    /// Returns true only when the focused surface contains at least two
    /// visible XRGB pixel values. A newly mapped xterm initially publishes a
    /// uniform background buffer; its prompt or cursor introduces visual
    /// detail once the terminal side is ready for input. Inspecting the
    /// focused surface avoids treating another client's draw as readiness.
    pub fn surface_has_visual_detail(
        &self,
        committed_surfaces: &[CommittedSurfaceState],
        surface: SurfaceId,
    ) -> bool {
        let Some(committed) = committed_surfaces
            .iter()
            .find(|committed| committed.surface == surface)
        else {
            return false;
        };
        let BufferSource::CpuBuffer { handle } = committed.buffer else {
            return false;
        };
        let Some(buffer) = self.buffers.get(handle) else {
            return false;
        };
        let Ok(width) = usize::try_from(buffer.size.width) else {
            return false;
        };
        let Ok(height) = usize::try_from(buffer.size.height) else {
            return false;
        };
        let Ok(stride) = usize::try_from(buffer.stride) else {
            return false;
        };
        let Some(row_bytes) = width.checked_mul(4) else {
            return false;
        };
        if width == 0 || height == 0 || stride < row_bytes || buffer.bytes.len() < 4 {
            return false;
        }
        let first = &buffer.bytes[..4];
        (0..height).any(|row| {
            let Some(start) = row.checked_mul(stride) else {
                return false;
            };
            let Some(end) = start.checked_add(row_bytes) else {
                return false;
            };
            buffer
                .bytes
                .get(start..end)
                .is_some_and(|bytes| bytes.chunks_exact(4).any(|pixel| pixel != first))
        })
    }

    pub fn frames_for_outputs(
        &self,
        outputs: &[HeadlessOutput],
    ) -> Result<Vec<LiveProductionComposedFrame>, Box<dyn std::error::Error>> {
        let primary = self
            .last_report
            .as_ref()
            .ok_or("persistent CPU scene has no composed primary frame")?;
        let mut frames = Vec::with_capacity(outputs.len());
        for (index, output) in outputs.iter().enumerate() {
            if index == 0 && output.size == primary.frame.size {
                frames.push(LiveProductionComposedFrame {
                    frame: primary.frame.clone(),
                    checksum: primary.checksum,
                    nonzero_pixel_bytes: primary.nonzero_pixel_bytes,
                });
                continue;
            }
            let marker_size = Size {
                width: output.size.width.min(64).max(1),
                height: output.size.height.min(64).max(1),
            };
            let marker_width = usize::try_from(marker_size.width)?;
            let marker_height = usize::try_from(marker_size.height)?;
            let marker_stride = marker_width
                .checked_mul(4)
                .ok_or("marker stride overflow")?;
            let marker_byte = u8::try_from((index + 1).min(255)).unwrap_or(255);
            let marker = LiveCpuCompositionLayer {
                geometry: Rect {
                    x: 0,
                    y: 0,
                    width: marker_size.width,
                    height: marker_size.height,
                },
                buffer: LiveCpuBufferSource {
                    handle: 0x5350_4800u64.saturating_add(index as u64),
                    size: marker_size,
                    stride: u32::try_from(marker_stride)?,
                    format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                    generation: 1,
                    bytes: vec![marker_byte; marker_stride.saturating_mul(marker_height)],
                },
            };
            let report = compose_live_cpu_frame(output.size, &[marker])
                .map_err(|error| format!("secondary output composition failed: {error:?}"))?;
            frames.push(LiveProductionComposedFrame {
                frame: report.frame,
                checksum: report.checksum,
                nonzero_pixel_bytes: report.nonzero_pixel_bytes,
            });
        }
        Ok(frames)
    }
}

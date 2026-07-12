use super::{
    detect_client_hints, import_root_window_tree_from_connection, intern_client_hint_atoms,
    name_composite_pixmaps, redirect_composite_targets,
};
use crate::prelude::*;
use crate::state::*;

use std::fs::File;
use std::os::unix::fs::FileExt;

use x11rb::protocol::shm::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

const BYTES_PER_PIXEL: usize = 4;

struct LiveShmSegment {
    id: u32,
    file: File,
    capacity: usize,
}

pub(crate) struct LiveReadbackBackend {
    path: LiveReadbackPath,
    segment: Option<LiveShmSegment>,
    shm_fallbacks: usize,
    max_readback_bytes: usize,
}

impl LiveReadbackBackend {
    pub(crate) fn negotiate(connection: &RustConnection) -> Self {
        let supported = connection
            .shm_query_version()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| {
                reply.major_version > 1 || (reply.major_version == 1 && reply.minor_version >= 2)
            });
        if supported {
            Self {
                path: LiveReadbackPath::MitShm,
                segment: None,
                shm_fallbacks: 0,
                max_readback_bytes: 0,
            }
        } else {
            Self {
                path: LiveReadbackPath::GetImageDegraded,
                segment: None,
                shm_fallbacks: 1,
                max_readback_bytes: 0,
            }
        }
    }

    pub(crate) const fn path(&self) -> LiveReadbackPath {
        self.path
    }

    pub(crate) const fn shm_fallbacks(&self) -> usize {
        self.shm_fallbacks
    }

    pub(crate) const fn max_readback_bytes(&self) -> usize {
        self.max_readback_bytes
    }

    pub(crate) fn detach(&mut self, connection: &RustConnection) {
        if let Some(segment) = self.segment.take() {
            let _ = connection.shm_detach(segment.id);
            let _ = connection.flush();
        }
    }

    pub(crate) fn read_full(
        &mut self,
        connection: &RustConnection,
        pixmap: u32,
        buffers: &mut CpuBufferStore,
    ) -> Result<CpuBufferSnapshot, XBridgeError> {
        if self.path == LiveReadbackPath::MitShm {
            match self.read_full_shm(connection, pixmap, buffers) {
                Ok(buffer) => return Ok(buffer),
                Err(_) => self.degrade(connection),
            }
        }
        let buffer = readback_composite_pixmap(connection, pixmap, buffers)?;
        self.observe_bytes(buffer.bytes.len());
        Ok(buffer)
    }

    pub(crate) fn read_patch(
        &mut self,
        connection: &RustConnection,
        pixmap: u32,
        rect: Rect,
        buffers: &mut CpuBufferStore,
    ) -> Result<CpuBufferPatchSnapshot, XBridgeError> {
        if self.path == LiveReadbackPath::MitShm {
            match self.read_patch_shm(connection, pixmap, rect, buffers) {
                Ok(buffer) => return Ok(buffer),
                Err(_) => self.degrade(connection),
            }
        }
        let buffer = readback_composite_pixmap_patch(connection, pixmap, rect, buffers)?;
        self.observe_bytes(buffer.bytes.len());
        Ok(buffer)
    }

    fn read_full_shm(
        &mut self,
        connection: &RustConnection,
        pixmap: u32,
        buffers: &mut CpuBufferStore,
    ) -> Result<CpuBufferSnapshot, XBridgeError> {
        let geometry = connection
            .get_geometry(pixmap)
            .map_err(|error| pixmap_readback_error(pixmap, error))?
            .reply()
            .map_err(|error| pixmap_readback_error(pixmap, error))?;
        let bytes =
            self.shm_get_image(connection, pixmap, 0, 0, geometry.width, geometry.height)?;
        let (depth, visual, bytes) = bytes;
        Ok(buffers.upsert_pixmap(
            pixmap,
            Size {
                width: i32::from(geometry.width),
                height: i32::from(geometry.height),
            },
            depth,
            visual,
            bytes,
        ))
    }

    fn read_patch_shm(
        &mut self,
        connection: &RustConnection,
        pixmap: u32,
        rect: Rect,
        buffers: &mut CpuBufferStore,
    ) -> Result<CpuBufferPatchSnapshot, XBridgeError> {
        let width =
            u16::try_from(rect.width).map_err(|error| pixmap_readback_error(pixmap, error))?;
        let height =
            u16::try_from(rect.height).map_err(|error| pixmap_readback_error(pixmap, error))?;
        let x = i16::try_from(rect.x).map_err(|error| pixmap_readback_error(pixmap, error))?;
        let y = i16::try_from(rect.y).map_err(|error| pixmap_readback_error(pixmap, error))?;
        let (_, _, bytes) = self.shm_get_image(connection, pixmap, x, y, width, height)?;
        buffers
            .patch_pixmap(pixmap, rect, bytes)
            .ok_or_else(|| XBridgeError::PixmapReadback {
                pixmap,
                message: "damage patch does not match its cached pixmap".to_owned(),
            })
    }

    fn shm_get_image(
        &mut self,
        connection: &RustConnection,
        pixmap: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> Result<(u8, u32, Vec<u8>), XBridgeError> {
        let expected = usize::from(width)
            .checked_mul(usize::from(height))
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| XBridgeError::PixmapReadback {
                pixmap,
                message: "MIT-SHM readback size overflow".to_owned(),
            })?;
        self.ensure_segment(connection, pixmap, expected)?;
        let segment = self.segment.as_ref().expect("SHM segment was ensured");
        let reply = connection
            .shm_get_image(
                pixmap,
                x,
                y,
                width,
                height,
                u32::MAX,
                u8::from(ImageFormat::Z_PIXMAP),
                segment.id,
                0,
            )
            .map_err(|error| pixmap_readback_error(pixmap, error))?
            .reply()
            .map_err(|error| pixmap_readback_error(pixmap, error))?;
        let size =
            usize::try_from(reply.size).map_err(|error| pixmap_readback_error(pixmap, error))?;
        if size != expected || size > segment.capacity {
            return Err(XBridgeError::PixmapReadback {
                pixmap,
                message: format!(
                    "MIT-SHM returned {size} bytes for an expected {expected}-byte XRGB image"
                ),
            });
        }
        let mut bytes = vec![0; size];
        segment
            .file
            .read_exact_at(&mut bytes, 0)
            .map_err(|error| pixmap_readback_error(pixmap, error))?;
        self.observe_bytes(size);
        Ok((reply.depth, reply.visual, bytes))
    }

    fn ensure_segment(
        &mut self,
        connection: &RustConnection,
        pixmap: u32,
        required: usize,
    ) -> Result<(), XBridgeError> {
        if self
            .segment
            .as_ref()
            .is_some_and(|segment| segment.capacity >= required)
        {
            return Ok(());
        }
        self.detach(connection);
        let id = connection
            .generate_id()
            .map_err(|error| pixmap_readback_error(pixmap, error))?;
        let size = u32::try_from(required).map_err(|error| pixmap_readback_error(pixmap, error))?;
        let reply = connection
            .shm_create_segment(id, size, false)
            .map_err(|error| pixmap_readback_error(pixmap, error))?
            .reply()
            .map_err(|error| pixmap_readback_error(pixmap, error))?;
        if reply.nfd != 1 {
            let _ = connection.shm_detach(id);
            return Err(XBridgeError::PixmapReadback {
                pixmap,
                message: format!(
                    "MIT-SHM CreateSegment returned {} file descriptors",
                    reply.nfd
                ),
            });
        }
        self.segment = Some(LiveShmSegment {
            id,
            file: reply.shm_fd.into(),
            capacity: required,
        });
        Ok(())
    }

    fn degrade(&mut self, connection: &RustConnection) {
        self.detach(connection);
        self.path = LiveReadbackPath::GetImageDegraded;
        self.shm_fallbacks = self.shm_fallbacks.saturating_add(1);
    }

    fn observe_bytes(&mut self, bytes: usize) {
        self.max_readback_bytes = self.max_readback_bytes.max(bytes);
    }
}

fn pixmap_readback_error(pixmap: u32, error: impl ToString) -> XBridgeError {
    XBridgeError::PixmapReadback {
        pixmap,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::LiveReadbackPath;

    #[test]
    fn readback_path_has_stable_evidence_names() {
        assert_eq!(LiveReadbackPath::MitShm.evidence_name(), "mit_shm");
        assert_eq!(
            LiveReadbackPath::GetImageDegraded.evidence_name(),
            "get_image_degraded"
        );
    }
}

pub fn smoke_readback_display(
    display_name: Option<&str>,
) -> Result<SmokeReadbackReport, XBridgeError> {
    capture_readback_display(display_name).map(|capture| capture.report)
}

pub fn capture_readback_display(
    display_name: Option<&str>,
) -> Result<SmokeReadbackCapture, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let mut mirror = import_root_window_tree_from_connection(&connection, screen_num)?;
    let atoms = intern_client_hint_atoms(&connection)?;
    let hints = detect_client_hints(&connection, screen_num, &mirror, atoms)?;
    mirror.apply_client_hints(&hints);
    mirror.apply_unmanaged_client_fallback();

    let targets = mirror.composite_redirect_targets();
    redirect_composite_targets(&connection, &targets)?;

    let mut pixmaps = CompositePixmapMap::default();
    name_composite_pixmaps(&connection, &targets, &mut pixmaps)?;

    let mut surface_ids = SurfaceIdMap::default();
    let mut surfaces = mirror.emit_surfaces(&mut surface_ids, &pixmaps);
    let mut buffers = CpuBufferStore::default();
    let readbacks = readback_surface_pixmaps(&connection, &mut surfaces, &mut buffers)?;
    let layers = layers_from_surfaces(&surfaces);
    let total_bytes = readbacks
        .iter()
        .map(|readback| readback.bytes.len())
        .sum::<usize>();

    Ok(SmokeReadbackCapture {
        report: SmokeReadbackReport {
            display_name: display_name.map(str::to_owned),
            mirrored_windows: mirror.windows().len(),
            surfaces: surfaces.len(),
            renderable_layers: layers.len(),
            redirect_targets: targets.len(),
            readbacks: readbacks.len(),
            total_bytes,
        },
        surfaces,
        layers,
        readbacks,
    })
}
pub fn readback_composite_pixmap<C>(
    connection: &C,
    pixmap: u32,
    buffers: &mut CpuBufferStore,
) -> Result<CpuBufferSnapshot, XBridgeError>
where
    C: Connection,
{
    let geometry = connection
        .get_geometry(pixmap)
        .map_err(|error| XBridgeError::PixmapGeometry {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapGeometry {
            pixmap,
            message: error.to_string(),
        })?;
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            pixmap,
            0,
            0,
            geometry.width,
            geometry.height,
            u32::MAX,
        )
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?;

    Ok(buffers.upsert_pixmap(
        pixmap,
        Size {
            width: i32::from(geometry.width),
            height: i32::from(geometry.height),
        },
        image.depth,
        image.visual,
        image.data,
    ))
}

pub fn readback_composite_pixmap_patch<C>(
    connection: &C,
    pixmap: u32,
    rect: Rect,
    buffers: &mut CpuBufferStore,
) -> Result<CpuBufferPatchSnapshot, XBridgeError>
where
    C: Connection,
{
    let width = u16::try_from(rect.width).map_err(|error| XBridgeError::PixmapReadback {
        pixmap,
        message: error.to_string(),
    })?;
    let height = u16::try_from(rect.height).map_err(|error| XBridgeError::PixmapReadback {
        pixmap,
        message: error.to_string(),
    })?;
    let x = i16::try_from(rect.x).map_err(|error| XBridgeError::PixmapReadback {
        pixmap,
        message: error.to_string(),
    })?;
    let y = i16::try_from(rect.y).map_err(|error| XBridgeError::PixmapReadback {
        pixmap,
        message: error.to_string(),
    })?;
    let image = connection
        .get_image(ImageFormat::Z_PIXMAP, pixmap, x, y, width, height, u32::MAX)
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::PixmapReadback {
            pixmap,
            message: error.to_string(),
        })?;

    buffers
        .patch_pixmap(pixmap, rect, image.data)
        .ok_or_else(|| XBridgeError::PixmapReadback {
            pixmap,
            message: "damage patch does not match its cached pixmap".to_owned(),
        })
}

pub fn readback_surface_pixmaps<C>(
    connection: &C,
    surfaces: &mut [SurfaceSnapshot],
    buffers: &mut CpuBufferStore,
) -> Result<Vec<CpuBufferSnapshot>, XBridgeError>
where
    C: Connection,
{
    let mut readbacks = Vec::new();

    for surface in surfaces {
        let BufferSource::XPixmap { pixmap } = surface.source else {
            continue;
        };
        let readback = readback_composite_pixmap(connection, pixmap, buffers)?;
        surface.source = BufferSource::CpuBuffer {
            handle: readback.handle,
        };
        readbacks.push(readback);
    }

    Ok(readbacks)
}

pub fn map_surface_cpu_buffers(surfaces: &mut [SurfaceSnapshot], buffers: &CpuBufferStore) {
    for surface in surfaces {
        let BufferSource::XPixmap { pixmap } = surface.source else {
            continue;
        };
        if let Some(handle) = buffers.handle_for_pixmap(pixmap) {
            surface.source = BufferSource::CpuBuffer { handle };
        }
    }
}

pub fn layers_from_surfaces(surfaces: &[SurfaceSnapshot]) -> Vec<LayerSnapshot> {
    surfaces
        .iter()
        .filter(|surface| surface.mapped && !surface.geometry.is_empty())
        .map(|surface| LayerSnapshot {
            surface: surface.surface,
            window: Some(surface.window),
            namespace: surface.namespace,
            stack_rank: surface.stack_rank,
            geometry: surface.geometry,
            source: surface.source,
            damage: surface.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: surface.generation,
            resize_sync: surface.resize_sync,
        })
        .collect()
}

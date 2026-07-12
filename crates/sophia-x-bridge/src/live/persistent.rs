use std::collections::{BTreeMap, BTreeSet};

use x11rb::connection::Connection;
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xproto::{InputFocus, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use super::{
    detect_client_hints, import_root_window_tree_from_connection, intern_client_hint_atoms,
    layers_from_surfaces, name_composite_pixmaps, readback_surface_pixmaps,
    redirect_composite_targets,
};
use crate::prelude::{Rect, XWindowId};
use crate::{
    CompositePixmapMap, CpuBufferStore, SmokeReadbackCapture, SmokeReadbackReport, SurfaceIdMap,
    XBridgeError,
};

/// Persistent form of the historical XComposite readback probe.
///
/// It intentionally exposes only Sophia snapshots and CPU buffers. XIDs remain
/// private to this adapter and can be removed when the native X Authority has
/// equivalent GL buffer handoff coverage.
pub struct LiveCompositeCapture {
    connection: RustConnection,
    screen_num: usize,
    redirected: BTreeSet<XWindowId>,
    geometry: BTreeMap<XWindowId, Rect>,
    pixmaps: CompositePixmapMap,
    surfaces: SurfaceIdMap,
    buffers: CpuBufferStore,
    focused_window: Option<u32>,
}

impl LiveCompositeCapture {
    pub fn connect(display_name: Option<&str>) -> Result<Self, XBridgeError> {
        let (connection, screen_num) =
            x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
                message: error.to_string(),
            })?;
        Ok(Self {
            connection,
            screen_num,
            redirected: BTreeSet::new(),
            geometry: BTreeMap::new(),
            pixmaps: CompositePixmapMap::default(),
            surfaces: SurfaceIdMap::default(),
            buffers: CpuBufferStore::default(),
            focused_window: None,
        })
    }

    pub fn capture(&mut self) -> Result<SmokeReadbackCapture, XBridgeError> {
        let mut mirror =
            import_root_window_tree_from_connection(&self.connection, self.screen_num)?;
        let atoms = intern_client_hint_atoms(&self.connection)?;
        let hints = detect_client_hints(&self.connection, self.screen_num, &mirror, atoms)?;
        mirror.apply_client_hints(&hints);
        mirror.apply_unmanaged_client_fallback();

        let targets = mirror.composite_redirect_targets();
        let current = targets
            .iter()
            .map(|target| target.window)
            .collect::<BTreeSet<_>>();

        for retired in self
            .redirected
            .difference(&current)
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some(pixmap) = self.pixmaps.remove_window(retired) {
                let _ = self.connection.free_pixmap(pixmap);
            }
            let _ = self.connection.composite_unredirect_window(
                retired.xid(),
                x11rb::protocol::composite::Redirect::MANUAL,
            );
            self.geometry.remove(&retired);
        }

        let new_targets = targets
            .iter()
            .copied()
            .filter(|target| !self.redirected.contains(&target.window))
            .collect::<Vec<_>>();
        redirect_composite_targets(&self.connection, &new_targets)?;

        for target in &targets {
            let next_geometry = mirror
                .windows()
                .iter()
                .find(|window| window.window == target.window)
                .map(|window| window.geometry);
            if self.geometry.get(&target.window).copied() != next_geometry {
                if let Some(pixmap) = self.pixmaps.remove_window(target.window) {
                    let _ = self.connection.free_pixmap(pixmap);
                }
                if let Some(next_geometry) = next_geometry {
                    self.geometry.insert(target.window, next_geometry);
                }
            }
        }
        name_composite_pixmaps(&self.connection, &targets, &mut self.pixmaps)?;
        self.connection
            .flush()
            .map_err(|error| XBridgeError::Connect {
                message: error.to_string(),
            })?;
        self.redirected = current;
        self.focused_window = targets.first().map(|target| target.window.xid());
        if let Some(window) = self.focused_window {
            self.connection
                .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
                .map_err(|error| XBridgeError::RoutedInput {
                    message: error.to_string(),
                })?;
        }

        let mut surfaces = mirror.emit_surfaces(&mut self.surfaces, &self.pixmaps);
        let readbacks =
            readback_surface_pixmaps(&self.connection, &mut surfaces, &mut self.buffers)?;
        let layers = layers_from_surfaces(&surfaces);
        let total_bytes = readbacks.iter().map(|buffer| buffer.bytes.len()).sum();
        Ok(SmokeReadbackCapture {
            report: SmokeReadbackReport {
                display_name: None,
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

    pub fn inject_key(
        &self,
        keycode: u8,
        pressed: bool,
        time_msec: u32,
    ) -> Result<(), XBridgeError> {
        if keycode < 8 || self.focused_window.is_none() {
            return Ok(());
        }
        self.connection
            .xtest_fake_input(
                if pressed {
                    KEY_PRESS_EVENT
                } else {
                    KEY_RELEASE_EVENT
                },
                keycode,
                time_msec,
                x11rb::NONE,
                0,
                0,
                0,
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        self.connection
            .flush()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })
    }
}

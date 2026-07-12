use std::collections::{BTreeMap, BTreeSet};

use x11rb::connection::Connection;
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::damage::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xproto::{InputFocus, KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use super::{
    create_damage_trackers, detect_client_hints, import_root_window_tree_from_connection,
    intern_client_hint_atoms, layers_from_surfaces, map_surface_cpu_buffers,
    name_composite_pixmaps, readback_composite_pixmap, readback_composite_pixmap_patch,
    redirect_composite_targets,
};
use crate::prelude::{Rect, XWindowId};
use crate::{
    CompositePixmapMap, CpuBufferStore, DamageTracker, LiveCompositeCaptureFrame,
    LiveCpuBufferUpdate, SmokeReadbackReport, SurfaceIdMap, XBridgeError, XDamageEvent,
};

const FULL_REPLACEMENT_DAMAGE_PERCENT: i64 = 50;

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
    damage: DamageTracker,
    surfaces: SurfaceIdMap,
    buffers: CpuBufferStore,
    focused_window: Option<u32>,
}

pub struct LiveXTestInput {
    connection: RustConnection,
}

impl LiveXTestInput {
    pub fn connect(display_name: Option<&str>) -> Result<Self, XBridgeError> {
        let (connection, _) =
            x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
                message: error.to_string(),
            })?;
        Ok(Self { connection })
    }

    pub fn inject_key(
        &self,
        keycode: u8,
        pressed: bool,
        source_time_msec: u32,
    ) -> Result<(), XBridgeError> {
        if keycode < 8 {
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
                xtest_delivery_delay_msec(source_time_msec),
                x11rb::NONE,
                0,
                0,
                0,
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
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
            damage: DamageTracker::default(),
            surfaces: SurfaceIdMap::default(),
            buffers: CpuBufferStore::default(),
            focused_window: None,
        })
    }

    pub fn capture(&mut self) -> Result<LiveCompositeCaptureFrame, XBridgeError> {
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
                self.buffers.remove_pixmap(pixmap);
                let _ = self.connection.free_pixmap(pixmap);
            }
            if let Some(damage) = self.damage.remove_window(retired) {
                let _ = self.connection.damage_destroy(damage);
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

        let mut full_replacements = new_targets
            .iter()
            .map(|target| target.window)
            .collect::<BTreeSet<_>>();

        for target in &targets {
            let next_geometry = mirror
                .windows()
                .iter()
                .find(|window| window.window == target.window)
                .map(|window| window.geometry);
            if self.geometry.get(&target.window).copied() != next_geometry {
                if let Some(pixmap) = self.pixmaps.remove_window(target.window) {
                    self.buffers.remove_pixmap(pixmap);
                    let _ = self.connection.free_pixmap(pixmap);
                }
                if let Some(damage) = self.damage.remove_window(target.window) {
                    let _ = self.connection.damage_destroy(damage);
                }
                if let Some(next_geometry) = next_geometry {
                    self.geometry.insert(target.window, next_geometry);
                }
                full_replacements.insert(target.window);
            }
        }
        name_composite_pixmaps(&self.connection, &targets, &mut self.pixmaps)?;
        create_damage_trackers(&self.connection, &targets, &mut self.damage)?;
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
                })?
                .check()
                .map_err(|error| XBridgeError::RoutedInput {
                    message: error.to_string(),
                })?;
        }

        while let Some(event) =
            self.connection
                .poll_for_event()
                .map_err(|error| XBridgeError::Connect {
                    message: error.to_string(),
                })?
        {
            if let Some(event) = XDamageEvent::from_x11_event(&event, &self.damage) {
                self.damage.apply_event(event);
            }
        }

        let mut surfaces = mirror.emit_surfaces(&mut self.surfaces, &self.pixmaps);
        let mut updates = Vec::new();
        for target in &targets {
            let Some(pixmap) = self.pixmaps.pixmap_for_window(target.window) else {
                continue;
            };
            let Some(geometry) = self.geometry.get(&target.window).copied() else {
                continue;
            };
            let damage = self.damage.drain_damage(target.window);
            let full_replacement = full_replacements.contains(&target.window)
                || self.buffers.handle_for_pixmap(pixmap).is_none();
            let damage_rect = bounding_damage_rect(&damage.rects, geometry);
            if !full_replacement && damage_rect.is_none() {
                continue;
            }
            if let Some(damage) = self.damage.damage_for_window(target.window) {
                self.connection
                    .damage_subtract(damage, x11rb::NONE, x11rb::NONE)
                    .map_err(|error| XBridgeError::DamageCreate {
                        window: target.window.xid(),
                        damage,
                        message: error.to_string(),
                    })?
                    .check()
                    .map_err(|error| XBridgeError::DamageCreate {
                        window: target.window.xid(),
                        damage,
                        message: error.to_string(),
                    })?;
            }
            let replace_for_damage =
                damage_rect.is_some_and(|rect| damage_requires_full_replacement(rect, geometry));
            if full_replacement || replace_for_damage {
                updates.push(LiveCpuBufferUpdate::Replace(readback_composite_pixmap(
                    &self.connection,
                    pixmap,
                    &mut self.buffers,
                )?));
            } else if let Some(rect) = damage_rect {
                updates.push(LiveCpuBufferUpdate::Patch(readback_composite_pixmap_patch(
                    &self.connection,
                    pixmap,
                    rect,
                    &mut self.buffers,
                )?));
            }
        }
        map_surface_cpu_buffers(&mut surfaces, &self.buffers);
        let layers = layers_from_surfaces(&surfaces);
        let total_bytes = updates.iter().map(LiveCpuBufferUpdate::byte_len).sum();
        Ok(LiveCompositeCaptureFrame {
            report: SmokeReadbackReport {
                display_name: None,
                mirrored_windows: mirror.windows().len(),
                surfaces: surfaces.len(),
                renderable_layers: layers.len(),
                redirect_targets: targets.len(),
                readbacks: updates.len(),
                total_bytes,
            },
            surfaces,
            layers,
            updates,
        })
    }
}

fn bounding_damage_rect(rects: &[Rect], geometry: Rect) -> Option<Rect> {
    let width = geometry.width.max(0);
    let height = geometry.height.max(0);
    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;
    for rect in rects {
        let rect_left = rect.x.clamp(0, width);
        let rect_top = rect.y.clamp(0, height);
        let rect_right = rect.x.saturating_add(rect.width).clamp(0, width);
        let rect_bottom = rect.y.saturating_add(rect.height).clamp(0, height);
        if rect_right <= rect_left || rect_bottom <= rect_top {
            continue;
        }
        left = left.min(rect_left);
        top = top.min(rect_top);
        right = right.max(rect_right);
        bottom = bottom.max(rect_bottom);
    }
    (right > left && bottom > top).then_some(Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}

fn damage_requires_full_replacement(damage: Rect, geometry: Rect) -> bool {
    let damage_area =
        i64::from(damage.width.max(0)).saturating_mul(i64::from(damage.height.max(0)));
    let surface_area =
        i64::from(geometry.width.max(0)).saturating_mul(i64::from(geometry.height.max(0)));
    surface_area == 0
        || damage_area.saturating_mul(100)
            >= surface_area.saturating_mul(FULL_REPLACEMENT_DAMAGE_PERCENT)
}

fn xtest_delivery_delay_msec(_source_time_msec: u32) -> u32 {
    // XTEST FakeInput's `time` field is a delivery delay, not an event
    // timestamp. Reusing libinput's monotonic timestamp can postpone delivery
    // for days, so live routed input must always request immediate delivery.
    x11rb::CURRENT_TIME
}

#[cfg(test)]
mod tests {
    use super::{
        bounding_damage_rect, damage_requires_full_replacement, xtest_delivery_delay_msec,
    };
    use crate::prelude::Rect;

    #[test]
    fn libinput_timestamp_is_not_reused_as_xtest_delivery_delay() {
        assert_eq!(xtest_delivery_delay_msec(u32::MAX), 0);
    }

    #[test]
    fn damage_is_clipped_and_coalesced_to_one_surface_local_rect() {
        let geometry = Rect {
            x: 80,
            y: 60,
            width: 100,
            height: 50,
        };
        assert_eq!(
            bounding_damage_rect(
                &[
                    Rect {
                        x: -5,
                        y: 3,
                        width: 15,
                        height: 7,
                    },
                    Rect {
                        x: 90,
                        y: 40,
                        width: 20,
                        height: 20,
                    },
                ],
                geometry,
            ),
            Some(Rect {
                x: 0,
                y: 3,
                width: 100,
                height: 47,
            })
        );
    }

    #[test]
    fn half_surface_damage_uses_full_replacement() {
        let geometry = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        };
        assert!(damage_requires_full_replacement(
            Rect {
                width: 100,
                height: 40,
                ..geometry
            },
            geometry
        ));
        assert!(!damage_requires_full_replacement(
            Rect {
                width: 20,
                height: 10,
                ..geometry
            },
            geometry
        ));
    }
}

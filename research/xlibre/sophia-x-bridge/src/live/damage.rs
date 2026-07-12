use crate::prelude::*;
use crate::state::*;

pub fn create_damage_trackers<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
    tracker: &mut DamageTracker,
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    connection
        .damage_query_version(1, 1)
        .map_err(|error| XBridgeError::DamageVersion {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::DamageVersion {
            message: error.to_string(),
        })?;

    for target in targets {
        if tracker.damage_for_window(target.window).is_some() {
            continue;
        }

        let damage = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;

        connection
            .damage_create(damage, target.window.xid(), ReportLevel::BOUNDING_BOX)
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

        tracker.insert_damage(target.window, damage);
    }

    Ok(())
}

pub fn emit_damage_frame(
    tracker: &mut DamageTracker,
    output: OutputId,
    frame_serial: u64,
    buffer_age: u32,
    root_generation: u64,
    surfaces: &[SurfaceSnapshot],
) -> DamageFrame {
    let mut affected_surfaces = Vec::new();
    let mut seen_surfaces = BTreeSet::new();
    let mut damage = Region::empty();

    for surface in surfaces {
        let Some(client) = surface.client else {
            continue;
        };

        let local_damage = tracker.drain_damage(client);
        if local_damage.is_empty() || !surface.mapped {
            continue;
        }

        let translated = translate_region(&local_damage, surface.geometry.x, surface.geometry.y);
        if translated.is_empty() {
            continue;
        }

        if seen_surfaces.insert(surface.surface) {
            affected_surfaces.push(surface.surface);
        }
        damage.extend(&translated);
    }

    DamageFrame {
        output,
        frame_serial,
        buffer_age,
        root_generation,
        affected_surfaces,
        damage,
    }
}
fn translate_region(region: &Region, dx: i32, dy: i32) -> Region {
    let mut translated = Region::empty();
    for rect in &region.rects {
        translated.push(Rect {
            x: rect.x.saturating_add(dx),
            y: rect.y.saturating_add(dy),
            width: rect.width,
            height: rect.height,
        });
    }
    translated
}

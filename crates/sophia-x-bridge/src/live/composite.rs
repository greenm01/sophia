use crate::prelude::*;
use crate::state::*;

pub fn redirect_composite_targets<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    connection
        .composite_query_version(0, 4)
        .map_err(|error| XBridgeError::CompositeVersion {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::CompositeVersion {
            message: error.to_string(),
        })?;

    for target in targets {
        connection
            .composite_redirect_window(target.window.xid(), target.update.to_x11())
            .map_err(|error| XBridgeError::CompositeRedirect {
                window: target.window.xid(),
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::CompositeRedirect {
                window: target.window.xid(),
                message: error.to_string(),
            })?;
    }

    Ok(())
}

pub fn name_composite_pixmaps<C>(
    connection: &C,
    targets: &[CompositeRedirectTarget],
    pixmaps: &mut CompositePixmapMap,
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    for target in targets {
        if pixmaps.pixmap_for_window(target.window).is_some() {
            continue;
        }

        let pixmap = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;

        connection
            .composite_name_window_pixmap(target.window.xid(), pixmap)
            .map_err(|error| XBridgeError::CompositeNamePixmap {
                window: target.window.xid(),
                pixmap,
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::CompositeNamePixmap {
                window: target.window.xid(),
                pixmap,
                message: error.to_string(),
            })?;

        pixmaps.insert_named_pixmap(target.window, pixmap);
    }

    Ok(())
}

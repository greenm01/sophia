use super::read_atom_list_property;
use crate::prelude::*;
use crate::state::*;

pub fn polite_close_surface<C>(
    connection: &C,
    mirror: &XMirrorState,
    surfaces: &SurfaceIdMap,
    atoms: XAtoms,
    surface: SurfaceId,
    timestamp: u32,
) -> Result<PoliteCloseOutcome, XBridgeError>
where
    C: Connection,
{
    let target = close_target_for_surface(mirror, surfaces, surface).ok_or_else(|| {
        XBridgeError::PoliteClose {
            window: 0,
            message: format!("surface {:?} has no X close target", surface),
        }
    })?;

    polite_close_window(connection, target, atoms, timestamp)
}

pub fn polite_close_window<C>(
    connection: &C,
    window: XWindowId,
    atoms: XAtoms,
    timestamp: u32,
) -> Result<PoliteCloseOutcome, XBridgeError>
where
    C: Connection,
{
    if !window_supports_wm_delete(connection, window, atoms)? {
        return Ok(PoliteCloseOutcome::UnsupportedProtocol { window });
    }

    let event = build_wm_delete_client_message(window, atoms, timestamp);
    connection
        .send_event(false, window.xid(), EventMask::NO_EVENT, event)
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::PoliteClose {
            window: window.xid(),
            message: error.to_string(),
        })?;

    Ok(PoliteCloseOutcome::SentDeleteWindow { window })
}

pub fn build_wm_delete_client_message(
    window: XWindowId,
    atoms: XAtoms,
    timestamp: u32,
) -> ClientMessageEvent {
    ClientMessageEvent::new(
        32,
        window.xid(),
        atoms.wm_protocols,
        ClientMessageData::from([atoms.wm_delete_window, timestamp, 0, 0, 0]),
    )
}

fn window_supports_wm_delete<C>(
    connection: &C,
    window: XWindowId,
    atoms: XAtoms,
) -> Result<bool, XBridgeError>
where
    C: Connection,
{
    Ok(
        read_atom_list_property(connection, window.xid(), atoms.wm_protocols)?
            .into_iter()
            .any(|atom| atom == atoms.wm_delete_window),
    )
}

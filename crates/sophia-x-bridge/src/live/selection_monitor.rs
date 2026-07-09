use super::intern_atom;
use crate::prelude::*;
use crate::state::*;

pub fn intern_selection_atoms<C>(connection: &C) -> Result<XSelectionAtoms, XBridgeError>
where
    C: Connection,
{
    Ok(XSelectionAtoms {
        primary: intern_atom(connection, "PRIMARY")?,
        secondary: intern_atom(connection, "SECONDARY")?,
        clipboard: intern_atom(connection, "CLIPBOARD")?,
    })
}

pub fn select_selection_owner_events<C>(
    connection: &C,
    window: Window,
    selections: &[Atom],
) -> Result<(), XBridgeError>
where
    C: Connection,
{
    let mask = SelectionEventMask::SET_SELECTION_OWNER
        | SelectionEventMask::SELECTION_WINDOW_DESTROY
        | SelectionEventMask::SELECTION_CLIENT_CLOSE;

    for selection in selections {
        connection
            .xfixes_select_selection_input(window, *selection, mask)
            .map_err(|error| XBridgeError::SelectionMonitor {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::SelectionMonitor {
                message: error.to_string(),
            })?;
    }

    connection
        .flush()
        .map_err(|error| XBridgeError::SelectionMonitor {
            message: error.to_string(),
        })?;

    Ok(())
}

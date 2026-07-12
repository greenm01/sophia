use crate::prelude::*;

pub(crate) fn wrap_xid(window: Window) -> XWindowId {
    XWindowId::new(window, 1)
}

pub(crate) fn nonzero_window(window: Window) -> Option<Window> {
    (window != 0).then_some(window)
}

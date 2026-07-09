//! Cross-namespace portal policy reducers.
//!
//! Portal code is intentionally off the compositor hot path. It turns
//! namespaced transfer requests into bounded commands that the runtime or
//! X bridge can execute without granting the policy code raw X authority.

mod clipboard;
mod drag_and_drop;
mod file_handoff;
mod notification;
mod screen_capture;
mod types;
mod uri_open;

mod prelude {
    pub(crate) use std::collections::BTreeMap;

    pub(crate) use sophia_protocol::{
        NamespaceId, PortalDecision, PortalTransfer, PortalTransferId, PortalTransferKind,
    };
}

pub use clipboard::*;
pub use drag_and_drop::*;
pub use file_handoff::*;
pub use notification::*;
pub use screen_capture::*;
pub use types::*;
pub use uri_open::*;

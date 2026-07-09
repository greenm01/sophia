use crate::live::intern_atom;
use crate::prelude::*;
use crate::routed_input::drain_pending_events;
use crate::state::*;

use super::dispatch::dispatch_clipboard_selection_request_event;
use super::handoff::{
    MAX_CLIPBOARD_TEXT_HANDOFF_BYTES, apply_clipboard_selection_failure,
    apply_clipboard_selection_handoff, clipboard_selection_failure_notify,
    clipboard_selection_text_handoff_notify,
};
use super::live_support::{
    clipboard_smoke_mirror, create_clipboard_smoke_window, wait_for_selection_notify,
    wait_for_selection_request,
};

mod flow;
mod report;
mod setup;

pub use flow::smoke_live_clipboard_portal;
pub use report::*;

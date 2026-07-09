use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveClipboardPortalSmokeReport {
    pub display_name: Option<String>,
    pub owner: XWindowId,
    pub requestor: XWindowId,
    pub selection: Atom,
    pub target: Atom,
    pub denied_property: Atom,
    pub approved_property: Atom,
    pub failure_property: Atom,
    pub success_property: Atom,
    pub handoff_bytes: usize,
    pub observed_handoff_bytes: usize,
}

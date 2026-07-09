use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutNodeSnapshot {
    pub surface: SurfaceId,
    pub workspace: WorkspaceId,
    pub kind: LayoutNodeKind,
    pub capabilities: LayoutNodeCapabilities,
    pub state: LayoutNodeState,
    pub constraints: SurfaceConstraints,
    pub geometry: Rect,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutNodeKind {
    Toplevel,
    Dialog,
    Utility,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutNodeCapabilities {
    pub movable: bool,
    pub resizable: bool,
    pub focusable: bool,
    pub closable: bool,
    pub fullscreenable: bool,
}

impl LayoutNodeCapabilities {
    pub const STANDARD_TOPLEVEL: Self = Self {
        movable: true,
        resizable: true,
        focusable: true,
        closable: true,
        fullscreenable: true,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutNodeState {
    pub focused: bool,
    pub urgent: bool,
    pub fullscreen: bool,
    pub floating: bool,
    pub visible: bool,
}

impl LayoutNodeState {
    pub const NORMAL: Self = Self {
        focused: false,
        urgent: false,
        fullscreen: false,
        floating: false,
        visible: true,
    };
}

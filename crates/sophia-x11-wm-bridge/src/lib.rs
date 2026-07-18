//! Blind legacy-X11 window-manager policy translation.
//!
//! Synthetic XIDs are private bridge handles. They never identify client X
//! resources and carry no namespace or metadata information.

use std::collections::BTreeMap;

use sophia_protocol::{
    LayoutNodeSnapshot, Rect, Size, SurfaceId, SurfacePlacement, SurfaceSizeRequest, TransactionId,
    Transform, WM_API_VERSION, WmActionId, WmBindingRegistration, WmCapabilities, WmCommand,
    WmHello, WmModifierMask, WmRequestKind, WmRequestPacket, WmResponsePacket, WmSessionAction,
    WmSessionDescriptor, WorkspaceId,
};

#[cfg(unix)]
mod runtime;

#[cfg(unix)]
pub use runtime::*;

pub const SYNTHETIC_ROOT_XID: u32 = sophia_x_authority::X_SETUP_DEFAULT_ROOT;
pub const FIRST_SYNTHETIC_WINDOW_XID: u32 = 0x1_0000;
pub const MAX_SYNTHETIC_WINDOWS: usize = 4_096;
pub const MAX_LEGACY_WM_REQUESTS: usize = 8_192;

pub const XMONAD_ACTION_FOCUS_NEXT: u64 = 1;
pub const XMONAD_ACTION_FOCUS_PREVIOUS: u64 = 2;
pub const XMONAD_ACTION_NEXT_LAYOUT: u64 = 3;
pub const XMONAD_ACTION_VIEW_WORKSPACE_BASE: u64 = 0x100;
pub const XMONAD_ACTION_MOVE_WORKSPACE_BASE: u64 = 0x200;
pub const XMONAD_ACTION_TERMINAL: u64 = 0x300;
pub const XMONAD_ACTION_CLOSE: u64 = 0x301;
pub const XMONAD_ACTION_LAUNCHER: u64 = 0x302;
pub const XMONAD_ACTION_FIREFOX: u64 = 0x303;
pub const XMONAD_ACTION_LOGOUT: u64 = 0x304;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LegacyWmProfile {
    #[default]
    LayoutOnly,
    Xmonad,
}

impl LegacyWmProfile {
    pub fn hello(self) -> WmHello {
        let bindings = match self {
            Self::LayoutOnly => Vec::new(),
            Self::Xmonad => xmonad_bindings(),
        };
        WmHello {
            api_version: WM_API_VERSION,
            capabilities: WmCapabilities::all_supported(),
            bindings,
        }
    }
}

fn xmonad_bindings() -> Vec<WmBindingRegistration> {
    let super_only = WmModifierMask {
        bits: WmModifierMask::SUPER,
    };
    let super_shift = WmModifierMask {
        bits: WmModifierMask::SUPER | WmModifierMask::SHIFT,
    };
    let mut bindings = vec![
        binding(XMONAD_ACTION_FOCUS_NEXT, 36, super_only),
        binding(XMONAD_ACTION_FOCUS_PREVIOUS, 37, super_only),
        binding(XMONAD_ACTION_NEXT_LAYOUT, 57, super_only),
        binding(XMONAD_ACTION_TERMINAL, 28, super_shift),
        binding(XMONAD_ACTION_CLOSE, 46, super_shift),
        binding(XMONAD_ACTION_LAUNCHER, 25, super_only),
        binding(XMONAD_ACTION_FIREFOX, 33, super_only),
        binding(XMONAD_ACTION_LOGOUT, 16, super_shift),
    ];
    for slot in 1..=9_u64 {
        let keycode = match slot {
            1 => 2,
            2 => 3,
            3 => 4,
            4 => 5,
            5 => 6,
            6 => 7,
            7 => 8,
            8 => 9,
            9 => 10,
            _ => unreachable!(),
        };
        bindings.push(binding(
            XMONAD_ACTION_VIEW_WORKSPACE_BASE + slot,
            keycode,
            super_only,
        ));
        bindings.push(binding(
            XMONAD_ACTION_MOVE_WORKSPACE_BASE + slot,
            keycode,
            super_shift,
        ));
    }
    bindings
}

fn binding(action: u64, keycode: u32, modifiers: WmModifierMask) -> WmBindingRegistration {
    WmBindingRegistration {
        action: WmActionId::from_raw(action),
        keycode,
        modifiers,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SyntheticXWindowId(u32);

impl SyntheticXWindowId {
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntheticXEvent {
    RootConfigured {
        bounds: Rect,
    },
    MapRequest {
        window: SyntheticXWindowId,
    },
    ConfigureNotify {
        window: SyntheticXWindowId,
        geometry: Rect,
    },
    UnmapNotify {
        window: SyntheticXWindowId,
    },
    DestroyNotify {
        window: SyntheticXWindowId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeEngineUpdate {
    pub transaction: TransactionId,
    pub events: Vec<SyntheticXEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyWmRequest {
    ConfigureWindow {
        window: SyntheticXWindowId,
        geometry: Rect,
        z_index: i32,
    },
    FocusWindow {
        window: SyntheticXWindowId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X11WmBridgeError {
    SyntheticWindowLimit,
    UnknownSyntheticWindow,
    InvalidGeometry,
    RequestLimit,
    UnsupportedAction,
    UnavailableSessionAction,
}
pub fn translate_xmonad_profile_action(
    request: &WmRequestPacket,
    session: &WmSessionDescriptor,
) -> Result<Option<WmResponsePacket>, X11WmBridgeError> {
    let WmRequestKind::ActionActivated(activation) = &request.kind else {
        return Ok(None);
    };
    let raw = activation.action.raw();
    let command = if (XMONAD_ACTION_VIEW_WORKSPACE_BASE + 1..=XMONAD_ACTION_VIEW_WORKSPACE_BASE + 9)
        .contains(&raw)
    {
        let workspace = WorkspaceId::from_raw(raw - XMONAD_ACTION_VIEW_WORKSPACE_BASE);
        if !session.workspaces.contains(&workspace) {
            return Err(X11WmBridgeError::UnsupportedAction);
        }
        WmCommand::ActivateWorkspace {
            output: activation.output,
            workspace,
        }
    } else if (XMONAD_ACTION_MOVE_WORKSPACE_BASE + 1..=XMONAD_ACTION_MOVE_WORKSPACE_BASE + 9)
        .contains(&raw)
    {
        let workspace = WorkspaceId::from_raw(raw - XMONAD_ACTION_MOVE_WORKSPACE_BASE);
        let surface = activation
            .focused_surface
            .ok_or(X11WmBridgeError::UnsupportedAction)?;
        if !session.workspaces.contains(&workspace) {
            return Err(X11WmBridgeError::UnsupportedAction);
        }
        WmCommand::AssignWorkspace { surface, workspace }
    } else {
        let action = match raw {
            XMONAD_ACTION_TERMINAL => WmSessionAction::LaunchTerminal,
            XMONAD_ACTION_CLOSE => WmSessionAction::CloseFocused,
            XMONAD_ACTION_LAUNCHER => WmSessionAction::LaunchApplicationMenu,
            XMONAD_ACTION_FIREFOX => WmSessionAction::LaunchFirefox,
            XMONAD_ACTION_LOGOUT => WmSessionAction::Logout,
            XMONAD_ACTION_FOCUS_NEXT | XMONAD_ACTION_FOCUS_PREVIOUS | XMONAD_ACTION_NEXT_LAYOUT => {
                return Ok(None);
            }
            _ => return Err(X11WmBridgeError::UnsupportedAction),
        };
        if !session.session_actions.contains(&action) {
            return Err(X11WmBridgeError::UnavailableSessionAction);
        }
        WmCommand::RequestSessionAction {
            action,
            target: (action == WmSessionAction::CloseFocused)
                .then_some(activation.focused_surface)
                .flatten(),
        }
    };
    Ok(Some(WmResponsePacket {
        transaction: request.transaction,
        commands: vec![command],
        timeout_msec: 300,
    }))
}

#[derive(Debug)]
pub struct X11WmBridgeState {
    next_xid: u32,
    surface_to_window: BTreeMap<SurfaceId, SyntheticXWindowId>,
    window_to_node: BTreeMap<SyntheticXWindowId, LayoutNodeSnapshot>,
    root_bounds: Option<Rect>,
}

impl Default for X11WmBridgeState {
    fn default() -> Self {
        Self {
            next_xid: FIRST_SYNTHETIC_WINDOW_XID,
            surface_to_window: BTreeMap::new(),
            window_to_node: BTreeMap::new(),
            root_bounds: None,
        }
    }
}

impl X11WmBridgeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn synthetic_window(&self, surface: SurfaceId) -> Option<SyntheticXWindowId> {
        self.surface_to_window.get(&surface).copied()
    }

    pub fn synthetic_window_count(&self) -> usize {
        self.surface_to_window.len()
    }

    pub fn synthetic_geometry(&self, window: SyntheticXWindowId) -> Option<Rect> {
        self.window_to_node.get(&window).map(|node| node.geometry)
    }

    pub fn apply_engine_request(
        &mut self,
        request: &WmRequestPacket,
    ) -> Result<BridgeEngineUpdate, X11WmBridgeError> {
        let mut events = Vec::new();
        match &request.kind {
            WmRequestKind::ManageSurface(manage) => {
                self.update_root(manage.bounds, &mut events);
                let (window, created) = self.upsert_node(manage.node.clone())?;
                events.push(if created {
                    SyntheticXEvent::MapRequest { window }
                } else {
                    SyntheticXEvent::ConfigureNotify {
                        window,
                        geometry: manage.node.geometry,
                    }
                });
            }
            WmRequestKind::RelayoutWorkspace(relayout) => {
                self.update_root(relayout.bounds, &mut events);
                for node in &relayout.nodes {
                    let (window, created) = self.upsert_node(node.clone())?;
                    events.push(if created {
                        SyntheticXEvent::MapRequest { window }
                    } else {
                        SyntheticXEvent::ConfigureNotify {
                            window,
                            geometry: node.geometry,
                        }
                    });
                }
            }
            WmRequestKind::SurfaceRemoved { surface, .. } => {
                if let Some(window) = self.surface_to_window.remove(surface) {
                    self.window_to_node.remove(&window);
                    events.push(SyntheticXEvent::UnmapNotify { window });
                    events.push(SyntheticXEvent::DestroyNotify { window });
                }
            }
            WmRequestKind::ActionActivated(activation) => {
                for node in &activation.nodes {
                    let (window, created) = self.upsert_node(node.clone())?;
                    events.push(if created {
                        SyntheticXEvent::MapRequest { window }
                    } else {
                        SyntheticXEvent::ConfigureNotify {
                            window,
                            geometry: node.geometry,
                        }
                    });
                }
            }
        }
        Ok(BridgeEngineUpdate {
            transaction: request.transaction,
            events,
        })
    }

    pub fn translate_legacy_requests(
        &self,
        transaction: TransactionId,
        requests: &[LegacyWmRequest],
        timeout_msec: u32,
    ) -> Result<WmResponsePacket, X11WmBridgeError> {
        if requests.len() > MAX_LEGACY_WM_REQUESTS {
            return Err(X11WmBridgeError::RequestLimit);
        }
        let mut commands = Vec::with_capacity(requests.len().saturating_mul(2));
        for request in requests {
            match *request {
                LegacyWmRequest::ConfigureWindow {
                    window,
                    geometry,
                    z_index,
                } => {
                    if geometry.is_empty() {
                        return Err(X11WmBridgeError::InvalidGeometry);
                    }
                    let node = self
                        .window_to_node
                        .get(&window)
                        .ok_or(X11WmBridgeError::UnknownSyntheticWindow)?;
                    let size = clamp_size(
                        Size {
                            width: geometry.width,
                            height: geometry.height,
                        },
                        node.constraints.min_size,
                        node.constraints.max_size,
                    );
                    let geometry = Rect {
                        width: size.width,
                        height: size.height,
                        ..geometry
                    };
                    commands.push(WmCommand::ConfigureSurface(SurfaceSizeRequest {
                        surface: node.surface,
                        size,
                    }));
                    commands.push(WmCommand::RenderSurface(SurfacePlacement {
                        surface: node.surface,
                        geometry,
                        z_index,
                        crop: None,
                        transform: Transform::IDENTITY,
                    }));
                }
                LegacyWmRequest::FocusWindow { window } => {
                    let node = self
                        .window_to_node
                        .get(&window)
                        .ok_or(X11WmBridgeError::UnknownSyntheticWindow)?;
                    if node.capabilities.focusable {
                        commands.push(WmCommand::FocusSurface(node.surface));
                    }
                }
            }
        }
        Ok(WmResponsePacket {
            transaction,
            commands,
            timeout_msec,
        })
    }

    fn update_root(&mut self, bounds: Rect, events: &mut Vec<SyntheticXEvent>) {
        if self.root_bounds != Some(bounds) {
            self.root_bounds = Some(bounds);
            events.push(SyntheticXEvent::RootConfigured { bounds });
        }
    }

    fn upsert_node(
        &mut self,
        node: LayoutNodeSnapshot,
    ) -> Result<(SyntheticXWindowId, bool), X11WmBridgeError> {
        if let Some(window) = self.surface_to_window.get(&node.surface).copied() {
            self.window_to_node.insert(window, node);
            return Ok((window, false));
        }
        if self.surface_to_window.len() >= MAX_SYNTHETIC_WINDOWS {
            return Err(X11WmBridgeError::SyntheticWindowLimit);
        }
        let window = SyntheticXWindowId(self.next_xid);
        self.next_xid = self
            .next_xid
            .checked_add(1)
            .ok_or(X11WmBridgeError::SyntheticWindowLimit)?;
        self.surface_to_window.insert(node.surface, window);
        self.window_to_node.insert(window, node);
        Ok((window, true))
    }
}

fn clamp_size(size: Size, min_size: Option<Size>, max_size: Option<Size>) -> Size {
    let mut width = size.width;
    let mut height = size.height;
    if let Some(minimum) = min_size {
        width = width.max(minimum.width);
        height = height.max(minimum.height);
    }
    if let Some(maximum) = max_size {
        width = width.min(maximum.width);
        height = height.min(maximum.height);
    }
    Size { width, height }
}

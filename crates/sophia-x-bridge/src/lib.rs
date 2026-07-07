use core::fmt;
use std::collections::{BTreeSet, VecDeque};

use sophia_protocol::{LayerSnapshot, NamespaceId, Rect, XWindowId, XWindowMirror};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, MapState, Place, Window};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XMirrorState {
    windows: Vec<XWindowMirror>,
}

impl XMirrorState {
    pub fn ingest_window(&mut self, mirror: XWindowMirror) {
        self.windows.push(mirror);
    }

    pub fn windows(&self) -> &[XWindowMirror] {
        &self.windows
    }

    pub fn apply_event(&mut self, event: XMirrorEvent) {
        match event {
            XMirrorEvent::Map { window } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.mapped = true;
                }
            }
            XMirrorEvent::Unmap { window } => {
                if let Some(mirror) = self.window_mut(window) {
                    mirror.mapped = false;
                }
            }
            XMirrorEvent::Destroy { window } => {
                self.remove_window(window);
            }
            XMirrorEvent::Configure {
                window,
                above_sibling,
                ..
            } => {
                self.apply_restack(window, above_sibling);
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Reparent { window, parent } => {
                self.reparent_window(window, parent);
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Property { window, .. } => {
                self.mark_metadata_stale(window);
            }
            XMirrorEvent::Restack { window, place } => {
                self.apply_circulate(window, place);
                self.mark_metadata_stale(window);
            }
        }
    }

    pub fn apply_client_hints(&mut self, hints: &XClientHints) {
        let client_windows = hints
            .ewmh_clients
            .iter()
            .chain(hints.icccm_clients.iter())
            .copied()
            .collect::<BTreeSet<_>>();

        for client in client_windows {
            let toplevel = self.toplevel_for_client(client).unwrap_or(client);

            if let Some(client_mirror) = self.window_mut(client) {
                client_mirror.client = Some(client);
                client_mirror.toplevel = Some(toplevel);
            }

            if let Some(toplevel_mirror) = self.window_mut(toplevel) {
                toplevel_mirror.client = Some(client);
                toplevel_mirror.toplevel = Some(toplevel);
            }
        }
    }

    pub fn emit_layers(&self) -> Vec<LayerSnapshot> {
        Vec::new()
    }

    fn window_mut(&mut self, window: XWindowId) -> Option<&mut XWindowMirror> {
        self.windows
            .iter_mut()
            .find(|mirror| mirror.window == window)
    }

    fn remove_window(&mut self, window: XWindowId) {
        self.windows.retain(|mirror| mirror.window != window);
        for mirror in &mut self.windows {
            mirror.children.retain(|child| *child != window);
        }
    }

    fn reparent_window(&mut self, window: XWindowId, parent: Option<XWindowId>) {
        for mirror in &mut self.windows {
            mirror.children.retain(|child| *child != window);
        }

        if let Some(mirror) = self.window_mut(window) {
            mirror.parent = parent;
        }

        if let Some(parent) = parent {
            if let Some(parent) = self.window_mut(parent) {
                if !parent.children.contains(&window) {
                    parent.children.push(window);
                }
            }
        }
    }

    fn apply_restack(&mut self, window: XWindowId, above_sibling: Option<XWindowId>) {
        let stack_rank = above_sibling
            .and_then(|sibling| self.windows.iter().find(|mirror| mirror.window == sibling))
            .map_or(0, |sibling| sibling.stack_rank.saturating_add(1));

        if let Some(mirror) = self.window_mut(window) {
            mirror.stack_rank = stack_rank;
        }
    }

    fn apply_circulate(&mut self, window: XWindowId, place: RestackPlace) {
        let rank = match place {
            RestackPlace::OnTop => self
                .windows
                .iter()
                .map(|mirror| mirror.stack_rank)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            RestackPlace::OnBottom => 0,
        };

        if let Some(mirror) = self.window_mut(window) {
            mirror.stack_rank = rank;
        }
    }

    fn mark_metadata_stale(&mut self, window: XWindowId) {
        if let Some(mirror) = self.window_mut(window) {
            mirror.stale_metadata = mirror.stale_metadata.saturating_add(1);
        }
    }

    fn toplevel_for_client(&self, client: XWindowId) -> Option<XWindowId> {
        let mut current = client;

        loop {
            let mirror = self
                .windows
                .iter()
                .find(|mirror| mirror.window == current)?;
            let Some(parent) = mirror.parent else {
                return Some(current);
            };
            let Some(parent_mirror) = self.windows.iter().find(|mirror| mirror.window == parent)
            else {
                return Some(current);
            };

            if parent_mirror.parent.is_none() {
                return Some(current);
            }

            current = parent;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XMirrorEvent {
    Map {
        window: XWindowId,
    },
    Unmap {
        window: XWindowId,
    },
    Destroy {
        window: XWindowId,
    },
    Configure {
        window: XWindowId,
        geometry: Rect,
        above_sibling: Option<XWindowId>,
    },
    Reparent {
        window: XWindowId,
        parent: Option<XWindowId>,
    },
    Property {
        window: XWindowId,
        atom: u32,
        deleted: bool,
    },
    Restack {
        window: XWindowId,
        place: RestackPlace,
    },
}

impl XMirrorEvent {
    pub fn from_x11_event(event: &Event) -> Option<Self> {
        match event {
            Event::MapNotify(event) => Some(Self::Map {
                window: wrap_xid(event.window),
            }),
            Event::UnmapNotify(event) => Some(Self::Unmap {
                window: wrap_xid(event.window),
            }),
            Event::DestroyNotify(event) => Some(Self::Destroy {
                window: wrap_xid(event.window),
            }),
            Event::ConfigureNotify(event) => Some(Self::Configure {
                window: wrap_xid(event.window),
                geometry: Rect {
                    x: i32::from(event.x),
                    y: i32::from(event.y),
                    width: i32::from(event.width),
                    height: i32::from(event.height),
                },
                above_sibling: nonzero_window(event.above_sibling).map(wrap_xid),
            }),
            Event::ReparentNotify(event) => Some(Self::Reparent {
                window: wrap_xid(event.window),
                parent: nonzero_window(event.parent).map(wrap_xid),
            }),
            Event::PropertyNotify(event) => Some(Self::Property {
                window: wrap_xid(event.window),
                atom: event.atom,
                deleted: u8::from(event.state) == 1,
            }),
            Event::CirculateNotify(event) => Some(Self::Restack {
                window: wrap_xid(event.window),
                place: RestackPlace::from_x11(event.place),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestackPlace {
    OnTop,
    OnBottom,
}

impl RestackPlace {
    fn from_x11(place: Place) -> Self {
        if u8::from(place) == u8::from(Place::ON_BOTTOM) {
            Self::OnBottom
        } else {
            Self::OnTop
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XBridgeError {
    Connect {
        message: String,
    },
    InvalidScreen {
        screen_num: usize,
    },
    QueryExtension {
        extension: RequiredExtension,
        message: String,
    },
    QueryTree {
        window: u32,
        message: String,
    },
    WindowAttributes {
        window: u32,
        message: String,
    },
    InternAtom {
        atom: String,
        message: String,
    },
    GetProperty {
        window: u32,
        property: u32,
        message: String,
    },
}

impl fmt::Display for XBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { message } => write!(f, "failed to connect to X display: {message}"),
            Self::InvalidScreen { screen_num } => write!(f, "invalid X screen {screen_num}"),
            Self::QueryExtension { extension, message } => {
                write!(
                    f,
                    "failed to query {} extension: {message}",
                    extension.name()
                )
            }
            Self::QueryTree { window, message } => {
                write!(
                    f,
                    "failed to query X window tree for {window:#x}: {message}"
                )
            }
            Self::WindowAttributes { window, message } => {
                write!(
                    f,
                    "failed to query X window attributes for {window:#x}: {message}"
                )
            }
            Self::InternAtom { atom, message } => {
                write!(f, "failed to intern X atom {atom}: {message}")
            }
            Self::GetProperty {
                window,
                property,
                message,
            } => {
                write!(
                    f,
                    "failed to get X property {property:#x} from {window:#x}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for XBridgeError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequiredExtension {
    Composite,
    Damage,
    XFixes,
    Shape,
    Render,
}

impl RequiredExtension {
    pub const ALL: [Self; 5] = [
        Self::Composite,
        Self::Damage,
        Self::XFixes,
        Self::Shape,
        Self::Render,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Composite => "Composite",
            Self::Damage => "DAMAGE",
            Self::XFixes => "XFIXES",
            Self::Shape => "SHAPE",
            Self::Render => "RENDER",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStatus {
    pub extension: RequiredExtension,
    pub present: bool,
    pub major_opcode: Option<u8>,
    pub first_event: Option<u8>,
    pub first_error: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceRecord {
    pub namespace: NamespaceId,
    pub label: String,
    pub source: NamespaceSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespaceSource {
    StaticConfig,
    XServer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticNamespaceConfig {
    namespaces: Vec<NamespaceRecord>,
}

impl StaticNamespaceConfig {
    pub fn new(namespaces: Vec<NamespaceRecord>) -> Self {
        Self { namespaces }
    }

    pub fn namespaces(&self) -> &[NamespaceRecord] {
        &self.namespaces
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XConnectionProbe {
    pub display_name: Option<String>,
    pub screen_num: usize,
    pub required_extensions: Vec<ExtensionStatus>,
    pub namespaces: StaticNamespaceConfig,
}

impl XConnectionProbe {
    pub fn missing_extensions(&self) -> Vec<RequiredExtension> {
        self.required_extensions
            .iter()
            .filter(|status| !status.present)
            .map(|status| status.extension)
            .collect()
    }

    pub fn has_required_extensions(&self) -> bool {
        self.missing_extensions().is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRootImport {
    pub probe: XConnectionProbe,
    pub mirror: XMirrorState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XAtoms {
    pub wm_state: Atom,
    pub net_client_list: Atom,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XClientHints {
    pub ewmh_clients: Vec<XWindowId>,
    pub icccm_clients: Vec<XWindowId>,
}

pub fn probe_display(
    display_name: Option<&str>,
    namespaces: StaticNamespaceConfig,
) -> Result<XConnectionProbe, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let required_extensions = query_required_extensions(&connection)?;

    Ok(XConnectionProbe {
        display_name: display_name.map(str::to_owned),
        screen_num,
        required_extensions,
        namespaces,
    })
}

pub fn import_root_window_tree(
    display_name: Option<&str>,
    namespaces: StaticNamespaceConfig,
) -> Result<XRootImport, XBridgeError> {
    let (connection, screen_num) =
        x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let required_extensions = query_required_extensions(&connection)?;
    let mut mirror = import_root_window_tree_from_connection(&connection, screen_num)?;
    let atoms = intern_client_hint_atoms(&connection)?;
    let hints = detect_client_hints(&connection, screen_num, &mirror, atoms)?;
    mirror.apply_client_hints(&hints);

    Ok(XRootImport {
        probe: XConnectionProbe {
            display_name: display_name.map(str::to_owned),
            screen_num,
            required_extensions,
            namespaces,
        },
        mirror,
    })
}

fn query_required_extensions<C>(connection: &C) -> Result<Vec<ExtensionStatus>, XBridgeError>
where
    C: Connection,
{
    let mut required_extensions = Vec::with_capacity(RequiredExtension::ALL.len());

    for extension in RequiredExtension::ALL {
        let reply = connection
            .query_extension(extension.name().as_bytes())
            .map_err(|error| XBridgeError::QueryExtension {
                extension,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::QueryExtension {
                extension,
                message: error.to_string(),
            })?;

        required_extensions.push(ExtensionStatus {
            extension,
            present: reply.present,
            major_opcode: reply.present.then_some(reply.major_opcode),
            first_event: reply.present.then_some(reply.first_event),
            first_error: reply.present.then_some(reply.first_error),
        });
    }

    Ok(required_extensions)
}

fn intern_client_hint_atoms<C>(connection: &C) -> Result<XAtoms, XBridgeError>
where
    C: Connection,
{
    Ok(XAtoms {
        wm_state: intern_atom(connection, "WM_STATE")?,
        net_client_list: intern_atom(connection, "_NET_CLIENT_LIST")?,
    })
}

fn intern_atom<C>(connection: &C, name: &str) -> Result<Atom, XBridgeError>
where
    C: Connection,
{
    connection
        .intern_atom(false, name.as_bytes())
        .map_err(|error| XBridgeError::InternAtom {
            atom: name.to_owned(),
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| XBridgeError::InternAtom {
            atom: name.to_owned(),
            message: error.to_string(),
        })
}

fn detect_client_hints<C>(
    connection: &C,
    screen_num: usize,
    mirror: &XMirrorState,
    atoms: XAtoms,
) -> Result<XClientHints, XBridgeError>
where
    C: Connection,
{
    let root = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?
        .root;
    let ewmh_clients = read_window_list_property(connection, root, atoms.net_client_list)?
        .into_iter()
        .map(wrap_xid)
        .collect();
    let mut icccm_clients = Vec::new();

    for mirror in mirror.windows() {
        if has_property(connection, mirror.window.xid(), atoms.wm_state)? {
            icccm_clients.push(mirror.window);
        }
    }

    Ok(XClientHints {
        ewmh_clients,
        icccm_clients,
    })
}

fn read_window_list_property<C>(
    connection: &C,
    window: Window,
    property: Atom,
) -> Result<Vec<Window>, XBridgeError>
where
    C: Connection,
{
    let reply = connection
        .get_property(false, window, property, AtomEnum::WINDOW, 0, u32::MAX / 4)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?;

    Ok(reply
        .value32()
        .map(|values| values.collect::<Vec<_>>())
        .unwrap_or_default())
}

fn has_property<C>(connection: &C, window: Window, property: Atom) -> Result<bool, XBridgeError>
where
    C: Connection,
{
    connection
        .get_property(false, window, property, AtomEnum::ANY, 0, 0)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })?
        .reply()
        .map(|reply| reply.type_ != 0)
        .map_err(|error| XBridgeError::GetProperty {
            window,
            property,
            message: error.to_string(),
        })
}

fn import_root_window_tree_from_connection<C>(
    connection: &C,
    screen_num: usize,
) -> Result<XMirrorState, XBridgeError>
where
    C: Connection,
{
    let root = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?
        .root;
    let mut queue = VecDeque::from([(root, None, 0)]);
    let mut visited = BTreeSet::new();
    let mut mirror = XMirrorState::default();

    while let Some((window, parent, stack_rank)) = queue.pop_front() {
        if !visited.insert(window) {
            continue;
        }

        let tree = connection
            .query_tree(window)
            .map_err(|error| XBridgeError::QueryTree {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::QueryTree {
                window,
                message: error.to_string(),
            })?;
        let attributes = connection
            .get_window_attributes(window)
            .map_err(|error| XBridgeError::WindowAttributes {
                window,
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::WindowAttributes {
                window,
                message: error.to_string(),
            })?;

        for (rank, child) in tree.children.iter().copied().enumerate() {
            let rank = u32::try_from(rank).expect("X child stack rank overflow");
            queue.push_back((child, Some(window), rank));
        }

        mirror.ingest_window(XWindowMirror {
            window: wrap_xid(window),
            parent: parent.map(wrap_xid),
            children: tree.children.iter().copied().map(wrap_xid).collect(),
            toplevel: None,
            client: None,
            mapped: u8::from(attributes.map_state) == u8::from(MapState::VIEWABLE),
            stack_rank,
            namespace: None,
            stale_metadata: 0,
        });
    }

    Ok(mirror)
}

fn wrap_xid(window: Window) -> XWindowId {
    XWindowId::new(window, 1)
}

fn nonzero_window(window: Window) -> Option<Window> {
    (window != 0).then_some(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(extension: RequiredExtension, present: bool) -> ExtensionStatus {
        ExtensionStatus {
            extension,
            present,
            major_opcode: present.then_some(128),
            first_event: present.then_some(64),
            first_error: present.then_some(32),
        }
    }

    #[test]
    fn probe_reports_missing_required_extensions() {
        let probe = XConnectionProbe {
            display_name: Some(":99".to_owned()),
            screen_num: 0,
            required_extensions: vec![
                status(RequiredExtension::Composite, true),
                status(RequiredExtension::Damage, false),
            ],
            namespaces: StaticNamespaceConfig::default(),
        };

        assert_eq!(probe.missing_extensions(), vec![RequiredExtension::Damage]);
        assert!(!probe.has_required_extensions());
    }

    #[test]
    fn static_namespace_config_records_known_namespaces() {
        let config = StaticNamespaceConfig::new(vec![NamespaceRecord {
            namespace: NamespaceId::from_raw(1),
            label: "trusted".to_owned(),
            source: NamespaceSource::StaticConfig,
        }]);

        assert_eq!(config.namespaces().len(), 1);
        assert_eq!(config.namespaces()[0].label, "trusted");
        assert_eq!(config.namespaces()[0].source, NamespaceSource::StaticConfig);
    }

    #[test]
    fn wraps_imported_xids_with_initial_generation() {
        assert_eq!(wrap_xid(0x1200042), XWindowId::new(0x1200042, 1));
    }

    fn mirror(window: u32, parent: Option<u32>, stack_rank: u32) -> XWindowMirror {
        XWindowMirror {
            window: wrap_xid(window),
            parent: parent.map(wrap_xid),
            children: Vec::new(),
            toplevel: None,
            client: None,
            mapped: false,
            stack_rank,
            namespace: None,
            stale_metadata: 0,
        }
    }

    #[test]
    fn mirror_events_update_map_state() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x10, None, 0));

        state.apply_event(XMirrorEvent::Map {
            window: wrap_xid(0x10),
        });
        assert!(state.windows()[0].mapped);

        state.apply_event(XMirrorEvent::Unmap {
            window: wrap_xid(0x10),
        });
        assert!(!state.windows()[0].mapped);
    }

    #[test]
    fn mirror_events_remove_destroyed_windows_from_parent_children() {
        let mut state = XMirrorState::default();
        let mut parent = mirror(0x10, None, 0);
        parent.children.push(wrap_xid(0x20));
        state.ingest_window(parent);
        state.ingest_window(mirror(0x20, Some(0x10), 0));

        state.apply_event(XMirrorEvent::Destroy {
            window: wrap_xid(0x20),
        });

        assert_eq!(state.windows().len(), 1);
        assert!(state.windows()[0].children.is_empty());
    }

    #[test]
    fn mirror_events_reparent_windows() {
        let mut state = XMirrorState::default();
        let mut old_parent = mirror(0x10, None, 0);
        old_parent.children.push(wrap_xid(0x30));
        state.ingest_window(old_parent);
        state.ingest_window(mirror(0x20, None, 1));
        state.ingest_window(mirror(0x30, Some(0x10), 0));

        state.apply_event(XMirrorEvent::Reparent {
            window: wrap_xid(0x30),
            parent: Some(wrap_xid(0x20)),
        });

        let old_parent = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x10))
            .unwrap();
        let new_parent = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();
        let child = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x30))
            .unwrap();

        assert!(old_parent.children.is_empty());
        assert_eq!(new_parent.children, vec![wrap_xid(0x30)]);
        assert_eq!(child.parent, Some(wrap_xid(0x20)));
        assert_eq!(child.stale_metadata, 1);
    }

    #[test]
    fn mirror_events_track_restack_and_property_staleness() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x10, None, 3));
        state.ingest_window(mirror(0x20, None, 5));

        state.apply_event(XMirrorEvent::Configure {
            window: wrap_xid(0x10),
            geometry: Rect {
                x: 1,
                y: 2,
                width: 300,
                height: 200,
            },
            above_sibling: Some(wrap_xid(0x20)),
        });
        state.apply_event(XMirrorEvent::Property {
            window: wrap_xid(0x10),
            atom: 42,
            deleted: false,
        });

        let window = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x10))
            .unwrap();

        assert_eq!(window.stack_rank, 6);
        assert_eq!(window.stale_metadata, 2);
    }

    #[test]
    fn client_hints_mark_root_child_as_toplevel() {
        let mut state = XMirrorState::default();
        state.ingest_window(mirror(0x01, None, 0));
        state.ingest_window(mirror(0x20, Some(0x01), 0));

        state.apply_client_hints(&XClientHints {
            ewmh_clients: vec![wrap_xid(0x20)],
            icccm_clients: Vec::new(),
        });

        let client = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();

        assert_eq!(client.client, Some(wrap_xid(0x20)));
        assert_eq!(client.toplevel, Some(wrap_xid(0x20)));
    }

    #[test]
    fn client_hints_promote_reparented_frame_as_toplevel() {
        let mut state = XMirrorState::default();
        let mut root = mirror(0x01, None, 0);
        root.children.push(wrap_xid(0x20));
        let mut frame = mirror(0x20, Some(0x01), 0);
        frame.children.push(wrap_xid(0x30));
        state.ingest_window(root);
        state.ingest_window(frame);
        state.ingest_window(mirror(0x30, Some(0x20), 0));

        state.apply_client_hints(&XClientHints {
            ewmh_clients: Vec::new(),
            icccm_clients: vec![wrap_xid(0x30)],
        });

        let frame = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x20))
            .unwrap();
        let client = state
            .windows()
            .iter()
            .find(|mirror| mirror.window == wrap_xid(0x30))
            .unwrap();

        assert_eq!(frame.client, Some(wrap_xid(0x30)));
        assert_eq!(frame.toplevel, Some(wrap_xid(0x20)));
        assert_eq!(client.client, Some(wrap_xid(0x30)));
        assert_eq!(client.toplevel, Some(wrap_xid(0x20)));
    }
}

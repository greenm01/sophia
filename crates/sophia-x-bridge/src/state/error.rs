use crate::prelude::*;

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
    WindowGeometry {
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
    PoliteClose {
        window: u32,
        message: String,
    },
    CompositeVersion {
        message: String,
    },
    CompositeRedirect {
        window: u32,
        message: String,
    },
    CompositeNamePixmap {
        window: u32,
        pixmap: u32,
        message: String,
    },
    GenerateId {
        message: String,
    },
    DamageVersion {
        message: String,
    },
    DamageCreate {
        window: u32,
        damage: u32,
        message: String,
    },
    PixmapGeometry {
        pixmap: u32,
        message: String,
    },
    PixmapReadback {
        pixmap: u32,
        message: String,
    },
    TestClient {
        message: String,
    },
    RoutedInput {
        message: String,
    },
    SelectionMonitor {
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
            Self::WindowGeometry { window, message } => {
                write!(
                    f,
                    "failed to query X window geometry for {window:#x}: {message}"
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
            Self::PoliteClose { window, message } => {
                write!(
                    f,
                    "failed to request polite close for {window:#x}: {message}"
                )
            }
            Self::CompositeVersion { message } => {
                write!(f, "failed to negotiate XComposite version: {message}")
            }
            Self::CompositeRedirect { window, message } => {
                write!(
                    f,
                    "failed to redirect X window {window:#x} with XComposite: {message}"
                )
            }
            Self::CompositeNamePixmap {
                window,
                pixmap,
                message,
            } => {
                write!(
                    f,
                    "failed to name XComposite pixmap {pixmap:#x} for X window {window:#x}: {message}"
                )
            }
            Self::GenerateId { message } => {
                write!(f, "failed to allocate an X resource ID: {message}")
            }
            Self::DamageVersion { message } => {
                write!(f, "failed to negotiate X Damage version: {message}")
            }
            Self::DamageCreate {
                window,
                damage,
                message,
            } => {
                write!(
                    f,
                    "failed to create X Damage object {damage:#x} for X window {window:#x}: {message}"
                )
            }
            Self::PixmapGeometry { pixmap, message } => {
                write!(
                    f,
                    "failed to query X pixmap geometry for {pixmap:#x}: {message}"
                )
            }
            Self::PixmapReadback { pixmap, message } => {
                write!(f, "failed to read X pixmap {pixmap:#x}: {message}")
            }
            Self::TestClient { message } => {
                write!(f, "failed to run Sophia X test client: {message}")
            }
            Self::RoutedInput { message } => {
                write!(f, "failed to run Sophia routed-input smoke: {message}")
            }
            Self::SelectionMonitor { message } => {
                write!(f, "failed to monitor X selections: {message}")
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

use crate::{NamespaceId, NamespacePortalCapability, PortalTransferId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortalTransfer {
    pub transfer: PortalTransferId,
    pub source_namespace: NamespaceId,
    pub target_namespace: NamespaceId,
    pub kind: PortalTransferKind,
    pub mime_type: Option<String>,
    pub byte_size: u64,
    pub decision: PortalDecision,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortalTransferKind {
    Clipboard,
    DragAndDrop,
    FileHandoff,
    ScreenCapture,
    ScreenRecording,
    UriOpen,
    Notification,
}

impl PortalTransferKind {
    pub const fn capability(self) -> NamespacePortalCapability {
        match self {
            Self::Clipboard => NamespacePortalCapability::Clipboard,
            Self::DragAndDrop => NamespacePortalCapability::DragAndDrop,
            Self::FileHandoff => NamespacePortalCapability::FileHandoff,
            Self::ScreenCapture => NamespacePortalCapability::ScreenCapture,
            Self::ScreenRecording => NamespacePortalCapability::ScreenRecording,
            Self::UriOpen => NamespacePortalCapability::UriOpen,
            Self::Notification => NamespacePortalCapability::Notification,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortalDecision {
    Pending,
    Allowed,
    Denied,
    Revoked,
}

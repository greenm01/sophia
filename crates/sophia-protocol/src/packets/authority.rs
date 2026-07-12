use crate::ids::XWindowId;
use crate::{BufferSource, SurfaceId, TransactionCommit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityKind {
    SophiaX,
    SophiaWayland,
    SophiaNative,
    XLibrePrototype,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorityFeedback {
    Transaction(TransactionCommit),
    Presented(SurfacePresentationFeedback),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfacePresentationFeedback {
    pub surface: SurfaceId,
    pub generation: u64,
    pub presentation_msec: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferReleaseFeedback {
    pub surface: SurfaceId,
    pub source: BufferSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuBufferFormat {
    Argb8888,
    Xrgb8888,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuBufferRegistration {
    pub handle: u64,
    pub size: crate::Size,
    pub stride: u32,
    pub format: CpuBufferFormat,
    pub generation: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthorityLocalId {
    raw: u64,
    generation: u32,
}

impl AuthorityLocalId {
    pub const NONE: Self = Self {
        raw: 0,
        generation: 0,
    };

    pub const fn new(raw: u64, generation: u32) -> Self {
        Self { raw, generation }
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn is_valid(self) -> bool {
        self.raw != 0 && self.generation != 0
    }
}

impl From<XWindowId> for AuthorityLocalId {
    fn from(window: XWindowId) -> Self {
        Self::new(u64::from(window.xid()), window.generation())
    }
}

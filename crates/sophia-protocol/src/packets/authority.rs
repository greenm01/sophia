use crate::ids::XWindowId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityKind {
    SophiaX,
    SophiaWayland,
    SophiaNative,
    XLibrePrototype,
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

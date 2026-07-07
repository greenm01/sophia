use core::marker::PhantomData;

macro_rules! simple_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            pub const INVALID: Self = Self(0);

            pub const fn from_raw(raw: u64) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u64 {
                self.0
            }

            pub const fn is_valid(self) -> bool {
                self.0 != 0
            }
        }
    };
}

simple_id!(NamespaceId);
simple_id!(OutputId);
simple_id!(SeatId);
simple_id!(DeviceId);
simple_id!(TransactionId);
simple_id!(PortalTransferId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceId {
    index: u32,
    generation: u32,
}

impl SurfaceId {
    pub const INVALID: Self = Self {
        index: u32::MAX,
        generation: 0,
    };

    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn is_valid(self) -> bool {
        self.index != u32::MAX && self.generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct XWindowId {
    xid: u32,
    generation: u32,
}

impl XWindowId {
    pub const NONE: Self = Self {
        xid: 0,
        generation: 0,
    };

    pub const fn new(xid: u32, generation: u32) -> Self {
        Self { xid, generation }
    }

    pub const fn xid(self) -> u32 {
        self.xid
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn is_valid(self) -> bool {
        self.xid != 0 && self.generation != 0
    }
}

#[derive(Debug)]
pub struct IdAllocator<T> {
    next: u64,
    _kind: PhantomData<fn() -> T>,
}

impl<T> IdAllocator<T> {
    pub const fn new() -> Self {
        Self {
            next: 1,
            _kind: PhantomData,
        }
    }

    pub fn next_raw(&mut self) -> u64 {
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .expect("Sophia ID counter overflow");
        id
    }
}

macro_rules! allocator_next {
    ($name:ident) => {
        impl IdAllocator<$name> {
            pub fn next_id(&mut self) -> $name {
                $name::from_raw(self.next_raw())
            }
        }
    };
}

allocator_next!(NamespaceId);
allocator_next!(OutputId);
allocator_next!(SeatId);
allocator_next!(DeviceId);
allocator_next!(TransactionId);
allocator_next!(PortalTransferId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_ids_start_after_zero() {
        let mut alloc = IdAllocator::<NamespaceId>::new();
        let first = alloc.next_id();
        let second = alloc.next_id();

        assert!(first.is_valid());
        assert_eq!(first.raw(), 1);
        assert_eq!(second.raw(), 2);
    }

    #[test]
    fn foreign_xids_keep_generation() {
        let id = XWindowId::new(0x1200042, 7);

        assert!(id.is_valid());
        assert_eq!(id.xid(), 0x1200042);
        assert_eq!(id.generation(), 7);
    }
}

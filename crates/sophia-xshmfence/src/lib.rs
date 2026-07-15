//! Narrow safe adapter for querying classic DRI3 xshmfence file descriptors.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XShmFenceQueryError {
    LibraryUnavailable,
    SymbolUnavailable,
    MapFailed,
    UnmapFailed,
    AllocationFailed,
}

impl core::fmt::Display for XShmFenceQueryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for XShmFenceQueryError {}

pub fn query(fd: &OwnedFd) -> Result<bool, XShmFenceQueryError> {
    type Map = unsafe extern "C" fn(i32) -> *mut core::ffi::c_void;
    type Query = unsafe extern "C" fn(*mut core::ffi::c_void) -> i32;
    type Unmap = unsafe extern "C" fn(*mut core::ffi::c_void) -> i32;

    // SAFETY: the library name is fixed and no constructors receive caller data.
    let library = unsafe { libloading::Library::new("libxshmfence.so.1") }
        .map_err(|_| XShmFenceQueryError::LibraryUnavailable)?;
    // SAFETY: these signatures are the stable libxshmfence C ABI.
    let map = unsafe { library.get::<Map>(b"xshmfence_map_shm\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: these signatures are the stable libxshmfence C ABI.
    let query = unsafe { library.get::<Query>(b"xshmfence_query\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: these signatures are the stable libxshmfence C ABI.
    let unmap = unsafe { library.get::<Unmap>(b"xshmfence_unmap_shm\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: fd remains owned and open through unmap; the library validates it.
    let fence = unsafe { map(fd.as_raw_fd()) };
    if fence.is_null() {
        return Err(XShmFenceQueryError::MapFailed);
    }
    // SAFETY: fence is the live mapping returned by xshmfence_map_shm.
    let signaled = unsafe { query(fence) } != 0;
    // SAFETY: fence is unmapped exactly once before the library is dropped.
    if unsafe { unmap(fence) } != 0 {
        return Err(XShmFenceQueryError::UnmapFailed);
    }
    Ok(signaled)
}

pub fn allocate() -> Result<OwnedFd, XShmFenceQueryError> {
    type Alloc = unsafe extern "C" fn() -> i32;
    // SAFETY: the library name is fixed and no constructors receive caller data.
    let library = unsafe { libloading::Library::new("libxshmfence.so.1") }
        .map_err(|_| XShmFenceQueryError::LibraryUnavailable)?;
    // SAFETY: this signature is the stable libxshmfence C ABI.
    let alloc = unsafe { library.get::<Alloc>(b"xshmfence_alloc_shm\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: xshmfence_alloc_shm has no arguments and returns a new owned FD.
    let raw = unsafe { alloc() };
    if raw < 0 {
        return Err(XShmFenceQueryError::AllocationFailed);
    }
    // SAFETY: successful allocation returned a unique owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}

pub fn trigger(fd: &OwnedFd) -> Result<(), XShmFenceQueryError> {
    type Map = unsafe extern "C" fn(i32) -> *mut core::ffi::c_void;
    type Trigger = unsafe extern "C" fn(*mut core::ffi::c_void);
    type Unmap = unsafe extern "C" fn(*mut core::ffi::c_void) -> i32;
    // SAFETY: the library name is fixed and no constructors receive caller data.
    let library = unsafe { libloading::Library::new("libxshmfence.so.1") }
        .map_err(|_| XShmFenceQueryError::LibraryUnavailable)?;
    // SAFETY: these signatures are the stable libxshmfence C ABI.
    let map = unsafe { library.get::<Map>(b"xshmfence_map_shm\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: these signatures are the stable libxshmfence C ABI.
    let trigger = unsafe { library.get::<Trigger>(b"xshmfence_trigger\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: these signatures are the stable libxshmfence C ABI.
    let unmap = unsafe { library.get::<Unmap>(b"xshmfence_unmap_shm\0") }
        .map_err(|_| XShmFenceQueryError::SymbolUnavailable)?;
    // SAFETY: fd remains open through unmap; the library validates it.
    let fence = unsafe { map(fd.as_raw_fd()) };
    if fence.is_null() {
        return Err(XShmFenceQueryError::MapFailed);
    }
    // SAFETY: fence is the live mapping returned by xshmfence_map_shm.
    unsafe { trigger(fence) };
    // SAFETY: fence is unmapped exactly once.
    if unsafe { unmap(fence) } != 0 {
        return Err(XShmFenceQueryError::UnmapFailed);
    }
    Ok(())
}

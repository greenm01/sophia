//! Narrow safe adapter for copying bytes from an existing SysV SHM segment.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadError {
    InvalidId,
    MissingSegment,
    RangeOverflow,
    OutOfBounds,
    AttachFailed,
    DetachFailed,
}

impl core::fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ReadError {}

pub fn copy_bytes(shmid: u32, offset: usize, len: usize) -> Result<Vec<u8>, ReadError> {
    let shmid = libc::c_int::try_from(shmid).map_err(|_| ReadError::InvalidId)?;
    let end = offset.checked_add(len).ok_or(ReadError::RangeOverflow)?;
    let mut metadata = core::mem::MaybeUninit::<libc::shmid_ds>::zeroed();
    // SAFETY: metadata points to writable storage for `shmctl(IPC_STAT)`.
    if unsafe { libc::shmctl(shmid, libc::IPC_STAT, metadata.as_mut_ptr()) } != 0 {
        return Err(ReadError::MissingSegment);
    }
    // SAFETY: successful IPC_STAT initialized the complete `shmid_ds` value.
    let metadata = unsafe { metadata.assume_init() };
    if end > metadata.shm_segsz {
        return Err(ReadError::OutOfBounds);
    }

    // SAFETY: a null address lets the kernel choose the mapping; SHM_RDONLY
    // prevents this adapter from mutating client-owned memory.
    let address = unsafe { libc::shmat(shmid, core::ptr::null(), libc::SHM_RDONLY) };
    if address == (-1_isize) as *mut libc::c_void {
        return Err(ReadError::AttachFailed);
    }
    // SAFETY: IPC_STAT established that offset..end lies inside the mapped
    // segment, and the mapping remains attached for the duration of the copy.
    let bytes =
        unsafe { core::slice::from_raw_parts((address as *const u8).add(offset), len).to_vec() };
    // SAFETY: address is exactly the live mapping returned by shmat above.
    if unsafe { libc::shmdt(address) } != 0 {
        return Err(ReadError::DetachFailed);
    }
    Ok(bytes)
}

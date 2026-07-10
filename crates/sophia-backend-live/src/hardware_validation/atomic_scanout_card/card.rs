use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutCard(std::fs::File);

impl RealAtomicScanoutCard {
    pub(super) fn open_nonblocking(path: &Path) -> io::Result<Self> {
        Ok(Self(
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
                .open(path)?,
        ))
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }

    pub fn try_clone_file(&self) -> io::Result<std::fs::File> {
        self.0.try_clone()
    }
}

impl AsFd for RealAtomicScanoutCard {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for RealAtomicScanoutCard {}
impl drm::control::Device for RealAtomicScanoutCard {}

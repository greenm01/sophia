use crate::Size;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetRecord {
    pub status: LiveGbmEglFrameTargetStatus,
    pub size: Size,
}

impl LiveGbmEglFrameTargetRecord {
    pub const fn new(size: Size) -> Self {
        Self {
            status: if size.width > 0 && size.height > 0 {
                LiveGbmEglFrameTargetStatus::Ready
            } else {
                LiveGbmEglFrameTargetStatus::InvalidSize
            },
            size,
        }
    }

    pub const fn is_valid_scanout_target(self) -> bool {
        matches!(self.status, LiveGbmEglFrameTargetStatus::Ready)
            && self.size.width > 0
            && self.size.height > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGbmEglFrameTargetStatus {
    Ready,
    InvalidSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetLifecycleReport {
    pub status: LiveGbmEglFrameTargetLifecycleStatus,
    pub target: LiveGbmEglFrameTargetRecord,
}

impl LiveGbmEglFrameTargetLifecycleReport {
    pub const fn created(target: LiveGbmEglFrameTargetRecord) -> Self {
        Self {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target,
        }
    }

    pub const fn from_size_update(
        previous: Option<LiveGbmEglFrameTargetRecord>,
        target: LiveGbmEglFrameTargetRecord,
    ) -> Self {
        let status = match (previous, target.status) {
            (_, LiveGbmEglFrameTargetStatus::InvalidSize) => {
                LiveGbmEglFrameTargetLifecycleStatus::Invalidated
            }
            (None, LiveGbmEglFrameTargetStatus::Ready) => {
                LiveGbmEglFrameTargetLifecycleStatus::Created
            }
            (Some(previous), LiveGbmEglFrameTargetStatus::Ready)
                if previous.status as u8 == target.status as u8
                    && previous.size.width == target.size.width
                    && previous.size.height == target.size.height =>
            {
                LiveGbmEglFrameTargetLifecycleStatus::Retained
            }
            (Some(_), LiveGbmEglFrameTargetStatus::Ready) => {
                LiveGbmEglFrameTargetLifecycleStatus::Resized
            }
        };

        Self { status, target }
    }

    pub const fn retired(target: LiveGbmEglFrameTargetRecord) -> Self {
        Self {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retired,
            target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGbmEglFrameTargetLifecycleStatus {
    Created,
    Retained,
    Resized,
    Invalidated,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetAllocationRequest {
    pub target: LiveGbmEglFrameTargetRecord,
}

impl LiveGbmEglFrameTargetAllocationRequest {
    pub const fn new(size: Size) -> Self {
        Self {
            target: LiveGbmEglFrameTargetRecord::new(size),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGbmEglFrameTargetAllocationReport {
    pub status: LiveGbmEglFrameTargetAllocationStatus,
    pub target: LiveGbmEglFrameTargetRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveGbmEglFrameTargetAllocationStatus {
    Ready,
    InvalidTarget,
    Unavailable,
    Degraded,
}

pub trait LiveGbmEglFrameTargetAllocator {
    fn allocate_frame_target(
        &mut self,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeGbmEglFrameTargetAllocator {
    pub status: LiveGbmEglFrameTargetAllocationStatus,
}

impl FakeGbmEglFrameTargetAllocator {
    pub const fn new(status: LiveGbmEglFrameTargetAllocationStatus) -> Self {
        Self { status }
    }
}

impl LiveGbmEglFrameTargetAllocator for FakeGbmEglFrameTargetAllocator {
    fn allocate_frame_target(
        &mut self,
        request: LiveGbmEglFrameTargetAllocationRequest,
    ) -> LiveGbmEglFrameTargetAllocationReport {
        let status = if request.target.is_valid_scanout_target() {
            self.status
        } else {
            LiveGbmEglFrameTargetAllocationStatus::InvalidTarget
        };

        LiveGbmEglFrameTargetAllocationReport {
            status,
            target: request.target,
        }
    }
}

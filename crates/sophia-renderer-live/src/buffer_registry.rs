use std::collections::BTreeMap;
use std::os::fd::OwnedFd;

use sophia_protocol::{
    BufferHandle, BufferSource, DmaBufDescriptor, DmaBufDescriptorError, SurfaceId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBufferState {
    WaitingForAcquireFence,
    Ready,
    Submitted,
}

#[derive(Debug)]
pub struct LiveDmaBufRegistration {
    pub descriptor: DmaBufDescriptor,
    pub plane_fds: Vec<OwnedFd>,
    pub acquire_fence: Option<OwnedFd>,
    pub state: LiveBufferState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveBufferRegistryError {
    InvalidDescriptor(DmaBufDescriptorError),
    PlaneFdCountMismatch,
    DuplicateHandle,
    UnknownHandle,
    AcquireFencePending,
    AlreadySubmitted,
}

#[derive(Debug, Default)]
pub struct LiveBufferRegistry {
    buffers: BTreeMap<BufferHandle, LiveDmaBufRegistration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingCpuPresentation {
    handle: u64,
    previous_committed_handle: Option<u64>,
}

#[derive(Debug, Default)]
pub struct LiveCpuBufferLifetimeRegistry {
    committed: BTreeMap<SurfaceId, u64>,
    pending: BTreeMap<SurfaceId, PendingCpuPresentation>,
}

impl LiveCpuBufferLifetimeRegistry {
    pub fn submit(&mut self, surface: SurfaceId, handle: u64) -> Option<BufferSource> {
        let replaced_pending = self.pending.insert(
            surface,
            PendingCpuPresentation {
                handle,
                previous_committed_handle: self.committed.get(&surface).copied(),
            },
        );
        replaced_pending.map(|pending| BufferSource::CpuBuffer {
            handle: pending.handle,
        })
    }

    pub fn retire_page_flip(&mut self, surface: SurfaceId, handle: u64) -> Vec<BufferSource> {
        let Some(pending) = self.pending.get(&surface).copied() else {
            return Vec::new();
        };
        if pending.handle != handle {
            return Vec::new();
        }
        self.pending.remove(&surface);
        self.committed.insert(surface, handle);
        pending
            .previous_committed_handle
            .filter(|previous| *previous != handle)
            .map(|previous| vec![BufferSource::CpuBuffer { handle: previous }])
            .unwrap_or_default()
    }

    pub fn reject(&mut self, surface: SurfaceId, handle: u64) -> Option<BufferSource> {
        let matches = self
            .pending
            .get(&surface)
            .is_some_and(|pending| pending.handle == handle);
        matches.then(|| {
            self.pending.remove(&surface);
            BufferSource::CpuBuffer { handle }
        })
    }

    pub fn committed_handle(&self, surface: SurfaceId) -> Option<u64> {
        self.committed.get(&surface).copied()
    }

    pub fn disconnect(&mut self) -> Vec<BufferSource> {
        let mut handles = self
            .pending
            .values()
            .map(|pending| pending.handle)
            .chain(self.committed.values().copied())
            .collect::<Vec<_>>();
        handles.sort_unstable();
        handles.dedup();
        self.pending.clear();
        self.committed.clear();
        handles
            .into_iter()
            .map(|handle| BufferSource::CpuBuffer { handle })
            .collect()
    }
}

impl LiveBufferRegistry {
    pub fn register(
        &mut self,
        descriptor: DmaBufDescriptor,
        plane_fds: Vec<OwnedFd>,
        acquire_fence: Option<OwnedFd>,
    ) -> Result<(), LiveBufferRegistryError> {
        descriptor
            .validate()
            .map_err(LiveBufferRegistryError::InvalidDescriptor)?;
        if plane_fds.len() != usize::from(descriptor.plane_count) {
            return Err(LiveBufferRegistryError::PlaneFdCountMismatch);
        }
        if self.buffers.contains_key(&descriptor.handle) {
            return Err(LiveBufferRegistryError::DuplicateHandle);
        }
        let state = if acquire_fence.is_some() {
            LiveBufferState::WaitingForAcquireFence
        } else {
            LiveBufferState::Ready
        };
        self.buffers.insert(
            descriptor.handle,
            LiveDmaBufRegistration {
                descriptor,
                plane_fds,
                acquire_fence,
                state,
            },
        );
        Ok(())
    }

    pub fn signal_acquire_fence(
        &mut self,
        handle: BufferHandle,
    ) -> Result<(), LiveBufferRegistryError> {
        let buffer = self
            .buffers
            .get_mut(&handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?;
        buffer.acquire_fence.take();
        if buffer.state == LiveBufferState::WaitingForAcquireFence {
            buffer.state = LiveBufferState::Ready;
        }
        Ok(())
    }

    pub fn submit(&mut self, handle: BufferHandle) -> Result<(), LiveBufferRegistryError> {
        let buffer = self
            .buffers
            .get_mut(&handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?;
        match buffer.state {
            LiveBufferState::WaitingForAcquireFence => {
                Err(LiveBufferRegistryError::AcquireFencePending)
            }
            LiveBufferState::Ready => {
                buffer.state = LiveBufferState::Submitted;
                Ok(())
            }
            LiveBufferState::Submitted => Err(LiveBufferRegistryError::AlreadySubmitted),
        }
    }

    pub fn retire_page_flip(&mut self, handle: BufferHandle) -> Option<BufferSource> {
        let submitted = self
            .buffers
            .get(&handle)
            .is_some_and(|buffer| buffer.state == LiveBufferState::Submitted);
        submitted.then(|| {
            self.buffers.remove(&handle);
            BufferSource::DmaBuf {
                handle: handle.raw(),
            }
        })
    }

    pub fn reject(&mut self, handle: BufferHandle) -> Option<BufferSource> {
        self.buffers.remove(&handle).map(|_| BufferSource::DmaBuf {
            handle: handle.raw(),
        })
    }

    pub fn disconnect(&mut self) -> Vec<BufferSource> {
        let handles = self.buffers.keys().copied().collect::<Vec<_>>();
        handles
            .into_iter()
            .filter_map(|handle| self.reject(handle))
            .collect()
    }

    pub fn state(&self, handle: BufferHandle) -> Option<LiveBufferState> {
        self.buffers.get(&handle).map(|buffer| buffer.state)
    }
}

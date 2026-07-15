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
    FenceQueryFailed,
    FdCloneFailed,
    DuplicateFence,
    UnknownFence,
    SourceInUse,
}

#[derive(Debug, Default)]
pub struct LiveBufferRegistry {
    buffers: BTreeMap<BufferHandle, LiveDmaBufRegistration>,
}

#[derive(Debug)]
struct LiveDmaBufSourceRegistration {
    descriptor: DmaBufDescriptor,
    plane_fds: Vec<OwnedFd>,
}

#[derive(Debug)]
struct LiveFenceSourceRegistration {
    initially_triggered: bool,
    fd: OwnedFd,
}

/// Renderer-private reusable DMA-BUF sources plus their in-flight Present
/// lifetimes.
///
/// DRI3 pixmaps outlive any single Present request. Source plane descriptors
/// therefore remain registered while each presentation receives its own
/// duplicated plane and acquire-fence ownership in [`LiveBufferRegistry`]. A
/// page-flip retirement drops only that presentation ownership, allowing the
/// same pixmap to be presented again without exposing native FDs outside the
/// renderer boundary.
#[derive(Debug, Default)]
pub struct LiveDmaBufPresentationRegistry {
    sources: BTreeMap<BufferHandle, LiveDmaBufSourceRegistration>,
    fences: BTreeMap<u64, LiveFenceSourceRegistration>,
    presentations: LiveBufferRegistry,
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

    pub fn poll_acquire_fence(
        &mut self,
        handle: BufferHandle,
    ) -> Result<bool, LiveBufferRegistryError> {
        let buffer = self
            .buffers
            .get(&handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?;
        if buffer.state != LiveBufferState::WaitingForAcquireFence {
            return Ok(true);
        }
        let signaled = sophia_xshmfence::query(
            buffer
                .acquire_fence
                .as_ref()
                .ok_or(LiveBufferRegistryError::FenceQueryFailed)?,
        )
        .map_err(|_| LiveBufferRegistryError::FenceQueryFailed)?;
        if signaled {
            self.signal_acquire_fence(handle)?;
        }
        Ok(signaled)
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

    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

impl LiveDmaBufPresentationRegistry {
    pub fn register_source(
        &mut self,
        descriptor: DmaBufDescriptor,
        plane_fds: Vec<OwnedFd>,
    ) -> Result<(), LiveBufferRegistryError> {
        descriptor
            .validate()
            .map_err(LiveBufferRegistryError::InvalidDescriptor)?;
        if plane_fds.len() != usize::from(descriptor.plane_count) {
            return Err(LiveBufferRegistryError::PlaneFdCountMismatch);
        }
        if self.sources.contains_key(&descriptor.handle) {
            return Err(LiveBufferRegistryError::DuplicateHandle);
        }
        self.sources.insert(
            descriptor.handle,
            LiveDmaBufSourceRegistration {
                descriptor,
                plane_fds,
            },
        );
        Ok(())
    }

    pub fn register_fence(
        &mut self,
        fence: u64,
        initially_triggered: bool,
        fd: OwnedFd,
    ) -> Result<(), LiveBufferRegistryError> {
        if self.fences.contains_key(&fence) {
            return Err(LiveBufferRegistryError::DuplicateFence);
        }
        self.fences.insert(
            fence,
            LiveFenceSourceRegistration {
                initially_triggered,
                fd,
            },
        );
        Ok(())
    }

    pub fn begin_present(
        &mut self,
        handle: BufferHandle,
        acquire_fence: Option<u64>,
    ) -> Result<(), LiveBufferRegistryError> {
        let source = self
            .sources
            .get(&handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?;
        let plane_fds = source
            .plane_fds
            .iter()
            .map(OwnedFd::try_clone)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| LiveBufferRegistryError::FdCloneFailed)?;
        let acquire_fence = acquire_fence
            .map(|fence| {
                let fence = self
                    .fences
                    .get(&fence)
                    .ok_or(LiveBufferRegistryError::UnknownFence)?;
                if fence.initially_triggered {
                    Ok(None)
                } else {
                    fence
                        .fd
                        .try_clone()
                        .map(Some)
                        .map_err(|_| LiveBufferRegistryError::FdCloneFailed)
                }
            })
            .transpose()?
            .flatten();
        self.presentations
            .register(source.descriptor, plane_fds, acquire_fence)
    }

    pub fn poll_acquire_fence(
        &mut self,
        handle: BufferHandle,
    ) -> Result<bool, LiveBufferRegistryError> {
        self.presentations.poll_acquire_fence(handle)
    }

    pub fn submit(&mut self, handle: BufferHandle) -> Result<(), LiveBufferRegistryError> {
        self.presentations.submit(handle)
    }

    pub fn retire_page_flip(&mut self, handle: BufferHandle) -> Option<BufferSource> {
        self.presentations.retire_page_flip(handle)
    }

    pub fn reject(&mut self, handle: BufferHandle) -> Option<BufferSource> {
        self.presentations.reject(handle)
    }

    pub fn remove_source(
        &mut self,
        handle: BufferHandle,
    ) -> Result<Option<BufferSource>, LiveBufferRegistryError> {
        if self.presentations.state(handle).is_some() {
            return Err(LiveBufferRegistryError::SourceInUse);
        }
        Ok(self.sources.remove(&handle).map(|_| BufferSource::DmaBuf {
            handle: handle.raw(),
        }))
    }

    pub fn remove_fence(&mut self, fence: u64) -> bool {
        self.fences.remove(&fence).is_some()
    }

    pub fn descriptor(&self, handle: BufferHandle) -> Option<DmaBufDescriptor> {
        self.sources.get(&handle).map(|source| source.descriptor)
    }

    pub fn try_clone_plane_fd(
        &self,
        handle: BufferHandle,
        plane: usize,
    ) -> Result<OwnedFd, LiveBufferRegistryError> {
        self.sources
            .get(&handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?
            .plane_fds
            .get(plane)
            .ok_or(LiveBufferRegistryError::PlaneFdCountMismatch)?
            .try_clone()
            .map_err(|_| LiveBufferRegistryError::FdCloneFailed)
    }

    pub fn state(&self, handle: BufferHandle) -> Option<LiveBufferState> {
        self.presentations.state(handle)
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn fence_count(&self) -> usize {
        self.fences.len()
    }

    pub fn presentation_count(&self) -> usize {
        self.presentations.len()
    }

    pub fn disconnect(&mut self) -> Vec<BufferSource> {
        let _ = self.presentations.disconnect();
        let handles = self.sources.keys().copied().collect::<Vec<_>>();
        self.sources.clear();
        self.fences.clear();
        handles
            .into_iter()
            .map(|handle| BufferSource::DmaBuf {
                handle: handle.raw(),
            })
            .collect()
    }
}

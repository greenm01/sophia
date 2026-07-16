//! Protocol-neutral live presentation resources.
//!
//! Protocol frontends translate their local IDs into these typed records. The
//! backend retains renderer-private FDs, polls acquire fences, builds mixed
//! composition input, and retires each presentation by transaction identity.

use std::os::fd::OwnedFd;

use sophia_protocol::{
    BufferHandle, DmaBufDescriptor, FenceHandle, Rect, TransactionId, Transform,
};
use sophia_renderer_live::{
    LiveBufferRegistryError, LiveBufferState, LiveCompositionPlacement, LiveCpuBufferSource,
    LiveCpuComposedFrame, LiveDmaBufPresentationRegistry, LiveOwnedDmaBufPlane,
    LiveOwnedMixedCompositionFrame, LiveOwnedMixedCompositionLayer, LiveOwnedMultiPlaneDmaBufFrame,
    LivePresentationDisconnectReport, LivePresentationRegistryLimits, LivePresentationRetirement,
    LiveResourceReleaseStatus,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePresentationSubmission {
    pub transaction: TransactionId,
    pub buffer: BufferHandle,
    pub acquire_fence: Option<FenceHandle>,
    pub idle_fence: Option<FenceHandle>,
}

#[derive(Debug, Default)]
pub struct LivePresentationResourceSession {
    registry: LiveDmaBufPresentationRegistry,
}

impl LivePresentationResourceSession {
    pub fn with_limits(limits: LivePresentationRegistryLimits) -> Self {
        Self {
            registry: LiveDmaBufPresentationRegistry::with_limits(limits),
        }
    }

    pub fn register_source(
        &mut self,
        descriptor: DmaBufDescriptor,
        plane_fds: Vec<OwnedFd>,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry.register_source(descriptor, plane_fds)
    }

    pub fn register_fence(
        &mut self,
        handle: FenceHandle,
        initially_triggered: bool,
        fd: OwnedFd,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry
            .register_fence(handle, initially_triggered, fd)
    }

    pub fn begin(
        &mut self,
        submission: LivePresentationSubmission,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry.begin_present(
            submission.transaction,
            submission.buffer,
            submission.acquire_fence,
            submission.idle_fence,
        )
    }

    pub fn poll_acquire_fence(
        &mut self,
        transaction: TransactionId,
    ) -> Result<bool, LiveBufferRegistryError> {
        self.registry.poll_acquire_fence(transaction)
    }

    pub fn state(&self, transaction: TransactionId) -> Option<LiveBufferState> {
        self.registry.state(transaction)
    }

    pub fn mark_submitted(
        &mut self,
        transaction: TransactionId,
    ) -> Result<(), LiveBufferRegistryError> {
        self.registry.submit(transaction)
    }

    pub fn build_mixed_frame(
        &self,
        transaction: TransactionId,
        cpu_background: Option<LiveCpuComposedFrame>,
        target: Rect,
        clip: Option<Rect>,
        alpha: f32,
    ) -> Result<LiveOwnedMixedCompositionFrame, LiveBufferRegistryError> {
        if self.registry.state(transaction) != Some(LiveBufferState::Ready) {
            return Err(LiveBufferRegistryError::AcquireFencePending);
        }
        let handle = self
            .registry
            .source_for_presentation(transaction)
            .ok_or(LiveBufferRegistryError::UnknownPresentation)?;
        let descriptor = self
            .registry
            .descriptor(handle)
            .ok_or(LiveBufferRegistryError::UnknownHandle)?;
        let mut planes: [Option<LiveOwnedDmaBufPlane>; 4] = std::array::from_fn(|_| None);
        for index in 0..usize::from(descriptor.plane_count) {
            let plane =
                descriptor.planes[index].ok_or(LiveBufferRegistryError::PlaneFdCountMismatch)?;
            planes[index] = Some(LiveOwnedDmaBufPlane {
                fd: self
                    .registry
                    .try_clone_presentation_plane_fd(transaction, index)?,
                offset: plane.offset,
                stride: plane.stride,
            });
        }

        let mut layers = Vec::with_capacity(usize::from(cpu_background.is_some()) + 1);
        if let Some(background) = cpu_background {
            let size = background.size;
            layers.push(LiveOwnedMixedCompositionLayer::Cpu {
                buffer: LiveCpuBufferSource {
                    handle: 0,
                    size,
                    stride: background.stride,
                    format: background.format,
                    generation: 0,
                    bytes: background.bytes,
                },
                placement: LiveCompositionPlacement {
                    target: Rect {
                        x: 0,
                        y: 0,
                        width: size.width,
                        height: size.height,
                    },
                    clip: None,
                    transform: Transform::IDENTITY,
                    alpha: 1.0,
                },
            });
        }
        layers.push(LiveOwnedMixedCompositionLayer::DmaBuf {
            frame: LiveOwnedMultiPlaneDmaBufFrame {
                width: descriptor.size.width as u32,
                height: descriptor.size.height as u32,
                format: descriptor.format,
                modifier: descriptor.modifier,
                plane_count: descriptor.plane_count,
                planes,
            },
            placement: LiveCompositionPlacement {
                target,
                clip,
                transform: Transform::IDENTITY,
                alpha,
            },
        });
        Ok(LiveOwnedMixedCompositionFrame { layers })
    }

    pub fn release_source(&mut self, handle: BufferHandle) -> LiveResourceReleaseStatus {
        self.registry.remove_source(handle)
    }

    pub fn release_fence(&mut self, handle: FenceHandle) -> LiveResourceReleaseStatus {
        self.registry.remove_fence(handle)
    }

    pub fn retire_page_flip(
        &mut self,
        transaction: TransactionId,
    ) -> Option<LivePresentationRetirement> {
        self.registry.retire_page_flip(transaction)
    }

    pub fn reject(&mut self, transaction: TransactionId) -> Option<LivePresentationRetirement> {
        self.registry.reject(transaction)
    }

    pub fn disconnect(&mut self) -> LivePresentationDisconnectReport {
        self.registry.disconnect()
    }

    pub fn source_count(&self) -> usize {
        self.registry.source_count()
    }

    pub fn fence_count(&self) -> usize {
        self.registry.fence_count()
    }

    pub fn presentation_count(&self) -> usize {
        self.registry.presentation_count()
    }
}

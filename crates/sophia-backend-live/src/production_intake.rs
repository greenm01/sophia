use sophia_protocol::{
    BufferHandle, DmaBufDescriptor, FenceHandle, SurfaceId, SurfaceTransaction, TransactionId,
};
use std::os::fd::OwnedFd;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct LiveProductionDmaBufRegistration {
    pub descriptor: DmaBufDescriptor,
    pub plane_fds: Vec<Arc<OwnedFd>>,
}

#[derive(Clone, Debug)]
pub struct LiveProductionFenceRegistration {
    pub handle: FenceHandle,
    pub initially_triggered: bool,
    pub fd: Arc<OwnedFd>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveProductionPresentSubmission {
    pub transaction: TransactionId,
    pub surface: SurfaceId,
    pub buffer: BufferHandle,
    pub acquire_fence: Option<FenceHandle>,
    pub idle_fence: Option<FenceHandle>,
}

#[derive(Clone, Debug)]
pub struct LiveProductionAuthorityBatch {
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
    pub removed_surfaces: Vec<SurfaceId>,
    pub dma_buf_registrations: Vec<LiveProductionDmaBufRegistration>,
    pub fence_registrations: Vec<LiveProductionFenceRegistration>,
    pub present_submissions: Vec<LiveProductionPresentSubmission>,
    pub released_dma_bufs: Vec<BufferHandle>,
    pub released_fences: Vec<FenceHandle>,
}

#[derive(Clone, Debug)]
pub struct LiveProductionPreparedAuthorityBatch {
    pub authority_commits: Vec<sophia_protocol::TransactionCommit>,
    pub active_transactions: Vec<SurfaceTransaction>,
}

#[derive(Clone, Debug)]
pub struct LiveProductionCpuSubmission {
    pub tick: crate::LiveBackendRuntimeTickReport,
    pub composition: crate::LiveCpuCompositionReport,
    pub composed: bool,
    pub compose_elapsed: std::time::Duration,
}

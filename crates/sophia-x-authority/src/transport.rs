use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, TrySendError};

use sophia_protocol::{SurfaceId, SurfaceTransaction, TransactionId};

use crate::{
    X11CoreDispatchTrace, XAuthorityCpuBufferUpdate, XDispatchResult, XServerFrontendClientId,
};

pub const X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone, Debug)]
pub struct XAuthorityDmaBufRegistration {
    pub pixmap: crate::XResourceId,
    pub descriptor: sophia_protocol::DmaBufDescriptor,
    pub plane_fds: Vec<Arc<OwnedFd>>,
}

impl PartialEq for XAuthorityDmaBufRegistration {
    fn eq(&self, other: &Self) -> bool {
        self.pixmap == other.pixmap
            && self.descriptor == other.descriptor
            && self.plane_fds.len() == other.plane_fds.len()
    }
}

impl Eq for XAuthorityDmaBufRegistration {}

#[derive(Clone, Debug)]
pub struct XAuthorityFenceRegistration {
    pub fence: crate::XResourceId,
    pub handle: sophia_protocol::FenceHandle,
    pub initially_triggered: bool,
    pub fd: Arc<OwnedFd>,
}

impl PartialEq for XAuthorityFenceRegistration {
    fn eq(&self, other: &Self) -> bool {
        self.fence == other.fence
            && self.handle == other.handle
            && self.initially_triggered == other.initially_triggered
    }
}

impl Eq for XAuthorityFenceRegistration {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAuthorityObservedTransactionBatch {
    /// The frontend connection that caused this batch, when the source is an
    /// X11 socket dispatch. Direct authority dispatches have no connection and
    /// therefore retain `None`.
    pub client: Option<XServerFrontendClientId>,
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
    /// Frontend-confirmed surface lifetimes that ended in this batch.
    pub removed_surfaces: Vec<SurfaceId>,
    pub cpu_buffer_updates: Vec<XAuthorityCpuBufferUpdate>,
    pub dma_buf_registrations: Vec<XAuthorityDmaBufRegistration>,
    pub fence_registrations: Vec<XAuthorityFenceRegistration>,
    pub present_submissions: Vec<crate::XAuthorityPresentSubmission>,
    pub released_dma_bufs: Vec<sophia_protocol::BufferHandle>,
    pub released_fences: Vec<sophia_protocol::FenceHandle>,
}

impl XAuthorityObservedTransactionBatch {
    pub fn from_dispatch_result(result: &XDispatchResult) -> Option<Self> {
        let response = result.response.as_ref()?;
        if response.transactions.is_empty() && response.removed_surfaces.is_empty() {
            return None;
        }

        Some(Self {
            client: None,
            transaction: response.transaction,
            transactions: response.transactions.clone(),
            removed_surfaces: response.removed_surfaces.clone(),
            cpu_buffer_updates: Vec::new(),
            dma_buf_registrations: Vec::new(),
            fence_registrations: Vec::new(),
            present_submissions: Vec::new(),
            released_dma_bufs: Vec::new(),
            released_fences: Vec::new(),
        })
    }

    pub fn from_dispatch_trace(trace: &X11CoreDispatchTrace<'_>) -> Option<Self> {
        let dma_buf_registrations = trace
            .dri3_pixmap_import
            .and_then(|import| {
                let plane_fds = trace
                    .received_fds
                    .iter()
                    .map(|fd| fd.try_clone().map(Arc::new))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                Some(XAuthorityDmaBufRegistration {
                    pixmap: import.pixmap,
                    descriptor: import.descriptor,
                    plane_fds,
                })
            })
            .into_iter()
            .collect::<Vec<_>>();
        let fence_registrations = trace
            .dri3_fence_import
            .and_then(|import| {
                Some(XAuthorityFenceRegistration {
                    fence: import.fence,
                    handle: import.handle,
                    initially_triggered: import.initially_triggered,
                    fd: Arc::new(trace.received_fds.first()?.try_clone().ok()?),
                })
            })
            .into_iter()
            .collect::<Vec<_>>();
        let response = trace.result.response.as_ref();
        if response.is_none()
            && dma_buf_registrations.is_empty()
            && fence_registrations.is_empty()
            && trace.present_submission.is_none()
            && trace.released_dma_bufs.is_empty()
            && trace.released_fences.is_empty()
        {
            return None;
        }
        let transactions = response
            .map(|response| response.transactions.clone())
            .unwrap_or_default();
        let removed_surfaces = response
            .map(|response| response.removed_surfaces.clone())
            .unwrap_or_default();
        if transactions.is_empty()
            && removed_surfaces.is_empty()
            && dma_buf_registrations.is_empty()
            && fence_registrations.is_empty()
            && trace.present_submission.is_none()
            && trace.released_dma_bufs.is_empty()
            && trace.released_fences.is_empty()
        {
            return None;
        }
        Some(Self {
            client: Some(trace.client),
            transaction: response.map_or(
                TransactionId::from_raw(u64::from(trace.sequence)),
                |response| response.transaction,
            ),
            transactions,
            removed_surfaces,
            cpu_buffer_updates: trace.cpu_buffer_update.cloned().into_iter().collect(),
            dma_buf_registrations,
            fence_registrations,
            present_submissions: trace.present_submission.into_iter().collect(),
            released_dma_bufs: trace.released_dma_bufs.to_vec(),
            released_fences: trace.released_fences.to_vec(),
        })
    }
}

/// Maps Engine-visible X11 surfaces back to the frontend client that created
/// or last updated them.
///
/// The Engine owns focus and hit testing; this table gives it the connection
/// identity required to turn that surface decision into an X11 input or
/// control route. A direct authority batch has no client identity and cannot
/// establish a route. Surface removals always clear a prior route.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XAuthorityClientSurfaceRoutes {
    clients: BTreeMap<SurfaceId, XServerFrontendClientId>,
}

impl XAuthorityClientSurfaceRoutes {
    pub fn observe(&mut self, batch: &XAuthorityObservedTransactionBatch) {
        for surface in &batch.removed_surfaces {
            self.clients.remove(surface);
        }
        let Some(client) = batch.client else {
            return;
        };
        for transaction in &batch.transactions {
            self.clients.insert(transaction.surface, client);
        }
    }

    pub fn client_for_surface(&self, surface: SurfaceId) -> Option<XServerFrontendClientId> {
        self.clients.get(&surface).copied()
    }

    pub fn len(&self) -> usize {
        self.clients.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XAuthorityTransportError {
    Backpressure { transaction: TransactionId },
    Disconnected { transaction: TransactionId },
}

impl core::fmt::Display for XAuthorityTransportError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Backpressure { transaction } => write!(
                formatter,
                "X authority observed transaction channel is full for transaction {}",
                transaction.raw()
            ),
            Self::Disconnected { transaction } => write!(
                formatter,
                "X authority observed transaction channel is disconnected for transaction {}",
                transaction.raw()
            ),
        }
    }
}

impl std::error::Error for XAuthorityTransportError {}

pub fn try_emit_x_authority_transactions(
    sender: &SyncSender<XAuthorityObservedTransactionBatch>,
    result: &XDispatchResult,
) -> Result<Option<XAuthorityObservedTransactionBatch>, XAuthorityTransportError> {
    let Some(batch) = XAuthorityObservedTransactionBatch::from_dispatch_result(result) else {
        return Ok(None);
    };

    sender
        .try_send(batch.clone())
        .map_err(|error| match error {
            TrySendError::Full(batch) => XAuthorityTransportError::Backpressure {
                transaction: batch.transaction,
            },
            TrySendError::Disconnected(batch) => XAuthorityTransportError::Disconnected {
                transaction: batch.transaction,
            },
        })?;

    Ok(Some(batch))
}

pub fn try_emit_x_authority_trace(
    sender: &SyncSender<XAuthorityObservedTransactionBatch>,
    trace: &X11CoreDispatchTrace<'_>,
) -> Result<Option<XAuthorityObservedTransactionBatch>, XAuthorityTransportError> {
    let Some(batch) = XAuthorityObservedTransactionBatch::from_dispatch_trace(trace) else {
        return Ok(None);
    };

    sender
        .try_send(batch.clone())
        .map_err(|error| match error {
            TrySendError::Full(batch) => XAuthorityTransportError::Backpressure {
                transaction: batch.transaction,
            },
            TrySendError::Disconnected(batch) => XAuthorityTransportError::Disconnected {
                transaction: batch.transaction,
            },
        })?;

    Ok(Some(batch))
}

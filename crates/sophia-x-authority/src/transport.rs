use std::collections::BTreeMap;
use std::sync::mpsc::{SyncSender, TrySendError};

use sophia_protocol::{SurfaceId, SurfaceTransaction, TransactionId};

use crate::{
    X11CoreDispatchTrace, XAuthorityCpuBufferUpdate, XDispatchResult, XServerFrontendClientId,
};

pub const X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY: usize = 256;

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
        })
    }

    pub fn from_dispatch_trace(trace: &X11CoreDispatchTrace<'_>) -> Option<Self> {
        let response = trace.result.response.as_ref()?;
        if response.transactions.is_empty() && response.removed_surfaces.is_empty() {
            return None;
        }
        Some(Self {
            client: Some(trace.client),
            transaction: response.transaction,
            transactions: response.transactions.clone(),
            removed_surfaces: response.removed_surfaces.clone(),
            cpu_buffer_updates: trace.cpu_buffer_update.cloned().into_iter().collect(),
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

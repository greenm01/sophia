use std::sync::mpsc::{SyncSender, TrySendError};

use sophia_protocol::{SurfaceTransaction, TransactionId};

use crate::{X11CoreDispatchTrace, XAuthorityCpuBufferUpdate, XDispatchResult};

pub const X_AUTHORITY_OBSERVED_TRANSACTION_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XAuthorityObservedTransactionBatch {
    pub transaction: TransactionId,
    pub transactions: Vec<SurfaceTransaction>,
    pub cpu_buffer_updates: Vec<XAuthorityCpuBufferUpdate>,
}

impl XAuthorityObservedTransactionBatch {
    pub fn from_dispatch_result(result: &XDispatchResult) -> Option<Self> {
        let response = result.response.as_ref()?;
        if response.transactions.is_empty() {
            return None;
        }

        Some(Self {
            transaction: response.transaction,
            transactions: response.transactions.clone(),
            cpu_buffer_updates: Vec::new(),
        })
    }

    pub fn from_dispatch_trace(trace: &X11CoreDispatchTrace<'_>) -> Option<Self> {
        let response = trace.result.response.as_ref()?;
        if response.transactions.is_empty() {
            return None;
        }
        Some(Self {
            transaction: response.transaction,
            transactions: response.transactions.clone(),
            cpu_buffer_updates: trace.cpu_buffer_update.cloned().into_iter().collect(),
        })
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

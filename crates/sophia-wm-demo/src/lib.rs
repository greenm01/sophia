use sophia_protocol::{LayoutTransaction, TransactionId};

pub fn empty_transaction(transaction: TransactionId) -> LayoutTransaction {
    LayoutTransaction {
        transaction,
        requested_sizes: Vec::new(),
        focus: None,
        render_positions: Vec::new(),
        timeout_msec: 300,
    }
}

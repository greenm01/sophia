use sophia_protocol::{CommittedSurfaceState, SurfaceTransaction};

/// Rebase full-state presentation snapshots onto Engine's visual generation.
///
/// An authority may continue its local request sequence after a presentation
/// is skipped. Engine deliberately does not apply the skipped visual state, so
/// the two generation sequences can diverge. Presentation snapshots carry a
/// complete replacement state; rebasing only their causal generation lets a
/// later frame recover while Engine still revalidates the exact visual
/// baseline when the matching page flip arrives.
pub fn rebase_full_state_present_transactions(
    transactions: &[SurfaceTransaction],
    committed: &[CommittedSurfaceState],
) -> Vec<SurfaceTransaction> {
    transactions
        .iter()
        .cloned()
        .map(|mut transaction| {
            transaction.previous_committed_generation = committed
                .iter()
                .find(|state| state.surface == transaction.surface)
                .map_or(0, |state| state.committed_generation);
            transaction
        })
        .collect()
}

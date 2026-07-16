use sophia_cli::presentation_transaction::rebase_full_state_present_transactions;
use sophia_protocol::{
    AuthorityKind, BufferSource, CommittedSurfaceState, Rect, Region, SurfaceId,
    SurfaceTransaction, SurfaceTransactionReadiness, TransactionId,
};

fn transaction(surface: SurfaceId, previous_generation: u64) -> SurfaceTransaction {
    SurfaceTransaction {
        transaction: TransactionId::from_raw(previous_generation + 100),
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: Rect {
            x: 20,
            y: 30,
            width: 640,
            height: 480,
        },
        target_buffer: BufferSource::DmaBuf { handle: 91 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: previous_generation,
    }
}

#[test]
fn first_present_rebases_to_the_empty_engine_generation() {
    let surface = SurfaceId::new(1, 1);
    let observed = transaction(surface, 7);

    let rebased = rebase_full_state_present_transactions(&[observed.clone()], &[]);

    assert_eq!(rebased[0].previous_committed_generation, 0);
    assert_eq!(rebased[0].target_geometry, observed.target_geometry);
    assert_eq!(rebased[0].target_buffer, observed.target_buffer);
}

#[test]
fn present_after_skip_reuses_the_last_visual_generation() {
    let surface = SurfaceId::new(2, 1);
    let committed = vec![CommittedSurfaceState {
        surface,
        committed_generation: 3,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        },
        buffer: BufferSource::DmaBuf { handle: 40 },
        damage: Region::empty(),
    }];

    let rebased = rebase_full_state_present_transactions(&[transaction(surface, 19)], &committed);

    assert_eq!(rebased[0].previous_committed_generation, 3);
    assert_eq!(committed[0].buffer, BufferSource::DmaBuf { handle: 40 });
}

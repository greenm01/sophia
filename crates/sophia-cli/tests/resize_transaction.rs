use std::collections::BTreeMap;

use sophia_cli::resize_transaction::{
    ResizeRollbackCoordinator, project_authority_batch_onto_layout,
};
use sophia_protocol::{
    AuthorityKind, BufferSource, LayerSnapshot, Rect, Region, ResizeSyncCapability, Size,
    SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness, TransactionId, Transform,
};
use sophia_x_authority::{
    XAuthorityCpuBufferSnapshot, XAuthorityCpuBufferUpdate, XAuthorityObservedTransactionBatch,
    XResourceId,
};

fn size(width: i32, height: i32) -> Size {
    Size { width, height }
}

#[test]
fn successful_resize_advances_the_committed_size() {
    let surface = SurfaceId::new(1, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    assert!(coordinator.accept_observation(surface, size(1024, 768)));
    coordinator.record_committed(surface, size(1024, 768));
    assert_eq!(coordinator.committed_size(surface), Some(size(1024, 768)));
    assert!(!coordinator.rollback_pending(surface));
}

#[test]
fn timeout_builds_a_compensating_configure_from_committed_state() {
    let surface = SurfaceId::new(2, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    let rollback = coordinator
        .begin_rollback([(surface, size(1024, 768))])
        .unwrap();
    assert_eq!(rollback.len(), 1);
    assert_eq!(rollback[0].surface, surface);
    assert_eq!(rollback[0].size, size(800, 600));
    assert!(rollback[0].transaction.raw() >= 1 << 63);
    assert!(coordinator.rollback_pending(surface));
    assert!(!coordinator.request_allowed(surface, size(1024, 768)));
    assert!(coordinator.request_allowed(surface, size(1280, 720)));
}

#[test]
fn late_abandoned_pixels_are_fenced_until_rollback_confirmation() {
    let surface = SurfaceId::new(3, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    coordinator
        .begin_rollback([(surface, size(1024, 768))])
        .unwrap();
    assert!(!coordinator.accept_observation(surface, size(1024, 768)));
    assert!(coordinator.accept_observation(surface, size(800, 600)));
    assert!(!coordinator.rollback_pending(surface));
}

#[test]
fn disconnect_cleans_committed_and_rollback_state() {
    let surface = SurfaceId::new(4, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    coordinator
        .begin_rollback([(surface, size(1024, 768))])
        .unwrap();
    coordinator.remove(surface);
    assert_eq!(coordinator.committed_size(surface), None);
    assert!(!coordinator.rollback_pending(surface));
    assert!(coordinator.request_allowed(surface, size(1024, 768)));
    assert!(coordinator.rollback_surfaces().next().is_none());
}

#[test]
fn resize_projection_preserves_generation_chain_and_cpu_updates() {
    let surface = SurfaceId::new(5, 1);
    let transaction = SurfaceTransaction {
        transaction: TransactionId::from_raw(118),
        authority: AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 800,
        },
        target_buffer: BufferSource::CpuBuffer { handle: 9 },
        damage: Region::single(Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 800,
        }),
        readiness: SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: 90,
    };
    let update = XAuthorityCpuBufferUpdate::Replace(XAuthorityCpuBufferSnapshot {
        handle: 9,
        drawable: XResourceId::new(9, 1),
        size: size(640, 800),
        stride: 2_560,
        format: u32::from_le_bytes(*b"XR24"),
        generation: 91,
        bytes: vec![1; 640 * 800 * 4],
    });
    let batch = XAuthorityObservedTransactionBatch {
        client: None,
        transaction: transaction.transaction,
        transactions: vec![transaction.clone()],
        removed_surfaces: Vec::new(),
        cpu_buffer_updates: vec![update.clone()],
        dma_buf_registrations: Vec::new(),
        fence_registrations: Vec::new(),
        present_submissions: Vec::new(),
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        protocol_errors: Vec::new(),
        expected_protocol_errors: Vec::new(),
        metadata: Vec::new(),
        selection_owner_change: false,
        selection_conversion: false,
    };
    let committed_geometry = Rect {
        x: 20,
        y: 30,
        width: 1280,
        height: 800,
    };
    let layers = BTreeMap::from([(
        surface,
        LayerSnapshot {
            surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: 0,
            geometry: committed_geometry,
            source: transaction.target_buffer,
            damage: Region::empty(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: 90,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        },
    )]);

    let projected = project_authority_batch_onto_layout(batch, &layers);

    assert_eq!(projected.transaction, TransactionId::from_raw(118));
    assert_eq!(projected.transactions.len(), 1);
    assert_eq!(projected.transactions[0].previous_committed_generation, 90);
    assert_eq!(
        projected.transactions[0].target_geometry,
        committed_geometry
    );
    assert_eq!(projected.cpu_buffer_updates, vec![update]);
}

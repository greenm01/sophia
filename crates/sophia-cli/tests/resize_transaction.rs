use sophia_cli::resize_transaction::ResizeRollbackCoordinator;
use sophia_protocol::{Size, SurfaceId};

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
    let rollback = coordinator.begin_rollback([surface]).unwrap();
    assert_eq!(rollback.len(), 1);
    assert_eq!(rollback[0].surface, surface);
    assert_eq!(rollback[0].size, size(800, 600));
    assert!(rollback[0].transaction.raw() >= 1 << 63);
    assert!(coordinator.rollback_pending(surface));
}

#[test]
fn late_abandoned_pixels_are_fenced_until_rollback_confirmation() {
    let surface = SurfaceId::new(3, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    coordinator.begin_rollback([surface]).unwrap();
    assert!(!coordinator.accept_observation(surface, size(1024, 768)));
    assert!(coordinator.accept_observation(surface, size(800, 600)));
    assert!(!coordinator.rollback_pending(surface));
}

#[test]
fn disconnect_cleans_committed_and_rollback_state() {
    let surface = SurfaceId::new(4, 1);
    let mut coordinator = ResizeRollbackCoordinator::default();
    coordinator.record_committed(surface, size(800, 600));
    coordinator.begin_rollback([surface]).unwrap();
    coordinator.remove(surface);
    assert_eq!(coordinator.committed_size(surface), None);
    assert!(!coordinator.rollback_pending(surface));
    assert!(coordinator.rollback_surfaces().next().is_none());
}

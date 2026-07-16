use std::fs::File;
use std::os::fd::OwnedFd;

use sophia_protocol::{
    BufferHandle, BufferSource, DRM_FORMAT_MOD_INVALID, DRM_FORMAT_XRGB8888, DmaBufDescriptor,
    DmaBufPlaneDescriptor, FenceHandle, Size, TransactionId,
};
use sophia_renderer_live::{
    LiveBufferRegistry, LiveBufferRegistryError, LiveBufferState, LiveCpuBufferLifetimeRegistry,
    LiveDmaBufPresentationRegistry, LiveIdleFenceStatus, LivePresentationRegistryLimits,
    LiveResourceReleaseStatus,
};

fn fd() -> OwnedFd {
    File::open("/dev/null").unwrap().into()
}

fn descriptor(raw: u64) -> DmaBufDescriptor {
    DmaBufDescriptor {
        handle: BufferHandle::from_raw(raw),
        size: Size {
            width: 64,
            height: 48,
        },
        format: DRM_FORMAT_XRGB8888,
        modifier: DRM_FORMAT_MOD_INVALID,
        plane_count: 1,
        planes: [
            Some(DmaBufPlaneDescriptor {
                offset: 0,
                stride: 256,
            }),
            None,
            None,
            None,
        ],
    }
}

#[test]
fn acquire_fence_and_page_flip_delay_release_exactly_once() {
    let handle = BufferHandle::from_raw(7);
    let mut registry = LiveBufferRegistry::default();
    registry
        .register(descriptor(7), vec![fd()], Some(fd()))
        .unwrap();
    assert_eq!(
        registry.submit(handle),
        Err(LiveBufferRegistryError::AcquireFencePending)
    );
    registry.signal_acquire_fence(handle).unwrap();
    assert_eq!(registry.state(handle), Some(LiveBufferState::Ready));
    registry.submit(handle).unwrap();
    assert_eq!(
        registry.retire_page_flip(handle),
        Some(BufferSource::DmaBuf { handle: 7 })
    );
    assert_eq!(registry.retire_page_flip(handle), None);
    assert_eq!(registry.reject(handle), None);
}

#[test]
fn rejection_and_disconnect_release_each_owned_registration_once() {
    let mut registry = LiveBufferRegistry::default();
    registry.register(descriptor(9), vec![fd()], None).unwrap();
    registry.register(descriptor(10), vec![fd()], None).unwrap();
    assert_eq!(
        registry.reject(BufferHandle::from_raw(9)),
        Some(BufferSource::DmaBuf { handle: 9 })
    );
    assert_eq!(registry.reject(BufferHandle::from_raw(9)), None);
    assert_eq!(
        registry.disconnect(),
        vec![BufferSource::DmaBuf { handle: 10 }]
    );
    assert!(registry.disconnect().is_empty());
}

#[test]
fn cpu_rejection_and_stale_retirement_preserve_last_good_pixels() {
    let surface = sophia_protocol::SurfaceId::new(3, 1);
    let mut registry = LiveCpuBufferLifetimeRegistry::default();
    assert_eq!(registry.submit(surface, 11), None);
    assert!(registry.retire_page_flip(surface, 11).is_empty());
    assert_eq!(registry.committed_handle(surface), Some(11));

    assert_eq!(registry.submit(surface, 12), None);
    assert!(registry.retire_page_flip(surface, 99).is_empty());
    assert_eq!(registry.committed_handle(surface), Some(11));
    assert_eq!(
        registry.reject(surface, 12),
        Some(BufferSource::CpuBuffer { handle: 12 })
    );
    assert_eq!(registry.committed_handle(surface), Some(11));

    registry.submit(surface, 13);
    assert_eq!(
        registry.retire_page_flip(surface, 13),
        vec![BufferSource::CpuBuffer { handle: 11 }]
    );
    assert_eq!(registry.committed_handle(surface), Some(13));
    assert_eq!(
        registry.disconnect(),
        vec![BufferSource::CpuBuffer { handle: 13 }]
    );
    assert!(registry.disconnect().is_empty());
}

#[test]
fn real_xshmfence_poll_holds_submission_until_triggered() {
    let fence = sophia_xshmfence::allocate().unwrap();
    let trigger = fence.try_clone().unwrap();
    let handle = BufferHandle::from_raw(21);
    let mut registry = LiveBufferRegistry::default();
    registry
        .register(descriptor(21), vec![fd()], Some(fence))
        .unwrap();
    assert_eq!(registry.poll_acquire_fence(handle), Ok(false));
    assert_eq!(
        registry.submit(handle),
        Err(LiveBufferRegistryError::AcquireFencePending)
    );
    sophia_xshmfence::trigger(&trigger).unwrap();
    assert_eq!(registry.poll_acquire_fence(handle), Ok(true));
    registry.submit(handle).unwrap();
}

#[test]
fn reusable_dmabuf_source_survives_fenced_present_retirement() {
    let handle = BufferHandle::from_raw(31);
    let fence_id = FenceHandle::from_raw(41);
    let first = TransactionId::from_raw(51);
    let second = TransactionId::from_raw(52);
    let fence = sophia_xshmfence::allocate().unwrap();
    let trigger = fence.try_clone().unwrap();
    let idle_fence = sophia_xshmfence::allocate().unwrap();
    let idle_query = idle_fence.try_clone().unwrap();
    let idle_fence_id = FenceHandle::from_raw(42);
    let mut registry = LiveDmaBufPresentationRegistry::default();
    registry
        .register_source(descriptor(handle.raw()), vec![fd()])
        .unwrap();
    registry.register_fence(fence_id, false, fence).unwrap();
    registry
        .register_fence(idle_fence_id, false, idle_fence)
        .unwrap();

    registry
        .begin_present(first, handle, Some(fence_id), Some(idle_fence_id))
        .unwrap();
    assert_eq!(
        registry.state(first),
        Some(LiveBufferState::WaitingForAcquireFence)
    );
    assert_eq!(registry.poll_acquire_fence(first), Ok(false));
    sophia_xshmfence::trigger(&trigger).unwrap();
    assert_eq!(registry.poll_acquire_fence(first), Ok(true));
    registry.submit(first).unwrap();
    registry.try_clone_presentation_plane_fd(first, 0).unwrap();
    let retirement = registry.retire_page_flip(first).unwrap();
    assert_eq!(
        retirement.source,
        BufferSource::DmaBuf {
            handle: handle.raw()
        }
    );
    assert_eq!(retirement.idle_fence, LiveIdleFenceStatus::Triggered);
    assert!(sophia_xshmfence::query(&idle_query).unwrap());
    assert_eq!(registry.presentation_count(), 0);
    assert_eq!(registry.source_count(), 1);

    registry.begin_present(second, handle, None, None).unwrap();
    assert_eq!(registry.state(second), Some(LiveBufferState::Ready));
    registry.submit(second).unwrap();
    assert!(registry.retire_page_flip(second).is_some());
    assert_eq!(
        registry.remove_source(handle),
        LiveResourceReleaseStatus::Released
    );
    assert_eq!(
        registry.remove_fence(fence_id),
        LiveResourceReleaseStatus::Released
    );
}

#[test]
fn persistent_dmabuf_disconnect_releases_each_source_once() {
    let mut registry = LiveDmaBufPresentationRegistry::default();
    registry
        .register_source(descriptor(51), vec![fd()])
        .unwrap();
    registry
        .register_source(descriptor(52), vec![fd()])
        .unwrap();
    registry
        .begin_present(
            TransactionId::from_raw(61),
            BufferHandle::from_raw(51),
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        registry.remove_source(BufferHandle::from_raw(51)),
        LiveResourceReleaseStatus::Deferred
    );
    let report = registry.disconnect();
    assert_eq!(
        report.released_sources,
        vec![
            BufferSource::DmaBuf { handle: 51 },
            BufferSource::DmaBuf { handle: 52 },
        ]
    );
    assert_eq!(report.retired_presentations, 1);
    let second = registry.disconnect();
    assert!(second.released_sources.is_empty());
    assert_eq!(second.retired_presentations, 0);
}

#[test]
fn repeated_pixmap_presentations_are_transaction_keyed_and_release_deferred() {
    let handle = BufferHandle::from_raw(71);
    let first = TransactionId::from_raw(81);
    let second = TransactionId::from_raw(82);
    let mut registry = LiveDmaBufPresentationRegistry::default();
    registry
        .register_source(descriptor(handle.raw()), vec![fd()])
        .unwrap();
    registry.begin_present(first, handle, None, None).unwrap();
    registry.begin_present(second, handle, None, None).unwrap();
    assert_eq!(registry.presentation_count(), 2);
    assert_eq!(
        registry.remove_source(handle),
        LiveResourceReleaseStatus::Deferred
    );

    registry.submit(first).unwrap();
    let first_retirement = registry.retire_page_flip(first).unwrap();
    assert!(!first_retirement.released_source);
    assert_eq!(registry.source_count(), 1);
    let second_retirement = registry.reject(second).unwrap();
    assert!(second_retirement.released_source);
    assert_eq!(registry.source_count(), 0);
    assert_eq!(registry.presentation_count(), 0);
}

#[test]
fn presentation_registry_limits_fail_closed() {
    let limits = LivePresentationRegistryLimits {
        sources: 1,
        fences: 1,
        presentations: 1,
    };
    let mut registry = LiveDmaBufPresentationRegistry::with_limits(limits);
    registry
        .register_source(descriptor(91), vec![fd()])
        .unwrap();
    assert_eq!(
        registry.register_source(descriptor(92), vec![fd()]),
        Err(LiveBufferRegistryError::CapacityExceeded)
    );
    registry
        .begin_present(
            TransactionId::from_raw(91),
            BufferHandle::from_raw(91),
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        registry.begin_present(
            TransactionId::from_raw(92),
            BufferHandle::from_raw(91),
            None,
            None,
        ),
        Err(LiveBufferRegistryError::CapacityExceeded)
    );
}

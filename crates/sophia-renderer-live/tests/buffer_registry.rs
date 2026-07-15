use std::fs::File;
use std::os::fd::OwnedFd;

use sophia_protocol::{
    BufferHandle, BufferSource, DRM_FORMAT_MOD_INVALID, DRM_FORMAT_XRGB8888, DmaBufDescriptor,
    DmaBufPlaneDescriptor, Size,
};
use sophia_renderer_live::{
    LiveBufferRegistry, LiveBufferRegistryError, LiveBufferState, LiveCpuBufferLifetimeRegistry,
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

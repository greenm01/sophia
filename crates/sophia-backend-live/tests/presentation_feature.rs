#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use std::fs::File;
use std::os::fd::OwnedFd;

use sophia_backend_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuComposedFrame, LivePresentationResourceSession,
    LivePresentationSubmission, LiveResourceReleaseStatus,
};
use sophia_protocol::{
    BufferHandle, BufferSource, DRM_FORMAT_MOD_INVALID, DmaBufDescriptor, DmaBufPlaneDescriptor,
    Rect, Size, TransactionId,
};

fn fd() -> OwnedFd {
    File::open("/dev/null").unwrap().into()
}

fn descriptor(handle: BufferHandle) -> DmaBufDescriptor {
    DmaBufDescriptor {
        handle,
        size: Size {
            width: 64,
            height: 48,
        },
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
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
fn backend_session_builds_mixed_cpu_gpu_frame_and_retires_exactly_once() {
    let handle = BufferHandle::from_raw(7);
    let transaction = TransactionId::from_raw(8);
    let mut session = LivePresentationResourceSession::default();
    session
        .register_source(descriptor(handle), vec![fd()])
        .unwrap();
    session
        .begin(LivePresentationSubmission {
            transaction,
            buffer: handle,
            acquire_fence: None,
            idle_fence: None,
        })
        .unwrap();
    let cpu = LiveCpuComposedFrame {
        size: Size {
            width: 128,
            height: 96,
        },
        stride: 512,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        bytes: vec![1; 128 * 96 * 4],
    };

    let frame = session
        .build_mixed_frame(
            transaction,
            Some(cpu),
            Rect {
                x: 20,
                y: 10,
                width: 64,
                height: 48,
            },
            None,
            1.0,
        )
        .unwrap();
    assert_eq!(frame.layers.len(), 2);
    session.mark_submitted(transaction).unwrap();
    assert_eq!(
        session.release_source(handle),
        LiveResourceReleaseStatus::Deferred
    );
    let retired = session.retire_page_flip(transaction).unwrap();
    assert_eq!(retired.source, BufferSource::DmaBuf { handle: 7 });
    assert!(retired.released_source);
    assert!(session.retire_page_flip(transaction).is_none());
    assert_eq!(session.source_count(), 0);
    assert_eq!(session.presentation_count(), 0);
}

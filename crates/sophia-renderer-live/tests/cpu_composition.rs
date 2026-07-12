use sophia_protocol::{Rect, Size};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferSource, LiveCpuBufferSourceRef,
    LiveCpuCompositionLayer, LiveCpuCompositionLayerRef, compose_live_cpu_frame,
    compose_live_cpu_frame_ref,
};

#[test]
fn cpu_composition_blits_clipped_xrgb_layers() {
    let source = LiveCpuBufferSource {
        handle: 1,
        size: Size {
            width: 2,
            height: 2,
        },
        stride: 8,
        format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        generation: 1,
        bytes: vec![0xff; 16],
    };
    let report = compose_live_cpu_frame(
        Size {
            width: 3,
            height: 3,
        },
        &[LiveCpuCompositionLayer {
            geometry: Rect {
                x: 2,
                y: 2,
                width: 2,
                height: 2,
            },
            buffer: source,
        }],
    )
    .unwrap();

    assert_eq!(report.layers_input, 1);
    assert_eq!(report.layers_composed, 1);
    assert_eq!(report.nonzero_pixel_bytes, 4);
    assert_ne!(report.checksum, 0);
    assert_eq!(&report.frame.bytes[32..36], &[0xff; 4]);
}

#[test]
fn borrowed_fullscreen_composition_preserves_pixels_and_metrics() {
    let pixels = vec![0x5a; 1280 * 720 * 4];
    let report = compose_live_cpu_frame_ref(
        Size {
            width: 1280,
            height: 720,
        },
        &[LiveCpuCompositionLayerRef {
            geometry: Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            buffer: LiveCpuBufferSourceRef {
                handle: 7,
                size: Size {
                    width: 1280,
                    height: 720,
                },
                stride: 1280 * 4,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 2,
                bytes: &pixels,
            },
        }],
    )
    .unwrap();
    assert_eq!(report.frame.bytes, pixels);
    assert_eq!(report.nonzero_pixel_bytes, 1280 * 720 * 4);
    assert_eq!(report.layers_composed, 1);
}

#[test]
fn borrowed_composition_clips_negative_geometry_by_rows() {
    let pixels = vec![0x33; 4 * 4 * 4];
    let report = compose_live_cpu_frame_ref(
        Size {
            width: 4,
            height: 4,
        },
        &[LiveCpuCompositionLayerRef {
            geometry: Rect {
                x: -2,
                y: -1,
                width: 4,
                height: 4,
            },
            buffer: LiveCpuBufferSourceRef {
                handle: 9,
                size: Size {
                    width: 4,
                    height: 4,
                },
                stride: 16,
                format: LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                generation: 1,
                bytes: &pixels,
            },
        }],
    )
    .unwrap();
    assert_eq!(report.nonzero_pixel_bytes, 2 * 3 * 4);
    assert_eq!(&report.frame.bytes[..8], &[0x33; 8]);
    assert!(report.frame.bytes[8..16].iter().all(|byte| *byte == 0));
}

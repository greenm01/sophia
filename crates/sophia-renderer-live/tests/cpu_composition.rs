use sophia_protocol::{Rect, Size};
use sophia_renderer_live::{
    LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, LiveCpuBufferSource, LiveCpuCompositionLayer,
    compose_live_cpu_frame,
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

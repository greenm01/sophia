use sophia_backend_live::{
    BufferImportPath, BufferSource, LiveRendererImportBoundary, LiveRendererImportDecision,
    LiveRendererImportRejection,
};

#[test]
fn cpu_upload_is_the_default_accepted_renderer_import_path() {
    let boundary = LiveRendererImportBoundary::cpu_only();

    assert_eq!(
        boundary.decide(BufferSource::CpuBuffer { handle: 77 }),
        LiveRendererImportDecision::Accepted {
            path: BufferImportPath::CpuReadback,
        }
    );
}

#[test]
fn empty_sources_fail_closed_before_renderer_import() {
    let boundary = LiveRendererImportBoundary::cpu_only();

    assert_eq!(
        boundary.decide(BufferSource::None),
        LiveRendererImportDecision::Rejected {
            reason: LiveRendererImportRejection::EmptySource,
        }
    );
}

#[test]
fn native_import_sources_are_deferred_without_a_live_renderer_boundary() {
    let boundary = LiveRendererImportBoundary::cpu_only();

    assert_eq!(
        boundary.decide(BufferSource::XPixmap { pixmap: 44 }),
        LiveRendererImportDecision::Deferred {
            requested: BufferImportPath::XPixmap,
            required_boundary: "live XPixmap renderer import",
        }
    );
    assert_eq!(
        boundary.decide(BufferSource::DmaBuf { handle: 55 }),
        LiveRendererImportDecision::Deferred {
            requested: BufferImportPath::DmaBuf,
            required_boundary: "live DMA-BUF renderer import",
        }
    );
}

#[test]
fn native_import_sources_can_be_admitted_by_an_explicit_renderer_boundary() {
    let boundary = LiveRendererImportBoundary::with_native_imports(true, true);

    assert_eq!(
        boundary.decide(BufferSource::XPixmap { pixmap: 44 }),
        LiveRendererImportDecision::Accepted {
            path: BufferImportPath::XPixmap,
        }
    );
    assert_eq!(
        boundary.decide(BufferSource::DmaBuf { handle: 55 }),
        LiveRendererImportDecision::Accepted {
            path: BufferImportPath::DmaBuf,
        }
    );
}

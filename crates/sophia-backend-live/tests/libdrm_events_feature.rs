#![cfg(feature = "libdrm-events")]

use std::{io, sync::mpsc};

#[cfg(feature = "gbm-probe")]
use sophia_backend_live::RenderDeviceDiscoveryBackend;
use sophia_backend_live::{
    CompositorBackendTickInput, FakeLibdrmNativePageFlipReader, FakeLibdrmPageFlipEventPoller,
    LibdrmBackendFdAuthority, LibdrmBackendFdAuthorityReport, LibdrmBackendFdAuthorityStatus,
    LibdrmDependencyAdmissionReport, LibdrmDependencyAdmissionStatus,
    LibdrmNativeAtomicCommitDevice, LibdrmNativeAtomicCommitFlagsReport,
    LibdrmNativeAtomicCommitRequest, LibdrmNativeAtomicCommitSubmitReport,
    LibdrmNativeAtomicCommitSubmitStatus, LibdrmNativeAtomicRequestBuildStatus,
    LibdrmNativeAtomicScanoutSmokeEvidence, LibdrmNativeAtomicScanoutSmokeStatus,
    LibdrmNativeConnectorSnapshot, LibdrmNativeCrtcRoute, LibdrmNativeEncoderSnapshot,
    LibdrmNativeEventAdapterReport, LibdrmNativeEventAdapterStatus, LibdrmNativeKmsSelectionDevice,
    LibdrmNativeOutputRoute, LibdrmNativeOutputSlot, LibdrmNativePageFlipCallback,
    LibdrmNativePageFlipDecodeReport, LibdrmNativePageFlipDecodeStatus,
    LibdrmNativePageFlipReadResult, LibdrmNativePageFlipReader, LibdrmNativePageFlipSource,
    LibdrmNativePageFlipSourceReport, LibdrmNativePageFlipSourceStatus, LibdrmNativePlaneSnapshot,
    LibdrmNativePollerDiagnostics, LibdrmNativePrimaryPlaneObjects,
    LibdrmNativePrimaryPlanePropertyDiscoveryStatus, LibdrmNativePrimaryPlanePropertyHandles,
    LibdrmNativePrimaryPlaneResourceCreateStatus, LibdrmNativePrimaryPlaneResourceDestroyStatus,
    LibdrmNativePrimaryPlaneResourceDevice, LibdrmNativePrimaryPlaneScanoutRetireStatus,
    LibdrmNativePrimaryPlaneScanoutSubmitStatus, LibdrmNativePrimaryPlaneSelectionResult,
    LibdrmNativePrimaryPlaneSelectionStatus, LibdrmNativePropertyHandleSet,
    LibdrmNativePropertyLookupDevice, LibdrmNativeReadAndPollReport, LibdrmNativeReadLoopReport,
    LibdrmNativeReadLoopStatus, LibdrmNativeRenderedScanoutContextStatus,
    LibdrmPageFlipEventPollReport, LibdrmPageFlipEventPollStatus, LibdrmPageFlipEventPoller,
    LibdrmRendererScanoutBuffer, LiveBackendConfig, LiveGbmEglFrameTargetStatus,
    LiveHardwareValidationGateReport, LiveHardwareValidationGateStatus,
    LiveHardwareValidationSmokeReport, LiveHardwareValidationSmokeStatus,
    LiveHardwareValidationTarget, LiveKmsScanoutTargetStatus, LiveLibdrmPollerDiagnostics,
    LiveLibdrmPollerDiagnosticsStatus, LiveLibdrmPollerStartupReport,
    LiveLibdrmPollerStartupStatus, LivePageFlipCallback, LivePageFlipCallbackDecision,
    LivePageFlipCallbackQueue, LivePageFlipCallbackReport, LivePageFlipCallbackSourceReport,
    LivePageFlipEvent, LivePageFlipEventStatus, LiveRenderedPrimaryPlaneScanoutBackpressureReport,
    LiveRenderedPrimaryPlaneScanoutBackpressureStatus, LiveRenderedPrimaryPlaneScanoutSubmitStatus,
    LiveRenderedScanoutBufferExport, LiveRenderedScanoutBufferExporter,
    LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus,
    LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus,
    NativeGbmRenderedScanoutBufferDiscoveryExporter, NativeGbmRenderedScanoutContextStatus,
    NativeLibdrmAtomicScanoutCommitter, NativeLibdrmPageFlipEventPoller,
    NativeLibdrmPageFlipEventReader, OutputId, QueuedInputPoller, RuntimeScanoutState, Size,
    build_native_primary_plane_atomic_request, create_native_primary_plane_resources,
    decode_native_page_flip_batch, destroy_native_primary_plane_resources, discover_live_backend,
    discover_native_primary_plane_property_handles, libdrm_dependency_admission_report,
    libdrm_fd_authority_report, native_libdrm_event_adapter_report,
    native_libdrm_event_adapter_report_for_authority, real_atomic_scanout_validation_gate,
    real_atomic_scanout_validation_smoke_report, real_libdrm_events_validation_gate,
    real_libdrm_events_validation_smoke_report, reduce_native_page_flip_event,
    retire_native_primary_plane_scanout_after_page_flip,
    retire_rendered_primary_plane_scanout_after_page_flip, select_native_primary_plane_target,
    submit_native_primary_plane_scanout_from_renderer_descriptor,
    submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor,
};
use sophia_renderer_live::{
    FakeRendererScanoutBufferExporter, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
    LiveGbmEglFrameTargetLifecycleReport, LiveGbmEglFrameTargetLifecycleStatus,
    LiveGbmEglFrameTargetRecord, LiveRendererScanoutBufferExportStatus,
    LiveRendererScanoutBufferExporter, NativeGbmRenderedScanoutContext,
};

#[test]
fn libdrm_dependency_is_admitted_without_exposing_native_event_shape() {
    assert_eq!(
        libdrm_dependency_admission_report(),
        LibdrmDependencyAdmissionReport {
            status: LibdrmDependencyAdmissionStatus::TypedPageFlipEventAvailable,
        }
    );
}

#[test]
fn real_libdrm_event_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert!(!skipped.is_requested());
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::LibdrmEvents,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_libdrm_events_validation_gate().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn real_libdrm_event_validation_smoke_fails_closed_without_device_opening_smoke() {
    let skipped = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            false,
        ),
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::SkippedOptInRequired,
        }
    );

    let requested = LiveHardwareValidationSmokeReport::fail_closed_from_gate(
        LiveHardwareValidationGateReport::from_env_presence(
            LiveHardwareValidationTarget::LibdrmEvents,
            true,
        ),
    );
    assert_eq!(
        requested,
        LiveHardwareValidationSmokeReport {
            target: LiveHardwareValidationTarget::LibdrmEvents,
            status: LiveHardwareValidationSmokeStatus::BackendUnavailable,
        }
    );

    assert_eq!(
        real_libdrm_events_validation_smoke_report().target,
        LiveHardwareValidationTarget::LibdrmEvents
    );
}

#[test]
fn real_atomic_scanout_validation_gate_is_explicit_and_reduced() {
    let skipped = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::AtomicScanout,
        false,
    );
    assert_eq!(
        skipped,
        LiveHardwareValidationGateReport {
            target: LiveHardwareValidationTarget::AtomicScanout,
            status: LiveHardwareValidationGateStatus::SkippedOptInRequired,
        }
    );
    assert_eq!(
        skipped.target.env_var(),
        "SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE"
    );

    let requested = LiveHardwareValidationGateReport::from_env_presence(
        LiveHardwareValidationTarget::AtomicScanout,
        true,
    );
    assert_eq!(
        requested.status,
        LiveHardwareValidationGateStatus::Requested
    );
    assert!(requested.is_requested());

    assert_eq!(
        real_atomic_scanout_validation_gate().target,
        LiveHardwareValidationTarget::AtomicScanout
    );
    assert_eq!(
        real_atomic_scanout_validation_smoke_report().target,
        LiveHardwareValidationTarget::AtomicScanout
    );
}

#[test]
fn libdrm_fd_authority_is_generation_checked_and_reduced() {
    assert_eq!(LibdrmBackendFdAuthority::new(0), None);

    let authority =
        LibdrmBackendFdAuthority::new(9).expect("nonzero generation should mint authority token");
    assert_eq!(authority.generation(), 9);
    assert_eq!(
        libdrm_fd_authority_report(authority),
        LibdrmBackendFdAuthorityReport {
            status: LibdrmBackendFdAuthorityStatus::BackendOwned,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_skeleton_reports_ready_without_opening_devices() {
    assert_eq!(
        native_libdrm_event_adapter_report(),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_event_adapter_accepts_authority_without_polling() {
    let authority =
        LibdrmBackendFdAuthority::new(12).expect("nonzero generation should mint authority token");

    assert_eq!(
        native_libdrm_event_adapter_report_for_authority(authority),
        LibdrmNativeEventAdapterReport {
            status: LibdrmNativeEventAdapterStatus::SkeletonReady,
        }
    );
}

#[test]
fn native_libdrm_page_flip_source_constructs_from_authority_without_reading_events() {
    let authority =
        LibdrmBackendFdAuthority::new(13).expect("nonzero generation should mint authority token");
    let source = LibdrmNativePageFlipSource::from_authority(authority);

    assert_eq!(
        source.report(),
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    );
}

#[derive(Debug)]
struct FakeNativeAtomicCommitDevice {
    result: io::Result<()>,
}

impl LibdrmNativeAtomicCommitDevice for FakeNativeAtomicCommitDevice {
    fn submit_atomic_commit(
        &self,
        _flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        self.result
            .as_ref()
            .map(|_| ())
            .map_err(|error| io::Error::new(error.kind(), "synthetic atomic commit failure"))
    }
}

#[derive(Debug)]
struct FakeNativePropertyLookupDevice {
    connector: io::Result<LibdrmNativePropertyHandleSet>,
    crtc: io::Result<LibdrmNativePropertyHandleSet>,
    plane: io::Result<LibdrmNativePropertyHandleSet>,
}

impl LibdrmNativePropertyLookupDevice for FakeNativePropertyLookupDevice {
    fn connector_property_handles(
        &self,
        _connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.connector)
    }

    fn crtc_property_handles(
        &self,
        _crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.crtc)
    }

    fn plane_property_handles(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        clone_io_result(&self.plane)
    }
}

#[derive(Debug)]
struct FakeNativeKmsSelectionDevice {
    connectors: io::Result<Vec<drm::control::connector::Handle>>,
    crtcs: io::Result<Vec<drm::control::crtc::Handle>>,
    planes: io::Result<Vec<drm::control::plane::Handle>>,
    connector_snapshot: io::Result<LibdrmNativeConnectorSnapshot>,
    encoder_snapshot: io::Result<LibdrmNativeEncoderSnapshot>,
    plane_snapshot: io::Result<LibdrmNativePlaneSnapshot>,
    plane_type: io::Result<Option<drm::control::PlaneType>>,
}

impl LibdrmNativeKmsSelectionDevice for FakeNativeKmsSelectionDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        clone_io_result(&self.connectors)
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        clone_io_result(&self.crtcs)
    }

    fn connector_snapshot(
        &self,
        _connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        clone_io_result(&self.connector_snapshot)
    }

    fn encoder_snapshot(
        &self,
        _encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        clone_io_result(&self.encoder_snapshot)
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        clone_io_result(&self.planes)
    }

    fn plane_snapshot(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        clone_io_result(&self.plane_snapshot)
    }

    fn plane_type(
        &self,
        _plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        clone_io_result(&self.plane_type)
    }
}

#[derive(Debug)]
struct FakeNativePrimaryPlaneResourceDevice {
    mode_blob: io::Result<u64>,
    framebuffer: io::Result<drm::control::framebuffer::Handle>,
    destroy_framebuffer: io::Result<()>,
    destroy_mode_blob: io::Result<()>,
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeNativePrimaryPlaneResourceDevice {
    fn create_mode_blob_for_selection(
        &self,
        _selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        clone_io_result(&self.mode_blob)
    }

    fn add_scanout_framebuffer<B>(
        &self,
        _buffer: &B,
        _depth: u32,
        _bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        clone_io_result(&self.framebuffer)
    }

    fn destroy_scanout_framebuffer(
        &self,
        _framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        clone_io_result(&self.destroy_framebuffer)
    }

    fn destroy_mode_blob(&self, _mode_blob: u64) -> io::Result<()> {
        clone_io_result(&self.destroy_mode_blob)
    }
}

#[derive(Debug)]
struct FakeNativePrimaryPlaneScanoutDevice {
    selection: FakeNativeKmsSelectionDevice,
    properties: FakeNativePropertyLookupDevice,
    resources: FakeNativePrimaryPlaneResourceDevice,
    submit: io::Result<()>,
}

impl LibdrmNativeKmsSelectionDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn connector_handles(&self) -> io::Result<Vec<drm::control::connector::Handle>> {
        self.selection.connector_handles()
    }

    fn crtc_handles(&self) -> io::Result<Vec<drm::control::crtc::Handle>> {
        self.selection.crtc_handles()
    }

    fn connector_snapshot(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativeConnectorSnapshot> {
        self.selection.connector_snapshot(connector)
    }

    fn encoder_snapshot(
        &self,
        encoder: drm::control::encoder::Handle,
    ) -> io::Result<LibdrmNativeEncoderSnapshot> {
        self.selection.encoder_snapshot(encoder)
    }

    fn plane_handles(&self) -> io::Result<Vec<drm::control::plane::Handle>> {
        self.selection.plane_handles()
    }

    fn plane_snapshot(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePlaneSnapshot> {
        self.selection.plane_snapshot(plane)
    }

    fn plane_type(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<Option<drm::control::PlaneType>> {
        self.selection.plane_type(plane)
    }
}

impl LibdrmNativePropertyLookupDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn connector_property_handles(
        &self,
        connector: drm::control::connector::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.connector_property_handles(connector)
    }

    fn crtc_property_handles(
        &self,
        crtc: drm::control::crtc::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.crtc_property_handles(crtc)
    }

    fn plane_property_handles(
        &self,
        plane: drm::control::plane::Handle,
    ) -> io::Result<LibdrmNativePropertyHandleSet> {
        self.properties.plane_property_handles(plane)
    }
}

impl LibdrmNativePrimaryPlaneResourceDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn create_mode_blob_for_selection(
        &self,
        selection: sophia_backend_live::LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<u64> {
        self.resources.create_mode_blob_for_selection(selection)
    }

    fn add_scanout_framebuffer<B>(
        &self,
        buffer: &B,
        depth: u32,
        bpp: u32,
    ) -> io::Result<drm::control::framebuffer::Handle>
    where
        B: drm::buffer::Buffer + ?Sized,
    {
        self.resources.add_scanout_framebuffer(buffer, depth, bpp)
    }

    fn destroy_scanout_framebuffer(
        &self,
        framebuffer: drm::control::framebuffer::Handle,
    ) -> io::Result<()> {
        self.resources.destroy_scanout_framebuffer(framebuffer)
    }

    fn destroy_mode_blob(&self, mode_blob: u64) -> io::Result<()> {
        self.resources.destroy_mode_blob(mode_blob)
    }
}

impl LibdrmNativeAtomicCommitDevice for FakeNativePrimaryPlaneScanoutDevice {
    fn submit_atomic_commit(
        &self,
        _flags: drm::control::AtomicCommitFlags,
        _request: drm::control::atomic::AtomicModeReq,
    ) -> io::Result<()> {
        clone_io_result(&self.submit)
    }
}

fn clone_io_result<T: Clone>(result: &io::Result<T>) -> io::Result<T> {
    result
        .as_ref()
        .cloned()
        .map_err(|error| io::Error::new(error.kind(), "synthetic property lookup failure"))
}

fn property_handle(raw: u32) -> drm::control::property::Handle {
    drm::control::from_u32(raw).expect("test property handle should be nonzero")
}

fn connector_handle() -> drm::control::connector::Handle {
    drm::control::from_u32(11).expect("test connector handle should be nonzero")
}

fn crtc_handle() -> drm::control::crtc::Handle {
    drm::control::from_u32(12).expect("test crtc handle should be nonzero")
}

fn encoder_handle() -> drm::control::encoder::Handle {
    drm::control::from_u32(16).expect("test encoder handle should be nonzero")
}

fn plane_handle() -> drm::control::plane::Handle {
    drm::control::from_u32(13).expect("test plane handle should be nonzero")
}

fn framebuffer_handle() -> drm::control::framebuffer::Handle {
    drm::control::from_u32(14).expect("test framebuffer handle should be nonzero")
}

fn primary_plane_properties() -> LibdrmNativePrimaryPlanePropertyHandles {
    LibdrmNativePrimaryPlanePropertyHandles::new(
        property_handle(101),
        property_handle(102),
        property_handle(103),
        property_handle(104),
        property_handle(105),
        property_handle(106),
        property_handle(107),
        property_handle(108),
        property_handle(109),
        property_handle(110),
        property_handle(111),
        property_handle(112),
        property_handle(113),
    )
}

fn primary_plane_objects(size: Size) -> LibdrmNativePrimaryPlaneObjects {
    LibdrmNativePrimaryPlaneObjects::new(
        connector_handle(),
        crtc_handle(),
        plane_handle(),
        framebuffer_handle(),
        15,
        size,
    )
}

fn full_property_lookup_device() -> FakeNativePropertyLookupDevice {
    FakeNativePropertyLookupDevice {
        connector: Ok(LibdrmNativePropertyHandleSet::new([(
            "CRTC_ID",
            property_handle(101),
        )])),
        crtc: Ok(LibdrmNativePropertyHandleSet::new([
            ("MODE_ID", property_handle(102)),
            ("ACTIVE", property_handle(103)),
        ])),
        plane: Ok(LibdrmNativePropertyHandleSet::new([
            ("FB_ID", property_handle(104)),
            ("CRTC_ID", property_handle(105)),
            ("SRC_X", property_handle(106)),
            ("SRC_Y", property_handle(107)),
            ("SRC_W", property_handle(108)),
            ("SRC_H", property_handle(109)),
            ("CRTC_X", property_handle(110)),
            ("CRTC_Y", property_handle(111)),
            ("CRTC_W", property_handle(112)),
            ("CRTC_H", property_handle(113)),
        ])),
    }
}

fn full_kms_selection_device() -> FakeNativeKmsSelectionDevice {
    FakeNativeKmsSelectionDevice {
        connectors: Ok(vec![connector_handle()]),
        crtcs: Ok(vec![crtc_handle()]),
        planes: Ok(vec![plane_handle()]),
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            Some(encoder_handle()),
            [encoder_handle()],
            Some(Size {
                width: 1280,
                height: 720,
            }),
        )),
        encoder_snapshot: Ok(LibdrmNativeEncoderSnapshot::new(
            Some(crtc_handle()),
            [crtc_handle()],
        )),
        plane_snapshot: Ok(LibdrmNativePlaneSnapshot::new([crtc_handle()])),
        plane_type: Ok(Some(drm::control::PlaneType::Primary)),
    }
}

fn full_primary_plane_resource_device() -> FakeNativePrimaryPlaneResourceDevice {
    FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Ok(15),
        framebuffer: Ok(framebuffer_handle()),
        destroy_framebuffer: Ok(()),
        destroy_mode_blob: Ok(()),
    }
}

fn full_primary_plane_scanout_device() -> FakeNativePrimaryPlaneScanoutDevice {
    FakeNativePrimaryPlaneScanoutDevice {
        selection: full_kms_selection_device(),
        properties: full_property_lookup_device(),
        resources: full_primary_plane_resource_device(),
        submit: Ok(()),
    }
}

fn scanout_descriptor(size: Size) -> sophia_renderer_live::LiveRendererScanoutBufferDescriptor {
    let mut exporter =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(
                size.width as u32 * 4,
                LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
                17,
            );

    exporter
        .export_scanout_buffer(LiveGbmEglFrameTargetRecord::new(size))
        .descriptor
        .expect("ready fake renderer export should include a scanout descriptor")
}

fn scanout_buffer(size: Size) -> LibdrmRendererScanoutBuffer {
    LibdrmRendererScanoutBuffer::from_descriptor(scanout_descriptor(size))
        .expect("ready renderer descriptor should become a backend-private DRM buffer")
}

#[derive(Debug, Eq, PartialEq)]
struct FakeRenderedScanoutOwner {
    raw: u32,
}

struct FakeRenderedScanoutExporter {
    status: LiveRendererScanoutBufferExportStatus,
    descriptor: Option<sophia_renderer_live::LiveRendererScanoutBufferDescriptor>,
    owner: Option<FakeRenderedScanoutOwner>,
    export_attempts: usize,
}

#[cfg(feature = "gbm-probe")]
struct MissingRenderDevice;

#[cfg(feature = "gbm-probe")]
impl RenderDeviceDiscoveryBackend for MissingRenderDevice {
    type Device = std::fs::File;

    fn open_render_device(&self) -> io::Result<Self::Device> {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "test render device unavailable",
        ))
    }
}

impl FakeRenderedScanoutExporter {
    fn exported(size: Size) -> Self {
        Self {
            status: LiveRendererScanoutBufferExportStatus::Exported,
            descriptor: Some(scanout_descriptor(size)),
            owner: Some(FakeRenderedScanoutOwner { raw: 7 }),
            export_attempts: 0,
        }
    }

    fn unavailable() -> Self {
        Self {
            status: LiveRendererScanoutBufferExportStatus::Unavailable,
            descriptor: None,
            owner: None,
            export_attempts: 0,
        }
    }

    fn export_attempts(&self) -> usize {
        self.export_attempts
    }
}

impl LiveRenderedScanoutBufferExporter for FakeRenderedScanoutExporter {
    type Owner = FakeRenderedScanoutOwner;

    fn export_rendered_scanout_buffer(
        &mut self,
        _target: LiveGbmEglFrameTargetRecord,
    ) -> LiveRenderedScanoutBufferExport<Self::Owner> {
        self.export_attempts = self.export_attempts.saturating_add(1);
        LiveRenderedScanoutBufferExport {
            status: self.status,
            descriptor: self.descriptor,
            owner: self.owner.take(),
        }
    }
}

#[test]
fn native_libdrm_atomic_commit_request_reports_reduced_flags() {
    let default_request =
        LibdrmNativeAtomicCommitRequest::new(drm::control::atomic::AtomicModeReq::new());
    assert_eq!(
        default_request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );

    let explicit_request =
        LibdrmNativeAtomicCommitRequest::new(drm::control::atomic::AtomicModeReq::new())
            .without_page_flip_event()
            .blocking()
            .allow_modeset()
            .test_only();
    assert_eq!(
        explicit_request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: false,
            nonblocking: false,
            allow_modeset: true,
            test_only: true,
        }
    );
}

#[test]
fn native_libdrm_primary_plane_property_discovery_feeds_request_builder() {
    let discovery = discover_native_primary_plane_property_handles(
        &full_property_lookup_device(),
        connector_handle(),
        crtc_handle(),
        plane_handle(),
    );

    assert_eq!(
        discovery.status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered
    );
    let properties = discovery
        .properties
        .expect("complete lookup should produce private property handles");
    let build = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        properties,
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    assert!(build.request.is_some());
}

#[test]
fn native_libdrm_primary_plane_property_discovery_fails_closed_for_missing_groups() {
    let missing_connector = FakeNativePropertyLookupDevice {
        connector: Ok(LibdrmNativePropertyHandleSet::new(Vec::<(
            &str,
            drm::control::property::Handle,
        )>::new())),
        ..full_property_lookup_device()
    };
    assert_eq!(
        discover_native_primary_plane_property_handles(
            &missing_connector,
            connector_handle(),
            crtc_handle(),
            plane_handle(),
        )
        .status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingConnectorProperty
    );

    let missing_crtc = FakeNativePropertyLookupDevice {
        crtc: Ok(LibdrmNativePropertyHandleSet::new([(
            "MODE_ID",
            property_handle(102),
        )])),
        ..full_property_lookup_device()
    };
    assert_eq!(
        discover_native_primary_plane_property_handles(
            &missing_crtc,
            connector_handle(),
            crtc_handle(),
            plane_handle(),
        )
        .status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingCrtcProperty
    );

    let missing_plane = FakeNativePropertyLookupDevice {
        plane: Ok(LibdrmNativePropertyHandleSet::new([
            ("FB_ID", property_handle(104)),
            ("CRTC_ID", property_handle(105)),
        ])),
        ..full_property_lookup_device()
    };
    assert_eq!(
        discover_native_primary_plane_property_handles(
            &missing_plane,
            connector_handle(),
            crtc_handle(),
            plane_handle(),
        )
        .status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::MissingPlaneProperty
    );
}

#[test]
fn native_libdrm_primary_plane_property_discovery_fails_closed_on_read_error() {
    let read_failed = FakeNativePropertyLookupDevice {
        connector: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_property_lookup_device()
    };
    let discovery = discover_native_primary_plane_property_handles(
        &read_failed,
        connector_handle(),
        crtc_handle(),
        plane_handle(),
    );

    assert_eq!(
        discovery.status,
        LibdrmNativePrimaryPlanePropertyDiscoveryStatus::ReadFailed
    );
    assert!(discovery.properties.is_none());
}

#[test]
fn native_libdrm_primary_plane_selection_feeds_request_builder() {
    let selection = select_native_primary_plane_target(&full_kms_selection_device());

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    let selected = selection
        .selection
        .expect("complete KMS path should select a private primary plane target");
    assert_eq!(
        selected.size(),
        Size {
            width: 1280,
            height: 720,
        }
    );
    let resource_create = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        resource_create.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let objects = resource_create
        .resources
        .expect("complete resource device should produce framebuffer and mode blob")
        .into_objects(selected);
    let properties = discover_native_primary_plane_property_handles(
        &full_property_lookup_device(),
        connector_handle(),
        crtc_handle(),
        plane_handle(),
    )
    .properties
    .expect("complete property lookup should produce private property handles");
    let build = build_native_primary_plane_atomic_request(objects, properties);

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    assert!(build.request.is_some());
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_chains_renderer_descriptor_to_atomic_commit() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    assert_eq!(
        result.properties,
        Some(LibdrmNativePrimaryPlanePropertyDiscoveryStatus::Discovered)
    );
    assert_eq!(
        result.resources,
        Some(LibdrmNativePrimaryPlaneResourceCreateStatus::Created)
    );
    assert_eq!(
        result.request,
        Some(LibdrmNativeAtomicRequestBuildStatus::Built)
    );
    assert_eq!(
        result.submit,
        Some(LibdrmNativeAtomicCommitSubmitStatus::Submitted)
    );

    let retired = result
        .submission
        .expect("submitted scanout should retain resource ownership until page flip")
        .retire(&device);
    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_uses_supplied_selection_snapshot() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
        &device,
        LibdrmNativePrimaryPlaneSelectionResult {
            status: LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector,
            selection: None,
        },
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::KmsTargetUnavailable
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
    );
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_submit_fails_closed_for_bad_descriptor() {
    let device = full_primary_plane_scanout_device();
    let descriptor = sophia_renderer_live::LiveRendererScanoutBufferDescriptor::new(
        Size {
            width: 1280,
            height: 720,
        },
        0,
        LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888,
        17,
    );
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(&device, descriptor);

    assert_eq!(
        result.status,
        LibdrmNativePrimaryPlaneScanoutSubmitStatus::ScanoutBufferUnavailable
    );
    assert_eq!(
        result.selection,
        LibdrmNativePrimaryPlaneSelectionStatus::Selected
    );
    assert!(result.properties.is_none());
    assert!(result.resources.is_none());
    assert!(result.request.is_none());
    assert!(result.submit.is_none());
    assert!(result.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_retires_only_after_accepted_page_flip() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = result
        .submission
        .expect("submitted scanout should retain resource ownership");

    let retired = retire_native_primary_plane_scanout_after_page_flip(
        &device,
        submission,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::Accepted,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Presented,
                frame_serial: Some(42),
            },
        },
    );

    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(retired.submission.is_none());
}

#[test]
fn native_libdrm_primary_plane_scanout_keeps_submission_until_page_flip_is_accepted() {
    let device = full_primary_plane_scanout_device();
    let result = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = result
        .submission
        .expect("submitted scanout should retain resource ownership");

    let waiting = retire_native_primary_plane_scanout_after_page_flip(
        &device,
        submission,
        &LivePageFlipCallbackReport {
            decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
            event: LivePageFlipEvent {
                status: LivePageFlipEventStatus::Rejected,
                frame_serial: Some(41),
            },
        },
    );

    assert_eq!(
        waiting.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert!(waiting.destroy.is_none());
    let submission = waiting
        .submission
        .expect("rejected page flip must return the in-flight resource owner");
    assert_eq!(
        submission.retire(&device).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn live_runtime_assembly_submits_rendered_primary_plane_scanout_through_reduced_seam() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let mut submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Submitted
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Exported)
    );
    assert_eq!(
        submitted.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
    );
    let submission = submitted
        .submission
        .take()
        .expect("rendered scanout submit should retain both owners");
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        retire_rendered_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    assert_eq!(
        retired.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state(),
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(
        retired.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert!(retired.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_tracks_rendered_scanout_until_accepted_page_flip() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-tracked-retire");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle,
            in_flight: false,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(submitted.in_flight, true);
    assert_eq!(submitted.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Submitted)
    );

    let blocked =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(blocked.in_flight, true);
    assert_eq!(blocked.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    let aged_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should age in-flight scanout ownership");
    assert_eq!(aged_tick.rendered_primary_plane_scanout_in_flight_ticks, 1);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 1);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 1,
            threshold_ticks: 2,
        }
    );

    let stalled_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should classify old in-flight scanout ownership");
    assert_eq!(
        stalled_tick.rendered_primary_plane_scanout_in_flight_ticks,
        2
    );
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::StalledWaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(0),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::WaitingForPageFlip,
            in_flight: true,
            in_flight_ticks: 2,
            threshold_ticks: 0,
        }
    );

    let stale = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(54),
        },
    };
    let waiting =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &stale);

    assert_eq!(
        waiting.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state, None);
    assert_eq!(waiting.in_flight, true);
    assert_eq!(waiting.in_flight_ticks, 2);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        assembly.rendered_primary_plane_scanout_backpressure_report(2),
        LiveRenderedPrimaryPlaneScanoutBackpressureReport {
            status: LiveRenderedPrimaryPlaneScanoutBackpressureStatus::Idle,
            in_flight: false,
            in_flight_ticks: 0,
            threshold_ticks: 2,
        }
    );
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe retired scanout state");

    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Retired]
    );
    assert_eq!(tick.rendered_primary_plane_scanout_in_flight_ticks, 0);
    assert_eq!(tick.engine.runtime.runtime_state.scanout_retirements, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_rejects_page_flip_replay_at_submission_baseline() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-baseline-replay");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let baseline = assembly.observe_page_flip_callback(LivePageFlipCallback {
        output: OutputId::from_raw(1),
        frame_serial: 55,
    });
    assert_eq!(baseline.decision, LivePageFlipCallbackDecision::Accepted);

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    let replay = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let waiting =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &replay);

    assert_eq!(
        waiting.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state, None);
    assert_eq!(waiting.in_flight, true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);

    let newer = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(56),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &newer);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Retired)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_does_not_track_failed_rendered_scanout_submit() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-tracked-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let submitted =
        assembly.submit_and_track_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(submitted.in_flight, false);
    assert_eq!(submitted.in_flight_ticks, 0);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(
        assembly.rendered_primary_plane_runtime_scanout_state(),
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe rejected scanout submit state");

    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected]
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired =
        assembly.retire_tracked_rendered_primary_plane_scanout_after_page_flip(&device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::NoSubmission
    );
    assert_eq!(retired.runtime_scanout_state, None);
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.in_flight_ticks, 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_retains_failed_rendered_scanout_cleanup_for_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(submitted.in_flight, true);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);

    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert_eq!(
        retired.runtime_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(retired.in_flight, false);
    assert_eq!(retired.cleanup_pending, true);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 1);

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe cleanup failure as rejected scanout state");
    assert_eq!(
        tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Rejected]
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, true);

    let mut blocked_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let blocked = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&retry_device, &mut blocked_exporter);
    assert_eq!(
        blocked.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        blocked.runtime_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(blocked.in_flight, false);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);

    assert_eq!(
        cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(
        cleanup.destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed)
    );
    assert_eq!(cleanup.cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());

    let no_cleanup = assembly.retry_tracked_rendered_primary_plane_scanout_cleanup(&retry_device);
    assert_eq!(
        no_cleanup.status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::NoCleanupPending
    );
    assert_eq!(no_cleanup.destroy, None);
    assert_eq!(no_cleanup.cleanup_pending, false);

    let clean_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should observe cleared cleanup state");
    assert_eq!(
        clean_tick.rendered_primary_plane_scanout_cleanup_pending,
        false
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_retries_pending_rendered_scanout_cleanup_before_submit() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-auto-retry");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let retry_device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);
    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &retry_device,
            &mut next_exporter,
        )
        .expect("device-backed tick should retry pending cleanup and submit next scanout");

    assert_eq!(
        tick.rendered_primary_plane_scanout_cleanup_retry
            .expect("pending cleanup should be retried")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanedUp
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, false);
    assert!(!assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("runtime should still submit the next scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_reports_failed_rendered_scanout_cleanup_retry() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-cleanup-auto-retry-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let failing_device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly
        .submit_and_track_rendered_primary_plane_scanout_with(&failing_device, &mut exporter);
    assert_eq!(
        submitted.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    let accepted = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(55),
        },
    };
    let retired = assembly
        .retire_tracked_rendered_primary_plane_scanout_after_page_flip(&failing_device, &accepted);
    assert_eq!(
        retired.status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::ResourceRetireFailed
    );
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());

    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &failing_device,
            &mut next_exporter,
        )
        .expect("device-backed tick should report failed cleanup retry");

    assert_eq!(
        tick.rendered_primary_plane_scanout_cleanup_retry
            .expect("pending cleanup should be retried")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutCleanupStatus::CleanupFailed
    );
    assert_eq!(tick.rendered_primary_plane_scanout_cleanup_pending, true);
    assert!(assembly.rendered_primary_plane_scanout_cleanup_pending());
    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("failed cleanup retry should defer the next submit")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::CleanupPending
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_submits_rendered_scanout_when_runtime_requests_scanout() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-command");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(2);
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 2));
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should use rendered primary-plane submit");

    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_submissions, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Submitted)
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_frame_serial,
        Some(tick.engine.tick.frame_serial)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);
    assert_eq!(tick.rendered_primary_plane_scanout_in_flight_ticks, 0);

    let deferred_tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should defer while previous submit is in flight");

    assert_eq!(
        deferred_tick
            .rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::AlreadyInFlight
    );
    assert_eq!(
        deferred_tick.rendered_primary_plane_scanout_in_flight_ticks,
        1
    );
    assert_eq!(
        deferred_tick
            .rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .in_flight_ticks,
        1
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .scanout_submissions,
        1
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .scanout_rejections,
        0
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .in_flight_scanouts,
        1
    );
    assert_eq!(
        deferred_tick
            .engine
            .runtime
            .runtime_state
            .last_scanout_state,
        Some(RuntimeScanoutState::Deferred)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 1);

    sender
        .try_send(LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 99,
        })
        .expect("test channel should accept page-flip callback");
    let mut next_exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let retire_and_submit_tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut next_exporter,
        )
        .expect("accepted page flip should retire previous submit and allow next submit");

    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_retire
            .expect("accepted page flip should retire in-flight scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip
    );
    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_retire
            .expect("accepted page flip should retire in-flight scanout")
            .in_flight_ticks,
        0
    );
    assert_eq!(
        retire_and_submit_tick.runtime_scanout_states,
        vec![RuntimeScanoutState::Retired]
    );
    assert_eq!(
        retire_and_submit_tick
            .rendered_primary_plane_scanout_submit
            .expect("runtime should submit the next rendered scanout")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
    );
    assert_eq!(
        retire_and_submit_tick
            .engine
            .runtime
            .runtime_state
            .scanout_retirements,
        1
    );
    assert_eq!(
        retire_and_submit_tick
            .engine
            .runtime
            .runtime_state
            .scanout_submissions,
        2
    );
    assert_eq!(
        retire_and_submit_tick
            .engine
            .runtime
            .runtime_state
            .in_flight_scanouts,
        1
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), true);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight_ticks(), 0);
    assert_eq!(
        retire_and_submit_tick.rendered_primary_plane_scanout_in_flight_ticks,
        0
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_rejects_rendered_scanout_when_kms_target_is_not_ready() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-kms-not-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 1280,
        height: 720,
    });
    assert_eq!(
        assembly.kms_scanout_target_observation().status,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should fail closed before export");
    let submit = tick
        .rendered_primary_plane_scanout_submit
        .expect("active scanout submit should be reported");

    assert_eq!(
        submit.status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submit.scanout_target,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );
    assert_eq!(submit.export, None);
    assert_eq!(submit.submit, None);
    assert_eq!(submit.in_flight, false);
    assert_eq!(exporter.export_attempts(), 0);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_direct_rendered_scanout_submit_rejects_not_ready_kms_target() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-direct-kms-not-ready");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });

    assembly.observe_gbm_egl_frame_target_size(Size {
        width: 1280,
        height: 720,
    });

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutTargetNotReady
    );
    assert_eq!(
        submitted.scanout_target,
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
    );
    assert_eq!(submitted.export, None);
    assert_eq!(submitted.submit, None);
    assert!(submitted.submission.is_none());
    assert_eq!(exporter.export_attempts(), 0);
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Rejected
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_tick_defers_rendered_scanout_when_previous_submit_is_in_flight() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-submit-command-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let tick = assembly
        .run_tick_with_rendered_primary_plane_scanout_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("runtime scanout command should fail closed through reduced state");

    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(assembly.pending_runtime_scanout_state_count(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[test]
fn live_runtime_tick_native_gbm_rendered_scanout_fails_closed_when_render_device_is_unavailable() {
    let root = ready_drm_sysfs_fixture("runtime-native-gbm-rendered-primary-plane-unavailable");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);

    let tick = assembly
        .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("native GBM rendered scanout path should fail closed through runtime state");

    assert_eq!(
        tick.rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        tick.engine.runtime.runtime_state.last_scanout_state,
        Some(RuntimeScanoutState::Rejected)
    );
    assert_eq!(tick.engine.runtime.runtime_state.scanout_rejections, 1);
    assert_eq!(assembly.rendered_primary_plane_scanout_in_flight(), false);
    assert_eq!(exporter.export_attempts(), 1);
    assert_eq!(exporter.context_open_attempts(), 1);
    assert_eq!(
        exporter.context_status(),
        Some(NativeGbmRenderedScanoutContextStatus::Unavailable)
    );
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );

    let second_tick = assembly
        .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_with(
            CompositorBackendTickInput::default(),
            &device,
            &mut exporter,
        )
        .expect("reusable native GBM exporter should survive another runtime tick");

    assert_eq!(
        second_tick
            .rendered_primary_plane_scanout_submit
            .expect("active scanout submit should be reported")
            .status,
        LiveTrackedRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(exporter.export_attempts(), 2);
    assert_eq!(exporter.context_open_attempts(), 2);
    assert_eq!(
        exporter.context_status(),
        Some(NativeGbmRenderedScanoutContextStatus::Unavailable)
    );
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(feature = "gbm-probe")]
#[test]
fn native_gbm_rendered_scanout_exporter_rejects_invalid_target_before_device_open() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 0,
        height: 720,
    });

    let export = exporter.export_rendered_scanout_buffer(target);

    assert_eq!(
        export.status,
        LiveRendererScanoutBufferExportStatus::InvalidTarget
    );
    assert_eq!(exporter.export_attempts(), 1);
    assert_eq!(exporter.context_open_attempts(), 0);
    assert_eq!(exporter.context_status(), None);
    assert!(!exporter.context_ready());
    assert_eq!(
        exporter.last_export_status(),
        Some(LiveRendererScanoutBufferExportStatus::InvalidTarget)
    );
    assert_eq!(exporter.last_target(), Some(target));
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Invalidated,
            target,
        })
    );
}

#[cfg(feature = "gbm-probe")]
#[test]
fn native_gbm_rendered_scanout_exporter_tracks_reduced_target_reuse_and_resize() {
    let mut exporter = NativeGbmRenderedScanoutBufferDiscoveryExporter::new(MissingRenderDevice);
    let first = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1280,
        height: 720,
    });
    let resized = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1920,
        height: 1080,
    });

    let first_export = exporter.export_rendered_scanout_buffer(first);
    assert_eq!(
        first_export.status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Created,
            target: first,
        })
    );

    let retained_export = exporter.export_rendered_scanout_buffer(first);
    assert_eq!(
        retained_export.status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Retained,
            target: first,
        })
    );

    let resized_export = exporter.export_rendered_scanout_buffer(resized);
    assert_eq!(
        resized_export.status,
        LiveRendererScanoutBufferExportStatus::Unavailable
    );
    assert_eq!(
        exporter.last_target_lifecycle(),
        Some(LiveGbmEglFrameTargetLifecycleReport {
            status: LiveGbmEglFrameTargetLifecycleStatus::Resized,
            target: resized,
        })
    );
    assert_eq!(exporter.last_target(), Some(resized));
    assert_eq!(exporter.export_attempts(), 3);
    assert_eq!(exporter.context_open_attempts(), 3);
}

#[test]
fn live_runtime_assembly_keeps_rendered_scanout_owner_until_page_flip_is_accepted() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-wait");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::exported(Size {
        width: 1280,
        height: 720,
    });
    let mut submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);
    let submission = submitted
        .submission
        .take()
        .expect("rendered scanout submit should retain both owners");
    let rejected = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::RejectedStaleFrameSerial,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Rejected,
            frame_serial: Some(54),
        },
    };

    let waiting =
        retire_rendered_primary_plane_scanout_after_page_flip(&device, submission, &rejected);

    assert_eq!(
        waiting.status,
        LibdrmNativePrimaryPlaneScanoutRetireStatus::WaitingForAcceptedPageFlip
    );
    assert_eq!(waiting.runtime_scanout_state(), None);
    assert!(waiting.destroy.is_none());
    let owner = waiting
        .submission
        .expect("waiting retirement must keep rendered scanout owner")
        .into_scanout_buffer();
    assert_eq!(owner, FakeRenderedScanoutOwner { raw: 7 });

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_runtime_assembly_fails_rendered_scanout_submit_before_kms_on_export_failure() {
    let root = ready_drm_sysfs_fixture("runtime-rendered-primary-plane-export-fail");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready backend should seed live assembly");
    let device = full_primary_plane_scanout_device();
    let mut exporter = FakeRenderedScanoutExporter::unavailable();

    let submitted = assembly.submit_rendered_primary_plane_scanout_with(&device, &mut exporter);

    assert_eq!(
        submitted.status,
        LiveRenderedPrimaryPlaneScanoutSubmitStatus::ScanoutExportFailed
    );
    assert_eq!(
        submitted.runtime_scanout_state(),
        RuntimeScanoutState::Rejected
    );
    assert_eq!(
        submitted.export,
        Some(LiveRendererScanoutBufferExportStatus::Unavailable)
    );
    assert!(submitted.submit.is_none());
    assert!(submitted.submission.is_none());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_atomic_scanout_smoke_evidence_passes_only_after_submit_page_flip_and_retire() {
    let device = full_primary_plane_scanout_device();
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence,
        LibdrmNativeAtomicScanoutSmokeEvidence {
            status: LibdrmNativeAtomicScanoutSmokeStatus::Passed,
            scanout_target: Some(LiveKmsScanoutTargetStatus::Ready),
            rendered_context: Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            gbm_export: Some(LiveRendererScanoutBufferExportStatus::Exported),
            submit: Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip),
            page_flip_poll: Some(LibdrmPageFlipEventPollStatus::Emitted),
            page_flip: Some(LivePageFlipEventStatus::Presented),
            retire: Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::RetiredAfterPageFlip),
            retire_destroy: Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed),
            retire_cleanup_pending: false,
        }
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_fails_closed_before_page_flip() {
    let device = full_primary_plane_scanout_device();
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        None,
        None,
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::PageFlipMissing
    );
    assert_eq!(
        evidence.submit,
        Some(LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip)
    );
    assert_eq!(
        evidence.page_flip_poll,
        Some(LibdrmPageFlipEventPollStatus::Idle)
    );
    assert_eq!(evidence.retire_destroy, None);
    assert_eq!(evidence.retire_cleanup_pending, false);
    assert_eq!(
        submission.retire(&device).status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_atomic_scanout_smoke_evidence_fails_before_submit_for_not_ready_target() {
    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        None,
        None,
        None,
        None,
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::KmsTargetUnavailable
    );
    assert_eq!(
        evidence.scanout_target,
        Some(LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch)
    );
    assert!(evidence.submit.is_none());
}

#[test]
fn native_atomic_scanout_smoke_evidence_reports_resource_retire_failure() {
    let device = FakeNativePrimaryPlaneScanoutDevice {
        resources: FakeNativePrimaryPlaneResourceDevice {
            destroy_framebuffer: Err(io::Error::other("test framebuffer destroy failed")),
            ..full_primary_plane_resource_device()
        },
        ..full_primary_plane_scanout_device()
    };
    let mut submit = submit_native_primary_plane_scanout_from_renderer_descriptor(
        &device,
        scanout_descriptor(Size {
            width: 1280,
            height: 720,
        }),
    );
    let submission = submit
        .submission
        .take()
        .expect("submitted scanout should retain resource ownership");
    let poll =
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        });
    let callback = LivePageFlipCallbackReport {
        decision: LivePageFlipCallbackDecision::Accepted,
        event: LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(42),
        },
    };
    let retired =
        retire_native_primary_plane_scanout_after_page_flip(&device, submission, &callback);

    let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
        LiveKmsScanoutTargetStatus::Ready,
        Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
        LiveRendererScanoutBufferExportStatus::Exported,
        Some(&submit),
        Some(&poll),
        Some(&callback),
        Some(&retired),
    );

    assert_eq!(
        evidence.status,
        LibdrmNativeAtomicScanoutSmokeStatus::RetireFailed
    );
    assert_eq!(
        evidence.retire,
        Some(LibdrmNativePrimaryPlaneScanoutRetireStatus::ResourceRetireFailed)
    );
    assert_eq!(
        evidence.retire_destroy,
        Some(LibdrmNativePrimaryPlaneResourceDestroyStatus::FramebufferDestroyFailed)
    );
    assert_eq!(evidence.retire_cleanup_pending, true);
}

#[test]
fn native_atomic_scanout_smoke_evidence_records_reduced_early_failures() {
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::no_primary_card().status,
        LibdrmNativeAtomicScanoutSmokeStatus::NoPrimaryCard
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed().status,
        LibdrmNativeAtomicScanoutSmokeStatus::KmsSelectionFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Ready),
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::GbmExportFailed
    );
    assert_eq!(
        LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            LiveKmsScanoutTargetStatus::Ready,
            Some(LibdrmNativeRenderedScanoutContextStatus::Unavailable),
            LiveRendererScanoutBufferExportStatus::Unavailable,
            None,
            None,
            None,
            None,
        )
        .status,
        LibdrmNativeAtomicScanoutSmokeStatus::RenderedContextUnavailable
    );
}

#[test]
fn native_libdrm_primary_plane_resources_validate_size_and_lifetime() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");
    let mismatched = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(Size {
            width: 1920,
            height: 1080,
        }),
    );
    assert_eq!(
        mismatched.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::BufferSizeMismatch
    );
    assert!(mismatched.resources.is_none());

    let created = create_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::Created
    );
    let destroyed = destroy_native_primary_plane_resources(
        &full_primary_plane_resource_device(),
        created
            .resources
            .expect("created resources should be destroyable"),
    );
    assert_eq!(
        destroyed.status,
        LibdrmNativePrimaryPlaneResourceDestroyStatus::Destroyed
    );
}

#[test]
fn native_libdrm_renderer_scanout_buffer_rejects_invalid_renderer_descriptors() {
    let target = LiveGbmEglFrameTargetRecord::new(Size {
        width: 1280,
        height: 720,
    });
    let mut invalid_exporter =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(0, LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888, 17);
    let invalid_descriptor = invalid_exporter.export_scanout_buffer(target).descriptor;
    assert!(invalid_descriptor.is_none());

    let mut unsupported_format =
        FakeRendererScanoutBufferExporter::new(LiveRendererScanoutBufferExportStatus::Exported)
            .with_descriptor(1280 * 4, 0, 17);
    assert!(
        unsupported_format
            .export_scanout_buffer(target)
            .descriptor
            .and_then(LibdrmRendererScanoutBuffer::from_descriptor)
            .is_none()
    );
}

#[test]
fn native_libdrm_primary_plane_resource_creation_fails_closed() {
    let selected = select_native_primary_plane_target(&full_kms_selection_device())
        .selection
        .expect("complete KMS path should select a target");

    let mode_failed = FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &mode_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::ModeBlobCreateFailed
    );
    assert!(created.resources.is_none());

    let mode_missing = FakeNativePrimaryPlaneResourceDevice {
        mode_blob: Err(io::Error::from(io::ErrorKind::InvalidInput)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &mode_missing,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::MissingMode
    );
    assert!(created.resources.is_none());

    let framebuffer_failed = FakeNativePrimaryPlaneResourceDevice {
        framebuffer: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_primary_plane_resource_device()
    };
    let created = create_native_primary_plane_resources(
        &framebuffer_failed,
        selected,
        &scanout_buffer(selected.size()),
    );
    assert_eq!(
        created.status,
        LibdrmNativePrimaryPlaneResourceCreateStatus::FramebufferCreateFailed
    );
    assert!(created.resources.is_none());
}

#[test]
fn native_libdrm_primary_plane_selection_reduces_missing_resource_groups() {
    let disconnected = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            false,
            Some(encoder_handle()),
            [encoder_handle()],
            Some(Size {
                width: 1280,
                height: 720,
            }),
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&disconnected).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoConnectedConnector
    );

    let modeless = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            Some(encoder_handle()),
            [encoder_handle()],
            None,
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&modeless).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoUsableMode
    );

    let no_encoder = FakeNativeKmsSelectionDevice {
        connector_snapshot: Ok(LibdrmNativeConnectorSnapshot::new(
            true,
            None,
            [],
            Some(Size {
                width: 1280,
                height: 720,
            }),
        )),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&no_encoder).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoUsableEncoder
    );

    let incompatible_crtc = FakeNativeKmsSelectionDevice {
        encoder_snapshot: Ok(LibdrmNativeEncoderSnapshot::new(None, [])),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&incompatible_crtc).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatibleCrtc
    );

    let no_primary_plane = FakeNativeKmsSelectionDevice {
        plane_type: Ok(Some(drm::control::PlaneType::Overlay)),
        ..full_kms_selection_device()
    };
    assert_eq!(
        select_native_primary_plane_target(&no_primary_plane).status,
        LibdrmNativePrimaryPlaneSelectionStatus::NoCompatiblePrimaryPlane
    );
}

#[test]
fn native_libdrm_primary_plane_selection_fails_closed_on_read_error() {
    let read_failed = FakeNativeKmsSelectionDevice {
        connectors: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_kms_selection_device()
    };
    let selection = select_native_primary_plane_target(&read_failed);

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed
    );
    assert!(selection.selection.is_none());

    let plane_read_failed = FakeNativeKmsSelectionDevice {
        plane_snapshot: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ..full_kms_selection_device()
    };
    let selection = select_native_primary_plane_target(&plane_read_failed);

    assert_eq!(
        selection.status,
        LibdrmNativePrimaryPlaneSelectionStatus::ReadFailed
    );
    assert!(selection.selection.is_none());
}

#[test]
fn native_libdrm_primary_plane_builder_creates_submit_ready_request() {
    let build = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: 720,
        }),
        primary_plane_properties(),
    );

    assert_eq!(build.status, LibdrmNativeAtomicRequestBuildStatus::Built);
    let request = build.request.expect("valid objects should build request");
    assert_eq!(
        request.reduced_flags(),
        LibdrmNativeAtomicCommitFlagsReport {
            page_flip_event: true,
            nonblocking: true,
            allow_modeset: false,
            test_only: false,
        }
    );

    let mut committer =
        NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice { result: Ok(()) });
    assert_eq!(
        committer.submit_native_atomic_commit(request),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        }
    );
}

#[test]
fn native_libdrm_primary_plane_builder_rejects_invalid_size() {
    let zero_width = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 0,
            height: 720,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        zero_width.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(zero_width.request.is_none());

    let negative_height = build_native_primary_plane_atomic_request(
        primary_plane_objects(Size {
            width: 1280,
            height: -1,
        }),
        primary_plane_properties(),
    );
    assert_eq!(
        negative_height.status,
        LibdrmNativeAtomicRequestBuildStatus::InvalidSize
    );
    assert!(negative_height.request.is_none());
}

#[test]
fn native_libdrm_atomic_committer_reduces_submit_results() {
    let mut committer =
        NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice { result: Ok(()) });
    assert_eq!(
        committer.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Submitted,
        }
    );
    assert_eq!(committer.submitted_count(), 1);
    assert_eq!(committer.rejected_count(), 0);

    let mut would_block = NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice {
        result: Err(io::Error::from(io::ErrorKind::WouldBlock)),
    });
    assert_eq!(
        would_block.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::WouldBlock,
        }
    );
    assert_eq!(would_block.submitted_count(), 0);
    assert_eq!(would_block.rejected_count(), 0);

    let mut rejected = NativeLibdrmAtomicScanoutCommitter::new(FakeNativeAtomicCommitDevice {
        result: Err(io::Error::from(io::ErrorKind::PermissionDenied)),
    });
    assert_eq!(
        rejected.submit_native_atomic_commit(LibdrmNativeAtomicCommitRequest::new(
            drm::control::atomic::AtomicModeReq::new()
        )),
        LibdrmNativeAtomicCommitSubmitReport {
            status: LibdrmNativeAtomicCommitSubmitStatus::Rejected,
        }
    );
    assert_eq!(rejected.submitted_count(), 0);
    assert_eq!(rejected.rejected_count(), 1);
}

#[test]
fn native_libdrm_read_loop_result_maps_to_reduced_poll_report() {
    assert_eq!(
        LibdrmNativeReadLoopReport::idle().into_poll_report().status,
        LibdrmPageFlipEventPollStatus::Idle
    );
    assert_eq!(
        LibdrmNativeReadLoopReport::would_block()
            .into_poll_report()
            .status,
        LibdrmPageFlipEventPollStatus::Idle
    );

    let decoded =
        LibdrmNativeReadLoopReport::callback_decoded(3).expect("decoded count must be nonzero");
    assert_eq!(decoded.status, LibdrmNativeReadLoopStatus::CallbackDecoded);
    assert_eq!(decoded.into_poll_report().callbacks.emitted, 3);
    assert_eq!(
        decoded.into_poll_report().status,
        LibdrmPageFlipEventPollStatus::Emitted
    );

    assert_eq!(LibdrmNativeReadLoopReport::callback_decoded(0), None);
    let rejected =
        LibdrmNativeReadLoopReport::callbacks_decoded(0, 2).expect("rejection count is observable");
    assert_eq!(
        rejected.status,
        LibdrmNativeReadLoopStatus::CallbackRejected
    );
    assert_eq!(rejected.decoded_callbacks, 0);
    assert_eq!(rejected.rejected_callbacks, 2);
    assert_eq!(
        rejected.into_poll_report().status,
        LibdrmPageFlipEventPollStatus::Idle
    );

    let mixed = LibdrmNativeReadLoopReport::callbacks_decoded(2, 1)
        .expect("decoded or rejected counts should produce a report");
    assert_eq!(mixed.status, LibdrmNativeReadLoopStatus::CallbackDecoded);
    assert_eq!(mixed.decoded_callbacks, 2);
    assert_eq!(mixed.rejected_callbacks, 1);
    assert_eq!(mixed.into_poll_report().callbacks.emitted, 2);

    assert_eq!(LibdrmNativeReadLoopReport::callbacks_decoded(0, 0), None);
    assert_eq!(
        LibdrmNativeReadLoopReport::read_failed()
            .into_poll_report()
            .status,
        LibdrmPageFlipEventPollStatus::Disconnected
    );
}

#[test]
fn native_libdrm_poller_skeleton_reports_idle_without_emitting_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(14).expect("nonzero generation should mint authority token");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller = NativeLibdrmPageFlipEventPoller::new(source);
    let (sender, receiver) = mpsc::sync_channel(1);

    assert_eq!(
        poller.source_report(),
        LibdrmNativePageFlipSourceReport {
            status: LibdrmNativePageFlipSourceStatus::ConstructedWithoutPolling,
        }
    );
    assert_eq!(
        poller.poll_page_flip_events(&sender, 4).status,
        LibdrmPageFlipEventPollStatus::Idle
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_libdrm_poller_drains_injected_callback_batch_without_fd_polling() {
    let authority =
        LibdrmBackendFdAuthority::new(15).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(4);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);
    assert_eq!(poller.pending_callback_count(), 3);

    let report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(report.callbacks.emitted, 2);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        poller.last_read_loop_report().status,
        LibdrmNativeReadLoopStatus::CallbackDecoded
    );
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 2);
    assert_eq!(poller.last_read_loop_report().rejected_callbacks, 1);
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("second callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 82,
        }
    );
}

#[test]
fn native_libdrm_reader_reads_bounded_callbacks_without_kms_identity() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let mut reader = FakeLibdrmNativePageFlipReader::new([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);

    let first = reader.read_ready_page_flip_callbacks(1);
    assert_eq!(
        first,
        LibdrmNativePageFlipReadResult {
            report: LibdrmNativeReadLoopReport::callback_decoded(1)
                .expect("one callback should produce a read report"),
            callbacks: vec![LibdrmNativePageFlipCallback::new(slot, 81)],
        }
    );
    assert_eq!(reader.queued_len(), 1);

    let second = reader.read_ready_page_flip_callbacks(4);
    assert_eq!(second.report.decoded_callbacks, 1);
    assert_eq!(
        second.callbacks,
        vec![LibdrmNativePageFlipCallback::new(slot, 82)]
    );
    assert_eq!(reader.queued_len(), 0);

    let empty = reader.read_ready_page_flip_callbacks(4);
    assert_eq!(empty.report, LibdrmNativeReadLoopReport::would_block());
    assert!(empty.callbacks.is_empty());
}

#[test]
fn native_libdrm_page_flip_event_reducer_uses_private_crtc_routes() {
    let crtc = drm::control::from_u32::<drm::control::crtc::Handle>(44)
        .expect("nonzero crtc handle should be constructible");
    let other_crtc = drm::control::from_u32::<drm::control::crtc::Handle>(45)
        .expect("nonzero crtc handle should be constructible");
    let slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let event = drm::control::PageFlipEvent {
        frame: 91,
        duration: std::time::Duration::from_millis(16),
        crtc,
    };

    assert_eq!(
        reduce_native_page_flip_event(&event, &[LibdrmNativeCrtcRoute::new(crtc, slot)]),
        Some(LibdrmNativePageFlipCallback::new(slot, 91))
    );
    assert_eq!(
        reduce_native_page_flip_event(&event, &[LibdrmNativeCrtcRoute::new(other_crtc, slot)]),
        None
    );
}

#[test]
fn native_libdrm_page_flip_event_reader_owns_device_and_private_crtc_routes() {
    let crtc = drm::control::from_u32::<drm::control::crtc::Handle>(44)
        .expect("nonzero crtc handle should be constructible");
    let slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let reader = NativeLibdrmPageFlipEventReader::new(())
        .with_crtc_routes([LibdrmNativeCrtcRoute::new(crtc, slot)]);

    assert_eq!(reader.crtc_route_count(), 1);
}

#[test]
fn native_libdrm_poller_reads_and_polls_bounded_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(24).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let mut reader = FakeLibdrmNativePageFlipReader::new([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);
    let (sender, receiver) = mpsc::sync_channel(4);

    let report = poller.read_and_poll_page_flip_events(&mut reader, &sender, 1, 4);

    assert_eq!(
        report,
        LibdrmNativeReadAndPollReport {
            read_loop: LibdrmNativeReadLoopReport::callback_decoded(1)
                .expect("one callback should produce a read report"),
            poll: LibdrmPageFlipEventPollReport {
                status: LibdrmPageFlipEventPollStatus::Emitted,
                callbacks: LivePageFlipCallbackSourceReport {
                    emitted: 1,
                    queued_remaining: 0,
                    backpressure: false,
                    disconnected: false,
                    max_reached: false,
                },
            },
        }
    );
    assert_eq!(reader.queued_len(), 1);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        receiver
            .try_recv()
            .expect("callback should be reduced and queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
}

#[test]
fn native_libdrm_poller_reports_read_failure_without_dropping_pending_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(25).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let mut reader =
        FakeLibdrmNativePageFlipReader::new([LibdrmNativePageFlipCallback::new(slot, 81)]);
    reader.fail_next_read();
    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(slot, 80)]);
    let (sender, receiver) = mpsc::sync_channel(4);

    let report = poller.read_and_poll_page_flip_events(&mut reader, &sender, 4, 4);

    assert_eq!(report.read_loop, LibdrmNativeReadLoopReport::read_failed());
    assert_eq!(
        report.poll.status,
        LibdrmPageFlipEventPollStatus::Disconnected
    );
    assert_eq!(poller.pending_callback_count(), 1);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_libdrm_poller_retains_injected_callbacks_on_backpressure() {
    let authority =
        LibdrmBackendFdAuthority::new(16).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(1);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);

    let first = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(first.status, LibdrmPageFlipEventPollStatus::Backpressure);
    assert_eq!(first.callbacks.emitted, 1);
    assert_eq!(first.callbacks.queued_remaining, 1);
    assert_eq!(poller.pending_callback_count(), 1);
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 2);
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );

    let second = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(second.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(second.callbacks.emitted, 1);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 1);
    assert_eq!(
        receiver
            .try_recv()
            .expect("retained callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 82,
        }
    );
}

#[test]
fn native_libdrm_poller_retains_injected_callbacks_on_disconnected_queue() {
    let authority =
        LibdrmBackendFdAuthority::new(17).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(1);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ]);
    drop(receiver);

    let report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Disconnected);
    assert_eq!(report.callbacks.emitted, 0);
    assert_eq!(report.callbacks.queued_remaining, 2);
    assert_eq!(poller.pending_callback_count(), 2);
    assert_eq!(poller.last_read_loop_report().decoded_callbacks, 1);
}

#[test]
fn native_libdrm_poller_replaces_routes_without_dropping_pending_callbacks() {
    let authority =
        LibdrmBackendFdAuthority::new(18).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(slot, 81)]);
    poller.replace_routes([LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(9),
    }]);

    assert_eq!(poller.route_count(), 1);
    assert_eq!(poller.pending_callback_count(), 1);

    let report = poller.poll_page_flip_events(&sender, 2);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        receiver
            .try_recv()
            .expect("callback should use replaced route"),
        LivePageFlipCallback {
            output: OutputId::from_raw(9),
            frame_serial: 81,
        }
    );
}

#[test]
fn native_libdrm_poller_rejects_pending_callbacks_after_route_removal() {
    let authority =
        LibdrmBackendFdAuthority::new(19).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(slot, 81)]);
    poller.replace_routes([]);

    let report = poller.poll_page_flip_events(&sender, 2);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Idle);
    assert_eq!(report.callbacks.emitted, 0);
    assert_eq!(poller.pending_callback_count(), 0);
    assert_eq!(
        poller.last_read_loop_report().status,
        LibdrmNativeReadLoopStatus::CallbackRejected
    );
    assert_eq!(poller.last_read_loop_report().rejected_callbacks, 1);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_libdrm_poller_diagnostics_report_only_reduced_counts() {
    let authority =
        LibdrmBackendFdAuthority::new(20).expect("nonzero generation should mint authority token");
    let first_slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let second_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot: first_slot,
            output: OutputId::from_raw(7),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(first_slot, 81),
        LibdrmNativePageFlipCallback::new(second_slot, 82),
    ]);

    assert_eq!(
        poller.diagnostics(),
        LibdrmNativePollerDiagnostics {
            route_count: 1,
            pending_callbacks: 2,
            last_read_loop: LibdrmNativeReadLoopReport::idle(),
        }
    );

    poller.replace_routes([
        LibdrmNativeOutputRoute {
            slot: first_slot,
            output: OutputId::from_raw(7),
        },
        LibdrmNativeOutputRoute {
            slot: second_slot,
            output: OutputId::from_raw(9),
        },
    ]);
    let report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(
        poller.diagnostics(),
        LibdrmNativePollerDiagnostics {
            route_count: 2,
            pending_callbacks: 0,
            last_read_loop: LibdrmNativeReadLoopReport::callback_decoded(2)
                .expect("decoded count should build a report"),
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("second callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(9),
            frame_serial: 82,
        }
    );
}

#[test]
fn live_runtime_assembly_reports_reduced_native_libdrm_poller_diagnostics() {
    let root = ready_drm_sysfs_fixture("native-libdrm-runtime-diagnostics");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(21).expect("nonzero generation should mint authority token");
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let source = LibdrmNativePageFlipSource::from_authority(authority);
    let mut poller =
        NativeLibdrmPageFlipEventPoller::new(source).with_routes([LibdrmNativeOutputRoute {
            slot,
            output: OutputId::from_raw(1),
        }]);
    let (sender, receiver) = mpsc::sync_channel(2);

    poller.inject_callbacks([
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
    ]);
    let poll_report = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll_report.status, LibdrmPageFlipEventPollStatus::Emitted);

    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_native_libdrm_poller_diagnostics(poller.diagnostics())
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));

    assert_eq!(
        assembly.libdrm_poller_diagnostics(),
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::CallbackDecoded,
            route_count: 1,
            pending_callbacks: 0,
            decoded_callbacks: 1,
            rejected_callbacks: 1,
        }
    );

    let tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain callback and report diagnostics");

    assert_eq!(
        tick.libdrm_poller,
        LiveLibdrmPollerDiagnostics {
            status: LiveLibdrmPollerDiagnosticsStatus::CallbackDecoded,
            route_count: 1,
            pending_callbacks: 0,
            decoded_callbacks: 1,
            rejected_callbacks: 1,
        }
    );
    assert_eq!(
        tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(81),
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_poller_constructs_routes_from_discovered_outputs_without_kms_identity() {
    let root = multi_output_drm_sysfs_fixture("native-libdrm-discovered-routes");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(22).expect("nonzero generation should mint authority token");
    let routes = report.native_libdrm_output_routes();

    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].slot.raw(), 1);
    assert_eq!(routes[0].output, OutputId::from_raw(1));
    assert_eq!(routes[1].slot.raw(), 2);
    assert_eq!(routes[1].output, OutputId::from_raw(2));
    assert_eq!(
        report.native_libdrm_poller_startup_report(),
        LiveLibdrmPollerStartupReport {
            status: LiveLibdrmPollerStartupStatus::Ready,
            route_count: 2,
        }
    );

    let mut poller = report
        .native_libdrm_poller_from_authority(authority)
        .expect("ready discovery should construct native poller");
    let (sender, receiver) = mpsc::sync_channel(2);

    assert_eq!(poller.diagnostics().route_count, 2);
    poller.inject_callbacks([LibdrmNativePageFlipCallback::new(routes[1].slot, 90)]);

    let poll_report = poller.poll_page_flip_events(&sender, 4);

    assert_eq!(poll_report.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(
        receiver
            .try_recv()
            .expect("callback should map through reduced output route"),
        LivePageFlipCallback {
            output: OutputId::from_raw(2),
            frame_serial: 90,
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_poller_construction_fails_closed_without_outputs() {
    let root = std::env::temp_dir().join("sophia-backend-live-native-libdrm-no-routes");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let authority =
        LibdrmBackendFdAuthority::new(23).expect("nonzero generation should mint authority token");

    assert!(report.native_libdrm_output_routes().is_empty());
    assert_eq!(
        report.native_libdrm_poller_startup_report(),
        LiveLibdrmPollerStartupReport {
            status: LiveLibdrmPollerStartupStatus::NoOutputs,
            route_count: 0,
        }
    );
    assert!(
        report
            .native_libdrm_poller_from_authority(authority)
            .is_none()
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_libdrm_page_flip_callback_decodes_without_native_resource_identity() {
    assert_eq!(LibdrmNativeOutputSlot::new(0), None);
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    assert_eq!(slot.raw(), 2);

    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
    }];
    let callback = LibdrmNativePageFlipCallback::new(slot, 81);

    assert_eq!(
        callback.decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::Decoded,
            callback: Some(LivePageFlipCallback {
                output: OutputId::from_raw(7),
                frame_serial: 81,
            }),
        }
    );

    let unknown_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    assert_eq!(
        LibdrmNativePageFlipCallback::new(unknown_slot, 82).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::UnknownOutputSlot,
            callback: None,
        }
    );
    assert_eq!(
        LibdrmNativePageFlipCallback::new(slot, 0).decode(&routes),
        LibdrmNativePageFlipDecodeReport {
            status: LibdrmNativePageFlipDecodeStatus::InvalidFrameSerial,
            callback: None,
        }
    );
}

#[test]
fn native_libdrm_page_flip_decode_batch_is_bounded_and_reduced() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let unknown_slot = LibdrmNativeOutputSlot::new(3).expect("nonzero slot should be valid");
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
    }];
    let callbacks = [
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 0),
        LibdrmNativePageFlipCallback::new(unknown_slot, 82),
        LibdrmNativePageFlipCallback::new(slot, 83),
    ];
    let (sender, receiver) = mpsc::sync_channel(4);

    let report = decode_native_page_flip_batch(&callbacks, &routes, &sender, 4);

    assert_eq!(
        report.read_loop.status,
        LibdrmNativeReadLoopStatus::CallbackDecoded
    );
    assert_eq!(report.read_loop.decoded_callbacks, 2);
    assert_eq!(report.read_loop.rejected_callbacks, 2);
    assert_eq!(report.poll.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(report.poll.callbacks.emitted, 2);
    assert_eq!(
        receiver
            .try_recv()
            .expect("first callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 81,
        }
    );
    assert_eq!(
        receiver
            .try_recv()
            .expect("second callback should be queued"),
        LivePageFlipCallback {
            output: OutputId::from_raw(7),
            frame_serial: 83,
        }
    );
    assert!(receiver.try_recv().is_err());

    let (sender, _receiver) = mpsc::sync_channel(4);
    let limited = decode_native_page_flip_batch(&callbacks, &routes, &sender, 1);
    assert_eq!(limited.read_loop.decoded_callbacks, 1);
    assert_eq!(limited.poll.callbacks.emitted, 1);
    assert_eq!(limited.poll.callbacks.max_reached, true);
    assert_eq!(limited.poll.callbacks.queued_remaining, 3);
}

#[test]
fn native_libdrm_page_flip_decode_batch_reports_backpressure_without_native_identity() {
    let slot = LibdrmNativeOutputSlot::new(2).expect("nonzero slot should be valid");
    let routes = [LibdrmNativeOutputRoute {
        slot,
        output: OutputId::from_raw(7),
    }];
    let callbacks = [
        LibdrmNativePageFlipCallback::new(slot, 81),
        LibdrmNativePageFlipCallback::new(slot, 82),
    ];
    let (sender, _receiver) = mpsc::sync_channel(1);

    let report = decode_native_page_flip_batch(&callbacks, &routes, &sender, 4);

    assert_eq!(report.read_loop.decoded_callbacks, 2);
    assert_eq!(report.read_loop.rejected_callbacks, 0);
    assert_eq!(
        report.poll.status,
        LibdrmPageFlipEventPollStatus::Backpressure
    );
    assert_eq!(report.poll.callbacks.emitted, 1);
    assert_eq!(report.poll.callbacks.queued_remaining, 1);
}

#[test]
fn libdrm_event_poll_report_projects_source_state_without_native_identity() {
    assert_eq!(
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 0,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        })
        .status,
        LibdrmPageFlipEventPollStatus::Idle
    );

    assert_eq!(
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 2,
            queued_remaining: 0,
            backpressure: false,
            disconnected: false,
            max_reached: false,
        })
        .status,
        LibdrmPageFlipEventPollStatus::Emitted
    );

    assert_eq!(
        LibdrmPageFlipEventPollReport::from_source_report(LivePageFlipCallbackSourceReport {
            emitted: 1,
            queued_remaining: 1,
            backpressure: true,
            disconnected: false,
            max_reached: false,
        })
        .status,
        LibdrmPageFlipEventPollStatus::Backpressure
    );
}

#[test]
fn fake_libdrm_page_flip_poller_feeds_runtime_queue() {
    let root = ready_drm_sysfs_fixture("fake-libdrm-page-flip-poller");
    let report = discover_live_backend(&LiveBackendConfig::new(&root));
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut poller = FakeLibdrmPageFlipEventPoller::new([
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 61,
        },
        LivePageFlipCallback {
            output: OutputId::from_raw(1),
            frame_serial: 62,
        },
    ]);

    let poll = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll.status, LibdrmPageFlipEventPollStatus::Backpressure);
    assert_eq!(poll.callbacks.emitted, 1);
    assert_eq!(poller.queued_len(), 1);

    let mut assembly = report
        .into_live_runtime_assembly(QueuedInputPoller::default())
        .expect("ready startup should seed live assembly")
        .with_page_flip_callback_queue(LivePageFlipCallbackQueue::new(receiver, 4));
    let first_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain first callback");
    assert_eq!(
        first_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(61),
        }
    );

    let poll = poller.poll_page_flip_events(&sender, 4);
    assert_eq!(poll.status, LibdrmPageFlipEventPollStatus::Emitted);
    assert_eq!(poller.queued_len(), 0);
    let second_tick = assembly
        .run_tick(CompositorBackendTickInput::default())
        .expect("runtime tick should drain second callback");
    assert_eq!(
        second_tick.page_flip,
        LivePageFlipEvent {
            status: LivePageFlipEventStatus::Presented,
            frame_serial: Some(62),
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

fn ready_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let connector = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&connector).unwrap();
    write_fixture_file(&connector, "status", "connected\n");
    write_fixture_file(&connector, "modes", "1920x1080\n");
    write_fixture_file(&connector, "connector_id", "42\n");
    write_fixture_file(&connector, "crtc_id", "99\n");
    root
}

fn multi_output_drm_sysfs_fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sophia-backend-live-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let first = root.join("card0-DP-1");
    let second = root.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    write_fixture_file(&first, "status", "connected\n");
    write_fixture_file(&first, "modes", "1920x1080\n");
    write_fixture_file(&first, "connector_id", "1234\n");
    write_fixture_file(&first, "crtc_id", "2234\n");
    write_fixture_file(&second, "status", "connected\n");
    write_fixture_file(&second, "modes", "2560x1440\n");
    write_fixture_file(&second, "connector_id", "9876\n");
    write_fixture_file(&second, "crtc_id", "8876\n");
    root
}

fn write_fixture_file(root: &std::path::Path, name: &str, contents: &str) {
    std::fs::write(root.join(name), contents).unwrap();
}

#[cfg(feature = "gbm-probe")]
mod atomic_scanout_hardware_smoke {
    use std::os::fd::{AsFd, BorrowedFd};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::*;
    use sophia_backend_live::LivePageFlipCallbackIntake;
    use sophia_renderer_live::LiveRendererScanoutBufferExportStatus;

    #[derive(Debug)]
    struct RealDrmCard(std::fs::File);

    impl RealDrmCard {
        fn open(path: &Path) -> io::Result<Self> {
            Ok(Self(
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)?,
            ))
        }

        fn try_clone(&self) -> io::Result<Self> {
            Ok(Self(self.0.try_clone()?))
        }

        fn try_clone_file(&self) -> io::Result<std::fs::File> {
            self.0.try_clone()
        }
    }

    impl AsFd for RealDrmCard {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.0.as_fd()
        }
    }

    impl drm::Device for RealDrmCard {}
    impl drm::control::Device for RealDrmCard {}

    #[test]
    fn native_atomic_scanout_smokes_real_primary_card_when_enabled() {
        if std::env::var_os("SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE").is_none() {
            return;
        }

        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("atomic_scanout_hardware_smoke::native_atomic_scanout_real_primary_card_child")
            .arg("--nocapture")
            .env("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD", "1")
            .spawn()
            .expect("real atomic scanout smoke child should start");
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            if let Some(status) = child
                .try_wait()
                .expect("real atomic scanout smoke child should be waitable")
            {
                assert!(
                    status.success(),
                    "real atomic scanout smoke child failed with status {status}"
                );
                return;
            }

            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("real atomic scanout smoke child timed out waiting for page-flip evidence");
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn native_atomic_scanout_real_primary_card_child() {
        if std::env::var_os("SOPHIA_REAL_ATOMIC_SCANOUT_CHILD").is_none() {
            return;
        }

        let Some(card_path) = first_openable_primary_card_node() else {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::no_primary_card();
            println!("{evidence:?}");
            assert_eq!(
                evidence.status,
                LibdrmNativeAtomicScanoutSmokeStatus::Passed
            );
            return;
        };
        let card = RealDrmCard::open(&card_path).expect("primary DRM card should open read/write");

        drm::Device::set_client_capability(&card, drm::ClientCapability::UniversalPlanes, true)
            .expect("primary DRM card should accept UniversalPlanes client capability");
        drm::Device::set_client_capability(&card, drm::ClientCapability::Atomic, true)
            .expect("primary DRM card should accept Atomic client capability");

        let selection = select_native_primary_plane_target(&card);
        if selection.status != LibdrmNativePrimaryPlaneSelectionStatus::Selected {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed();
            println!("{evidence:?}");
            assert_eq!(
                evidence.status,
                LibdrmNativeAtomicScanoutSmokeStatus::Passed
            );
        }
        let selected = selection
            .selection
            .expect("real KMS target selection should produce primary-plane target");
        let slot = LibdrmNativeOutputSlot::new(1).expect("slot one should be valid");
        let output = OutputId::from_raw(1);
        let target = LiveGbmEglFrameTargetRecord::new(selected.size());
        let scanout_target = if target.status != LiveGbmEglFrameTargetStatus::Ready {
            LiveKmsScanoutTargetStatus::InvalidFrameTarget
        } else if target.size != selected.size() {
            LiveKmsScanoutTargetStatus::FrameTargetSizeMismatch
        } else {
            LiveKmsScanoutTargetStatus::Ready
        };
        if scanout_target != LiveKmsScanoutTargetStatus::Ready {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                None,
                LiveRendererScanoutBufferExportStatus::Unavailable,
                None,
                None,
                None,
                None,
            );
            println!("{evidence:?}");
            assert_eq!(
                evidence.status,
                LibdrmNativeAtomicScanoutSmokeStatus::Passed
            );
            return;
        }

        let context_report =
            NativeGbmRenderedScanoutContext::from_backend_device_result(card.try_clone_file());
        let rendered_context = match context_report.status {
            NativeGbmRenderedScanoutContextStatus::Ready => {
                LibdrmNativeRenderedScanoutContextStatus::Ready
            }
            NativeGbmRenderedScanoutContextStatus::Unavailable => {
                LibdrmNativeRenderedScanoutContextStatus::Unavailable
            }
            NativeGbmRenderedScanoutContextStatus::Degraded => {
                LibdrmNativeRenderedScanoutContextStatus::Degraded
            }
        };
        let Some(context) = context_report.context else {
            let export_status = match context_report.status {
                NativeGbmRenderedScanoutContextStatus::Ready => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
                NativeGbmRenderedScanoutContextStatus::Unavailable => {
                    LiveRendererScanoutBufferExportStatus::Unavailable
                }
                NativeGbmRenderedScanoutContextStatus::Degraded => {
                    LiveRendererScanoutBufferExportStatus::Degraded
                }
            };
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                Some(rendered_context),
                export_status,
                None,
                None,
                None,
                None,
            );
            println!("{evidence:?}");
            assert_eq!(
                evidence.status,
                LibdrmNativeAtomicScanoutSmokeStatus::Passed
            );
            return;
        };

        let export = context.export_rendered_owned_scanout_buffer(target);
        if export.status != LiveRendererScanoutBufferExportStatus::Exported {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                Some(rendered_context),
                export.status,
                None,
                None,
                None,
                None,
            );
            println!("{evidence:?}");
            assert_eq!(
                evidence.status,
                LibdrmNativeAtomicScanoutSmokeStatus::Passed
            );
        }
        let owned_buffer = export
            .buffer
            .expect("real GBM scanout export should retain owned buffer");

        let mut submit = submit_native_primary_plane_scanout_from_selection_and_renderer_descriptor(
            &card,
            LibdrmNativePrimaryPlaneSelectionResult {
                status: selection.status,
                selection: Some(selected),
            },
            owned_buffer.descriptor(),
        );
        if submit.status != LibdrmNativePrimaryPlaneScanoutSubmitStatus::SubmittedWaitingForPageFlip
        {
            let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
                scanout_target,
                Some(rendered_context),
                export.status,
                Some(&submit),
                None,
                None,
                None,
            );
            println!("{evidence:?}");
            assert_eq!(
                evidence.status,
                LibdrmNativeAtomicScanoutSmokeStatus::Passed
            );
        }
        let submission = submit
            .submission
            .take()
            .expect("submitted scanout should retain resources");

        let mut reader = NativeLibdrmPageFlipEventReader::new(
            card.try_clone()
                .expect("page-flip reader should clone the DRM card fd"),
        )
        .with_crtc_routes([selected.crtc_route(slot)]);
        let source = LibdrmNativePageFlipSource::from_authority(
            LibdrmBackendFdAuthority::new(1).expect("nonzero authority generation should mint"),
        );
        let mut poller = NativeLibdrmPageFlipEventPoller::new(source)
            .with_routes([LibdrmNativeOutputRoute { slot, output }]);
        let (sender, receiver) = mpsc::sync_channel(1);
        let poll = poller.read_and_poll_page_flip_events(&mut reader, &sender, 1, 1);
        let callback = receiver.try_recv().ok();
        let mut intake = LivePageFlipCallbackIntake::new(output);
        let mut submission = Some(submission);
        let mut callback_report = None;
        let mut retired = None;
        if let Some(callback) = callback {
            let report = intake.observe(callback);
            retired = Some(retire_native_primary_plane_scanout_after_page_flip(
                &card,
                submission
                    .take()
                    .expect("callback path should still own submitted resources"),
                &report,
            ));
            callback_report = Some(report);
        }

        let evidence = LibdrmNativeAtomicScanoutSmokeEvidence::from_pipeline_reports(
            scanout_target,
            Some(rendered_context),
            export.status,
            Some(&submit),
            Some(&poll.poll),
            callback_report.as_ref(),
            retired.as_ref(),
        );
        println!("{evidence:?}");
        assert_eq!(
            evidence.status,
            LibdrmNativeAtomicScanoutSmokeStatus::Passed
        );
        drop(owned_buffer);
    }

    fn first_openable_primary_card_node() -> Option<PathBuf> {
        let entries = std::fs::read_dir("/dev/dri").ok()?;
        let mut candidates = Vec::new();

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.starts_with("card") {
                candidates.push(entry.path());
            }
        }

        candidates.sort();
        candidates
            .into_iter()
            .find(|path| RealDrmCard::open(path).is_ok())
    }
}

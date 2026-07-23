#[cfg(feature = "gbm-probe")]
use super::RealAtomicScanoutRenderDeviceDiscovery;
use super::{
    RealAtomicScanoutCard, RealAtomicScanoutCardSelection, RealAtomicScanoutCardSelectionStatus,
};
use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSession {
    pub(super) card: RealAtomicScanoutCard,
    selections: Vec<LibdrmNativePrimaryPlaneSelection>,
    outputs: Vec<OutputId>,
    pub(super) reader: NativeLibdrmPageFlipEventReader<RealAtomicScanoutCard>,
    pub(super) poller: NativeLibdrmPageFlipEventPoller,
    #[cfg(feature = "gbm-probe")]
    cursor_buffer: Option<drm::control::dumbbuffer::DumbBuffer>,
    #[cfg(feature = "gbm-probe")]
    cursor_framebuffer: Option<drm::control::framebuffer::Handle>,
    #[cfg(feature = "gbm-probe")]
    cursor_planes: Option<Vec<RealAtomicCursorPlane>>,
    #[cfg(feature = "gbm-probe")]
    cursor_plane: Option<drm::control::plane::Handle>,
    #[cfg(feature = "gbm-probe")]
    cursor_crtc: Option<drm::control::crtc::Handle>,
    #[cfg(feature = "gbm-probe")]
    cursor_crtcs_sanitized: bool,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Debug)]
struct RealAtomicCursorPlane {
    plane: drm::control::plane::Handle,
    crtcs: Vec<drm::control::crtc::Handle>,
    fb_id: drm::control::property::Handle,
    crtc_id: drm::control::property::Handle,
    src_x: drm::control::property::Handle,
    src_y: drm::control::property::Handle,
    src_w: drm::control::property::Handle,
    src_h: drm::control::property::Handle,
    crtc_x: drm::control::property::Handle,
    crtc_y: drm::control::property::Handle,
    crtc_w: drm::control::property::Handle,
    crtc_h: drm::control::property::Handle,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicHardwareCursorUpdate {
    Visible,
    Hidden,
    Deferred,
}

impl RealAtomicScanoutPageFlipSession {
    #[cfg(feature = "gbm-probe")]
    fn discover_atomic_cursor_planes(&self) -> io::Result<Vec<RealAtomicCursorPlane>> {
        let mut cursor_planes = Vec::new();
        for plane in LibdrmNativeKmsSelectionDevice::plane_handles(&self.card)? {
            if LibdrmNativeKmsSelectionDevice::plane_type(&self.card, plane)?
                != Some(drm::control::PlaneType::Cursor)
            {
                continue;
            }
            let snapshot = LibdrmNativeKmsSelectionDevice::plane_snapshot(&self.card, plane)?;
            let crtcs = self
                .selections
                .iter()
                .filter_map(|selection| {
                    snapshot
                        .supports_crtc(selection.crtc)
                        .then_some(selection.crtc)
                })
                .collect::<Vec<_>>();
            if crtcs.is_empty() {
                continue;
            }
            let properties =
                LibdrmNativePropertyLookupDevice::plane_property_handles(&self.card, plane)?;
            let required = |name| {
                properties.get(name).ok_or_else(|| {
                    io::Error::other(format!("atomic cursor plane is missing {name}"))
                })
            };
            cursor_planes.push(RealAtomicCursorPlane {
                plane,
                crtcs,
                fb_id: required("FB_ID")?,
                crtc_id: required("CRTC_ID")?,
                src_x: required("SRC_X")?,
                src_y: required("SRC_Y")?,
                src_w: required("SRC_W")?,
                src_h: required("SRC_H")?,
                crtc_x: required("CRTC_X")?,
                crtc_y: required("CRTC_Y")?,
                crtc_w: required("CRTC_W")?,
                crtc_h: required("CRTC_H")?,
            });
        }
        if cursor_planes.is_empty() {
            return Err(io::Error::other(
                "selected KMS outputs expose no compatible atomic cursor plane",
            ));
        }
        Ok(cursor_planes)
    }

    #[cfg(feature = "gbm-probe")]
    fn detach_atomic_cursor_planes(
        &self,
        planes: &[RealAtomicCursorPlane],
        nonblocking: bool,
    ) -> io::Result<ClassicHardwareCursorUpdate> {
        use drm::control::Device as _;

        let mut request = drm::control::atomic::AtomicModeReq::new();
        for cursor in planes {
            request.add_property(
                cursor.plane,
                cursor.fb_id,
                drm::control::property::Value::Framebuffer(None),
            );
            request.add_property(
                cursor.plane,
                cursor.crtc_id,
                drm::control::property::Value::CRTC(None),
            );
        }
        let flags = if nonblocking {
            drm::control::AtomicCommitFlags::NONBLOCK
        } else {
            drm::control::AtomicCommitFlags::empty()
        };
        match self.card.atomic_commit(flags, request) {
            Ok(()) => Ok(ClassicHardwareCursorUpdate::Hidden),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Ok(ClassicHardwareCursorUpdate::Deferred)
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(feature = "gbm-probe")]
    pub fn update_classic_hardware_cursor(
        &mut self,
        target: Option<(LibdrmNativePrimaryPlaneSelection, i32, i32)>,
    ) -> io::Result<ClassicHardwareCursorUpdate> {
        use drm::control::Device as _;

        const EDGE: u32 = 64;
        if self.cursor_planes.is_none() {
            self.cursor_planes = Some(self.discover_atomic_cursor_planes()?);
        }
        if !self.cursor_crtcs_sanitized {
            let planes = self
                .cursor_planes
                .as_deref()
                .ok_or_else(|| io::Error::other("atomic cursor planes disappeared"))?;
            if self.detach_atomic_cursor_planes(planes, false)?
                == ClassicHardwareCursorUpdate::Deferred
            {
                return Ok(ClassicHardwareCursorUpdate::Deferred);
            }
            self.cursor_crtcs_sanitized = true;
        }
        if self.cursor_buffer.is_none() {
            let mut buffer =
                self.card
                    .create_dumb_buffer((EDGE, EDGE), drm::buffer::DrmFourcc::Argb8888, 32)?;
            let pitch = usize::try_from(drm::buffer::Buffer::pitch(&buffer))
                .map_err(|_| io::Error::other("cursor pitch exceeds address space"))?;
            {
                let mut mapping = self.card.map_dumb_buffer(&mut buffer)?;
                mapping.fill(0);
                for (y, row) in sophia_renderer_live::CLASSIC_X11_CURSOR_SHAPE
                    .iter()
                    .enumerate()
                {
                    for (x, pixel) in row.iter().copied().enumerate() {
                        let color = match pixel {
                            b'W' => [0xff, 0xff, 0xff, 0xff],
                            b'#' => [0x00, 0x00, 0x00, 0xff],
                            _ => continue,
                        };
                        let offset = y * pitch + x * 4;
                        mapping[offset..offset + 4].copy_from_slice(&color);
                    }
                }
            }
            self.cursor_framebuffer = Some(self.card.add_framebuffer(&buffer, 32, 32)?);
            self.cursor_buffer = Some(buffer);
        }

        let target = target.filter(|(selection, _, _)| {
            self.selections
                .iter()
                .any(|candidate| candidate.crtc == selection.crtc)
        });
        let Some((selection, x, y)) = target else {
            let Some(previous_plane) = self.cursor_plane else {
                return Ok(ClassicHardwareCursorUpdate::Hidden);
            };
            let cursor = self
                .cursor_planes
                .as_deref()
                .and_then(|planes| planes.iter().find(|cursor| cursor.plane == previous_plane))
                .cloned()
                .ok_or_else(|| io::Error::other("active atomic cursor plane disappeared"))?;
            let outcome = self.detach_atomic_cursor_planes(&[cursor], true)?;
            if outcome != ClassicHardwareCursorUpdate::Deferred {
                self.cursor_plane = None;
                self.cursor_crtc = None;
            }
            return Ok(outcome);
        };
        let cursor = self
            .cursor_planes
            .as_deref()
            .and_then(|planes| {
                planes
                    .iter()
                    .find(|cursor| cursor.crtcs.contains(&selection.crtc))
            })
            .cloned()
            .ok_or_else(|| io::Error::other("target CRTC has no atomic cursor plane"))?;
        let framebuffer = self
            .cursor_framebuffer
            .ok_or_else(|| io::Error::other("atomic cursor framebuffer is unavailable"))?;
        let mut request = drm::control::atomic::AtomicModeReq::new();
        if self.cursor_plane.is_some_and(|plane| plane != cursor.plane)
            && let Some(previous) = self.cursor_plane
            && let Some(previous) = self
                .cursor_planes
                .as_deref()
                .and_then(|planes| planes.iter().find(|cursor| cursor.plane == previous))
        {
            request.add_property(
                previous.plane,
                previous.fb_id,
                drm::control::property::Value::Framebuffer(None),
            );
            request.add_property(
                previous.plane,
                previous.crtc_id,
                drm::control::property::Value::CRTC(None),
            );
        }
        request.add_property(
            cursor.plane,
            cursor.fb_id,
            drm::control::property::Value::Framebuffer(Some(framebuffer)),
        );
        request.add_property(
            cursor.plane,
            cursor.crtc_id,
            drm::control::property::Value::CRTC(Some(selection.crtc)),
        );
        request.add_property(
            cursor.plane,
            cursor.src_x,
            drm::control::property::Value::UnsignedRange(0),
        );
        request.add_property(
            cursor.plane,
            cursor.src_y,
            drm::control::property::Value::UnsignedRange(0),
        );
        request.add_property(
            cursor.plane,
            cursor.src_w,
            drm::control::property::Value::UnsignedRange(u64::from(EDGE) << 16),
        );
        request.add_property(
            cursor.plane,
            cursor.src_h,
            drm::control::property::Value::UnsignedRange(u64::from(EDGE) << 16),
        );
        request.add_property(
            cursor.plane,
            cursor.crtc_x,
            drm::control::property::Value::SignedRange(i64::from(x)),
        );
        request.add_property(
            cursor.plane,
            cursor.crtc_y,
            drm::control::property::Value::SignedRange(i64::from(y)),
        );
        request.add_property(
            cursor.plane,
            cursor.crtc_w,
            drm::control::property::Value::UnsignedRange(u64::from(EDGE)),
        );
        request.add_property(
            cursor.plane,
            cursor.crtc_h,
            drm::control::property::Value::UnsignedRange(u64::from(EDGE)),
        );
        match self
            .card
            .atomic_commit(drm::control::AtomicCommitFlags::NONBLOCK, request)
        {
            Ok(()) => {
                self.cursor_plane = Some(cursor.plane);
                self.cursor_crtc = Some(selection.crtc);
                Ok(ClassicHardwareCursorUpdate::Visible)
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                Ok(ClassicHardwareCursorUpdate::Deferred)
            }
            Err(error) => Err(error),
        }
    }

    pub fn card(&self) -> &RealAtomicScanoutCard {
        &self.card
    }

    pub fn selection(&self) -> LibdrmNativePrimaryPlaneSelection {
        self.selections[0]
    }

    pub fn selections(&self) -> &[LibdrmNativePrimaryPlaneSelection] {
        &self.selections
    }

    pub fn outputs(&self) -> &[OutputId] {
        &self.outputs
    }

    pub fn vrr_properties_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> LibdrmNativeVrrPropertyDiscoveryResult {
        discover_native_vrr_properties(&self.card, selection.connector, selection.crtc)
    }

    pub fn property_names_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<(Vec<String>, Vec<String>)> {
        let mut connector = self
            .card
            .connector_property_handles(selection.connector)?
            .names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut crtc = self
            .card
            .crtc_property_handles(selection.crtc)?
            .names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        connector.sort();
        crtc.sort();
        Ok((connector, crtc))
    }

    #[cfg(feature = "gbm-probe")]
    pub fn render_device_discovery(&self) -> io::Result<RealAtomicScanoutRenderDeviceDiscovery> {
        RealAtomicScanoutRenderDeviceDiscovery::from_card(&self.card)
    }

    #[cfg(all(feature = "gbm-probe", feature = "libdrm-events"))]
    pub fn preferred_xrgb8888_scanout_modifiers(&self) -> Vec<u64> {
        self.preferred_xrgb8888_scanout_modifiers_for_selection(self.selection())
    }

    #[cfg(all(feature = "gbm-probe", feature = "libdrm-events"))]
    pub fn preferred_xrgb8888_scanout_modifiers_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> Vec<u64> {
        let discovery = discover_native_primary_plane_property_handles(
            &self.card,
            selection.connector,
            selection.crtc,
            selection.plane,
        );
        let Some(properties) = discovery.properties else {
            return Vec::new();
        };
        let Some(in_formats) = properties.plane_in_formats() else {
            return Vec::new();
        };

        let Ok(plane_properties) =
            drm::control::Device::get_properties(&self.card, selection.plane)
        else {
            return Vec::new();
        };
        let Some(blob_id) = plane_properties
            .iter()
            .find_map(|(property, value)| (*property == in_formats).then_some(*value))
        else {
            return Vec::new();
        };
        if blob_id == 0 {
            return Vec::new();
        }

        let Ok(blob) = drm::control::Device::get_property_blob(&self.card, blob_id) else {
            return Vec::new();
        };
        let parsed = LibdrmNativePlaneFormatModifierTable::parse_for_format(
            &blob,
            drm::buffer::DrmFourcc::Xrgb8888,
        );
        let Some(table) = parsed.table else {
            return Vec::new();
        };

        table.modifiers().iter().copied().map(u64::from).collect()
    }

    #[cfg(all(feature = "gbm-probe", feature = "libinput-events"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout<P, E>(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        E: RenderDeviceDiscoveryBackend,
    {
        runtime.run_session_loop_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
            input,
            readiness,
            page_flip_budget,
            &self.card,
            exporter,
            &mut self.reader,
            &mut self.poller,
            sender,
        )
    }

    #[cfg(all(feature = "gbm-probe", feature = "libdrm-events"))]
    #[allow(clippy::too_many_arguments)]
    pub fn run_native_gbm_runtime_tick<P, E>(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<P>,
        input: CompositorBackendTickInput,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> Result<LiveBackendRuntimeNativePageFlipTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        E: RenderDeviceDiscoveryBackend,
    {
        runtime
            .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
                input,
                &self.card,
                exporter,
                &mut self.reader,
                &mut self.poller,
                sender,
                max_read,
                max_emit,
            )
    }

    #[cfg(feature = "libdrm-events")]
    pub fn poll_native_page_flip_events(
        &mut self,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> LibdrmNativeReadAndPollReport {
        self.poller
            .read_and_poll_page_flip_events(&mut self.reader, sender, max_read, max_emit)
    }
}

impl Drop for RealAtomicScanoutPageFlipSession {
    fn drop(&mut self) {
        #[cfg(feature = "gbm-probe")]
        {
            use drm::control::Device as _;
            if let Some(planes) = self.cursor_planes.as_deref() {
                let _ = self.detach_atomic_cursor_planes(planes, false);
            }
            self.cursor_plane = None;
            self.cursor_crtc = None;
            if let Some(framebuffer) = self.cursor_framebuffer.take() {
                let _ = self.card.destroy_framebuffer(framebuffer);
            }
            if let Some(buffer) = self.cursor_buffer.take() {
                let _ = self.card.destroy_dumb_buffer(buffer);
            }
        }
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSessionResult {
    pub status: RealAtomicScanoutPageFlipSessionStatus,
    pub card_selection_status: RealAtomicScanoutCardSelectionStatus,
    pub session: Option<RealAtomicScanoutPageFlipSession>,
}

impl RealAtomicScanoutPageFlipSessionResult {
    pub fn failure_evidence(&self) -> Option<LibdrmNativeAtomicScanoutSmokeEvidence> {
        match self.status {
            RealAtomicScanoutPageFlipSessionStatus::Ready => None,
            RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed => {
                Some(self.card_selection_status.failure_evidence())
            }
            RealAtomicScanoutPageFlipSessionStatus::CardCloneFailed => {
                let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed();
                evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::PageFlipReaderUnavailable;
                Some(evidence)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutPageFlipSessionStatus {
    Ready,
    CardSelectionFailed,
    CardCloneFailed,
}

impl RealAtomicScanoutCardSelection {
    pub fn into_page_flip_session(
        mut self,
        slot: LibdrmNativeOutputSlot,
        output: OutputId,
        authority: LibdrmBackendFdAuthority,
    ) -> RealAtomicScanoutPageFlipSessionResult {
        let Some(card) = self.card.take() else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        let Some(selection) = self.selection else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        if self.status != RealAtomicScanoutCardSelectionStatus::Selected {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };

        let Ok(reader_card) = card.try_clone() else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardCloneFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        let reader = NativeLibdrmPageFlipEventReader::new(reader_card)
            .with_crtc_routes([selection.crtc_route(slot)]);
        let poller = NativeLibdrmPageFlipEventPoller::new(
            LibdrmNativePageFlipSource::from_authority(authority),
        )
        .with_routes([LibdrmNativeOutputRoute { slot, output }]);

        RealAtomicScanoutPageFlipSessionResult {
            status: RealAtomicScanoutPageFlipSessionStatus::Ready,
            card_selection_status: self.status,
            session: Some(RealAtomicScanoutPageFlipSession {
                card,
                selections: vec![selection],
                outputs: vec![output],
                reader,
                poller,
                #[cfg(feature = "gbm-probe")]
                cursor_buffer: None,
                #[cfg(feature = "gbm-probe")]
                cursor_framebuffer: None,
                #[cfg(feature = "gbm-probe")]
                cursor_planes: None,
                #[cfg(feature = "gbm-probe")]
                cursor_plane: None,
                #[cfg(feature = "gbm-probe")]
                cursor_crtc: None,
                #[cfg(feature = "gbm-probe")]
                cursor_crtcs_sanitized: false,
            }),
        }
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSessionSetResult {
    pub status: RealAtomicScanoutPageFlipSessionSetStatus,
    pub sessions: Vec<RealAtomicScanoutPageFlipSession>,
    pub output_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutPageFlipSessionSetStatus {
    Ready,
    SelectionFailed,
    CardCloneFailed,
    CapacityExceeded,
}

impl RealAtomicScanoutSelectionSet {
    pub fn into_page_flip_sessions(
        self,
        authority: LibdrmBackendFdAuthority,
    ) -> RealAtomicScanoutPageFlipSessionSetResult {
        if self.status != RealAtomicScanoutSelectionSetStatus::SelectedAll {
            return RealAtomicScanoutPageFlipSessionSetResult {
                status: RealAtomicScanoutPageFlipSessionSetStatus::SelectionFailed,
                sessions: Vec::new(),
                output_count: 0,
            };
        }
        let mut sessions = Vec::new();
        let mut next_output = 1u64;
        let mut next_slot = 1u16;
        for target_set in self.cards {
            let Ok(reader_card) = target_set.card.try_clone() else {
                return RealAtomicScanoutPageFlipSessionSetResult {
                    status: RealAtomicScanoutPageFlipSessionSetStatus::CardCloneFailed,
                    sessions: Vec::new(),
                    output_count: 0,
                };
            };
            let mut crtc_routes = Vec::new();
            let mut output_routes = Vec::new();
            let mut outputs = Vec::new();
            for selection in target_set.selections.iter().copied() {
                let Some(slot) = LibdrmNativeOutputSlot::new(next_slot) else {
                    return RealAtomicScanoutPageFlipSessionSetResult {
                        status: RealAtomicScanoutPageFlipSessionSetStatus::CapacityExceeded,
                        sessions: Vec::new(),
                        output_count: 0,
                    };
                };
                let output = OutputId::from_raw(next_output);
                crtc_routes.push(selection.crtc_route(slot));
                output_routes.push(LibdrmNativeOutputRoute { slot, output });
                outputs.push(output);
                next_output = next_output.saturating_add(1);
                next_slot = next_slot.saturating_add(1);
            }
            let reader =
                NativeLibdrmPageFlipEventReader::new(reader_card).with_crtc_routes(crtc_routes);
            let poller = NativeLibdrmPageFlipEventPoller::new(
                LibdrmNativePageFlipSource::from_authority(authority),
            )
            .with_routes(output_routes);
            sessions.push(RealAtomicScanoutPageFlipSession {
                card: target_set.card,
                selections: target_set.selections,
                outputs,
                reader,
                poller,
                #[cfg(feature = "gbm-probe")]
                cursor_buffer: None,
                #[cfg(feature = "gbm-probe")]
                cursor_framebuffer: None,
                #[cfg(feature = "gbm-probe")]
                cursor_planes: None,
                #[cfg(feature = "gbm-probe")]
                cursor_plane: None,
                #[cfg(feature = "gbm-probe")]
                cursor_crtc: None,
                #[cfg(feature = "gbm-probe")]
                cursor_crtcs_sanitized: false,
            });
        }
        let output_count = usize::try_from(next_output.saturating_sub(1)).unwrap_or(usize::MAX);
        RealAtomicScanoutPageFlipSessionSetResult {
            status: RealAtomicScanoutPageFlipSessionSetStatus::Ready,
            sessions,
            output_count,
        }
    }
}

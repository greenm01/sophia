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
    cursor_crtc: Option<drm::control::crtc::Handle>,
}

impl RealAtomicScanoutPageFlipSession {
    #[cfg(feature = "gbm-probe")]
    pub fn update_classic_hardware_cursor(
        &mut self,
        target: Option<(LibdrmNativePrimaryPlaneSelection, i32, i32)>,
    ) -> io::Result<bool> {
        use drm::control::Device as _;

        const EDGE: u32 = 64;
        if self.cursor_buffer.is_none() {
            let mut buffer =
                self.card
                    .create_dumb_buffer((EDGE, EDGE), drm::buffer::DrmFourcc::Argb8888, 32)?;
            let pitch = usize::try_from(drm::buffer::Buffer::pitch(&buffer))
                .map_err(|_| io::Error::other("cursor pitch exceeds address space"))?;
            {
                let mut mapping = self.card.map_dumb_buffer(&mut buffer)?;
                mapping.fill(0);
                const SHAPE: [&[u8]; 18] = [
                    b"##................",
                    b"#W#...............",
                    b"#WW#..............",
                    b"#WWW#.............",
                    b"#WWWW#............",
                    b"#WWWWW#...........",
                    b"#WWWWWW#..........",
                    b"#WWWWWWW#.........",
                    b"#WWWWWWWW#........",
                    b"#WWWWWWWWW#.......",
                    b"#WWWWW#####.......",
                    b"#WWW#W#...........",
                    b"#WW#.#W#..........",
                    b"#W#..#W#..........",
                    b"##...#WW#.........",
                    b"#....#WW#.........",
                    b".....#WW#.........",
                    b"......##..........",
                ];
                for (y, row) in SHAPE.iter().enumerate() {
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
            self.cursor_buffer = Some(buffer);
        }

        let target = target.filter(|(selection, _, _)| {
            self.selections
                .iter()
                .any(|candidate| candidate.crtc == selection.crtc)
        });
        if self.cursor_crtc != target.map(|(selection, _, _)| selection.crtc) {
            if let Some(previous) = self.cursor_crtc.take() {
                #[allow(deprecated)]
                self.card
                    .set_cursor::<drm::control::dumbbuffer::DumbBuffer>(previous, None)?;
            }
            if let Some((selection, _, _)) = target {
                #[allow(deprecated)]
                self.card
                    .set_cursor2(selection.crtc, self.cursor_buffer.as_ref(), (0, 0))?;
                self.cursor_crtc = Some(selection.crtc);
            }
        }
        if let Some((selection, x, y)) = target {
            #[allow(deprecated)]
            self.card.move_cursor(selection.crtc, (x, y))?;
            Ok(true)
        } else {
            Ok(false)
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
            if let Some(crtc) = self.cursor_crtc.take() {
                #[allow(deprecated)]
                let _ = self
                    .card
                    .set_cursor::<drm::control::dumbbuffer::DumbBuffer>(crtc, None);
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
                cursor_crtc: None,
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
                cursor_crtc: None,
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

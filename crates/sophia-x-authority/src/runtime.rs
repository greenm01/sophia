use std::collections::BTreeMap;

use sophia_portal::ClipboardPortal;
use sophia_protocol::{AuthoritySurface, NamespaceId, Rect, Region, Size, TransactionId};

use crate::{
    ClipboardSelectionDispatch, ClipboardSelectionExecutionError,
    ClipboardSelectionExecutionOutcome, ClipboardSelectionFailureRequest,
    ClipboardSelectionHandoff, ClipboardSelectionNotify, ClipboardSelectionProxy,
    ClipboardSourcePayload, ClipboardTextProperty, PendingClipboardSelection, X_ATOM_ATOM,
    X_ATOM_NONE, XAtomTable, XAuthorityCpuBufferUpdate, XAuthorityPortalCommand,
    XAuthorityRequestKind, XAuthorityRequestPacket, XAuthorityResponsePacket,
    XAuthorityRuntimeError, XAuthoritySelectionArtifact, XByteOrder, XDrawingUpdate,
    XGraphicsContextTable, XGraphicsContextValues, XPoint, XPropertyChange, XPropertyMode,
    XPropertyTable, XResourceKind, XResourceTable, XSelectionEvent, XSelectionMonitor,
    XShmSegmentTable, XSoftwareBufferStore, XWindowLifecycleEvent, XWindowTable,
    clipboard_selection_failure_notify, dispatch_clipboard_selection_request,
    surface_transaction_from_drawing_update,
};

/// Effects of releasing every currently supported resource allocated from one
/// X11 client connection's setup range.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XAuthorityClientResourceRelease {
    /// X11 windows whose properties must be removed from the frontend table.
    pub destroyed_windows: Vec<crate::XResourceId>,
    /// Sophia surfaces that must be removed from Engine's committed snapshot.
    pub removed_surfaces: Vec<sophia_protocol::SurfaceId>,
    pub released_pixmaps: usize,
    pub released_fonts: usize,
    pub released_cursors: usize,
    pub released_graphics_contexts: usize,
    pub released_shm_segments: usize,
}

#[derive(Debug, Default)]
pub struct XAuthorityRuntime {
    resources: XResourceTable,
    windows: XWindowTable,
    shm_segments: XShmSegmentTable,
    selections: XSelectionMonitor,
    clipboard: ClipboardPortal,
    pending_clipboard: BTreeMap<sophia_protocol::PortalTransferId, PendingClipboardSelection>,
    clipboard_proxies: BTreeMap<crate::XResourceId, ClipboardSelectionProxy>,
    next_clipboard_proxy: u32,
    software_buffers: XSoftwareBufferStore,
    graphics_contexts: XGraphicsContextTable,
    window_background_pixels: BTreeMap<crate::XResourceId, u32>,
    last_cpu_buffer_update: Option<XAuthorityCpuBufferUpdate>,
}

impl XAuthorityRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_dispatch(&mut self) {
        self.last_cpu_buffer_update = None;
    }

    pub fn take_cpu_buffer_update(&mut self) -> Option<XAuthorityCpuBufferUpdate> {
        self.last_cpu_buffer_update.take()
    }

    pub fn apply(&mut self, request: XAuthorityRequestPacket) -> XAuthorityResponsePacket {
        match self.apply_checked(&request) {
            Ok(response) => response,
            Err(error) => {
                let mut response = XAuthorityResponsePacket::rejected(request.transaction, error);
                if let XAuthorityRequestKind::RequestSelection {
                    requestor,
                    selection,
                    target,
                    time,
                    transfer,
                    ..
                } = request.kind
                {
                    response
                        .selection_artifacts
                        .push(XAuthoritySelectionArtifact::Failure(
                            clipboard_selection_failure_notify(ClipboardSelectionFailureRequest {
                                transfer,
                                requestor,
                                selection,
                                target,
                                time,
                            }),
                        ));
                }
                response
            }
        }
    }

    fn apply_checked(
        &mut self,
        request: &XAuthorityRequestPacket,
    ) -> Result<XAuthorityResponsePacket, XAuthorityRuntimeError> {
        let mut response = XAuthorityResponsePacket::accepted(request.transaction);

        match &request.kind {
            XAuthorityRequestKind::CreateWindow {
                window,
                surface,
                geometry,
                constraints,
                generation,
            } => {
                self.resources.insert(
                    *window,
                    XResourceKind::Window,
                    request.namespace,
                    *generation,
                )?;
                if let Some(surface) = self.windows.apply(XWindowLifecycleEvent::Created {
                    id: *window,
                    surface: *surface,
                    namespace: request.namespace,
                    geometry: *geometry,
                    constraints: *constraints,
                    generation: *generation,
                })? {
                    response.surfaces.push(surface);
                }
            }
            XAuthorityRequestKind::MapWindow { window, generation } => {
                self.resources
                    .lookup(request.namespace, *window, XResourceKind::Window)?;
                if let Some(surface) = self.windows.apply(XWindowLifecycleEvent::Mapped {
                    id: *window,
                    generation: *generation,
                })? {
                    response.surfaces.push(surface);
                }
            }
            XAuthorityRequestKind::PresentPixmap {
                window,
                pixmap,
                damage,
                previous_committed_generation,
                timeout_msec,
            } => {
                let transaction = surface_transaction_from_drawing_update(
                    &self.windows,
                    XDrawingUpdate::present_pixmap(
                        request.transaction,
                        request.namespace,
                        *window,
                        *pixmap,
                        damage.clone(),
                        *previous_committed_generation,
                        *timeout_msec,
                    ),
                )?;
                self.windows
                    .advance_generation(*window, *previous_committed_generation)?;
                response.transactions.push(transaction);
            }
            XAuthorityRequestKind::SetSelectionOwner {
                selection,
                owner,
                timestamp,
                selection_timestamp,
                kind,
            } => {
                if let Some(owner) = owner {
                    self.resources
                        .lookup(request.namespace, *owner, XResourceKind::Window)?;
                }
                let update = self.selections.apply_event(
                    XSelectionEvent {
                        selection: *selection,
                        owner: *owner,
                        timestamp: *timestamp,
                        selection_timestamp: *selection_timestamp,
                        kind: *kind,
                    },
                    &self.windows,
                );
                if let Some(previous_owner) = update.previous.and_then(|record| record.owner)
                    && Some(previous_owner) != *owner
                {
                    response
                        .selection_artifacts
                        .push(XAuthoritySelectionArtifact::Clear {
                            owner: previous_owner,
                            selection: *selection,
                            time: *timestamp,
                        });
                }
            }
            XAuthorityRequestKind::RequestSelection {
                requestor,
                selection,
                target,
                target_name,
                property,
                time,
                transfer,
            } => {
                self.resources
                    .lookup(request.namespace, *requestor, XResourceKind::Window)?;
                let dispatch = dispatch_clipboard_selection_request(
                    crate::XSelectionRequest {
                        requestor: *requestor,
                        selection: *selection,
                        target: *target,
                        target_name: target_name.clone(),
                        property: *property,
                        time: *time,
                    },
                    &self.selections,
                    &self.windows,
                    *transfer,
                    &mut self.clipboard,
                )?;
                match dispatch {
                    ClipboardSelectionDispatch::SameNamespace(request) => response
                        .selection_artifacts
                        .push(XAuthoritySelectionArtifact::Request(request)),
                    ClipboardSelectionDispatch::CrossNamespace {
                        portal_request,
                        command,
                    } => {
                        self.pending_clipboard.insert(
                            *transfer,
                            PendingClipboardSelection {
                                namespace: request.namespace,
                                portal_request,
                                byte_order: XByteOrder::LittleEndian,
                            },
                        );
                        if let Some(command) = XAuthorityPortalCommand::from_portal_command(command)
                        {
                            response.portal_commands.push(command);
                        }
                    }
                }
            }
        }

        Ok(response)
    }

    pub(crate) fn set_pending_clipboard_byte_order(
        &mut self,
        transfer: sophia_protocol::PortalTransferId,
        byte_order: XByteOrder,
    ) {
        if let Some(pending) = self.pending_clipboard.get_mut(&transfer) {
            pending.byte_order = byte_order;
        }
    }

    pub fn begin_clipboard_source_request(
        &mut self,
        grant: &sophia_protocol::PortalGrant,
    ) -> Result<ClipboardSelectionProxy, ClipboardSelectionExecutionError> {
        let pending = self
            .pending_clipboard
            .get(&grant.transfer)
            .ok_or(ClipboardSelectionExecutionError::UnknownTransfer)?;
        if grant.state != sophia_protocol::PortalGrantState::Active
            || grant.source_generation != pending.portal_request.request.generation
            || grant.source_namespace != pending.portal_request.request.source_namespace
            || grant.target_namespace != pending.portal_request.request.target_namespace
        {
            return Err(ClipboardSelectionExecutionError::StaleOwnerGeneration);
        }
        let owner_record = self
            .selections
            .current_owner_for_selection(pending.portal_request.failure.selection)
            .ok_or(ClipboardSelectionExecutionError::StaleOwnerGeneration)?;
        if owner_record.generation != grant.source_generation {
            return Err(ClipboardSelectionExecutionError::StaleOwnerGeneration);
        }
        let owner = owner_record
            .owner
            .ok_or(ClipboardSelectionExecutionError::StaleOwnerGeneration)?;
        let raw = 0x0001_0000u32
            .checked_add(self.next_clipboard_proxy)
            .filter(|raw| *raw < 0x0020_0000)
            .ok_or(ClipboardSelectionExecutionError::ExecutorFailure)?;
        self.next_clipboard_proxy = self.next_clipboard_proxy.saturating_add(1);
        let proxy = ClipboardSelectionProxy {
            transfer: grant.transfer,
            namespace: grant.source_namespace,
            owner,
            requestor: crate::XResourceId::new(u64::from(raw), 1),
            selection: pending.portal_request.failure.selection,
            target: pending.portal_request.failure.target,
            property: pending.portal_request.failure.target,
            time: pending.portal_request.failure.time,
        };
        self.clipboard_proxies.insert(proxy.requestor, proxy);
        Ok(proxy)
    }

    pub fn is_clipboard_proxy(&self, namespace: NamespaceId, window: crate::XResourceId) -> bool {
        self.clipboard_proxies
            .get(&window)
            .is_some_and(|proxy| proxy.namespace == namespace)
    }

    pub fn capture_clipboard_source_payload(
        &mut self,
        requestor: crate::XResourceId,
        property: crate::XAtom,
        properties: &mut XPropertyTable,
    ) -> Result<ClipboardSourcePayload, ClipboardSelectionExecutionError> {
        let proxy = self
            .clipboard_proxies
            .remove(&requestor)
            .ok_or(ClipboardSelectionExecutionError::UnknownTransfer)?;
        if property == X_ATOM_NONE || property != proxy.property {
            properties.remove_window(proxy.namespace, proxy.requestor);
            return Err(ClipboardSelectionExecutionError::ExecutorFailure);
        }
        let bytes = properties
            .get(proxy.namespace, proxy.requestor, property)
            .map(|record| record.bytes.clone())
            .ok_or(ClipboardSelectionExecutionError::ExecutorFailure)?;
        properties.remove_window(proxy.namespace, proxy.requestor);
        if bytes.len() > crate::MAX_CLIPBOARD_TEXT_HANDOFF_BYTES {
            return Err(ClipboardSelectionExecutionError::PayloadTooLarge);
        }
        Ok(ClipboardSourcePayload {
            transfer: proxy.transfer,
            bytes,
        })
    }

    pub fn discard_clipboard_proxies(
        &mut self,
        transfer: sophia_protocol::PortalTransferId,
    ) -> Vec<(NamespaceId, crate::XResourceId)> {
        let removed = self
            .clipboard_proxies
            .values()
            .filter(|proxy| proxy.transfer == transfer)
            .map(|proxy| (proxy.namespace, proxy.requestor))
            .collect::<Vec<_>>();
        self.clipboard_proxies
            .retain(|_, proxy| proxy.transfer != transfer);
        removed
    }

    /// Completes one broker-approved clipboard transfer. X11 request context
    /// stays in the authority; the executor supplies only a correlated,
    /// bounded payload.
    pub fn execute_clipboard_payload(
        &mut self,
        transfer: sophia_protocol::PortalTransferId,
        grant: &sophia_protocol::PortalGrant,
        payload: &[u8],
        atoms: &mut XAtomTable,
        properties: &mut XPropertyTable,
    ) -> Result<ClipboardSelectionExecutionOutcome, ClipboardSelectionExecutionError> {
        let pending = self
            .pending_clipboard
            .remove(&transfer)
            .ok_or(ClipboardSelectionExecutionError::UnknownTransfer)?;
        let failure = pending.portal_request.failure;
        let fail = |error| ClipboardSelectionExecutionOutcome::Failed {
            error,
            notify: ClipboardSelectionNotify {
                time: failure.time,
                requestor: failure.requestor,
                selection: failure.selection,
                target: failure.target,
                property: X_ATOM_NONE,
            },
        };
        if grant.transfer != transfer
            || grant.state != sophia_protocol::PortalGrantState::Active
            || grant.source_generation != pending.portal_request.request.generation
            || grant.source_namespace != pending.portal_request.request.source_namespace
            || grant.target_namespace != pending.portal_request.request.target_namespace
        {
            return Ok(fail(ClipboardSelectionExecutionError::StaleOwnerGeneration));
        }
        let Some(owner) = self
            .selections
            .current_owner_for_selection(failure.selection)
        else {
            return Ok(fail(ClipboardSelectionExecutionError::StaleOwnerGeneration));
        };
        if owner.generation != pending.portal_request.request.generation {
            return Ok(fail(ClipboardSelectionExecutionError::StaleOwnerGeneration));
        }
        if pending.portal_request.property == X_ATOM_NONE {
            return Ok(fail(ClipboardSelectionExecutionError::MissingProperty));
        }
        if payload.len() > crate::MAX_CLIPBOARD_TEXT_HANDOFF_BYTES {
            return Ok(fail(ClipboardSelectionExecutionError::PayloadTooLarge));
        }
        let selection_name = atoms.name(failure.selection);
        if selection_name != Some("PRIMARY") && selection_name != Some("CLIPBOARD") {
            return Ok(fail(ClipboardSelectionExecutionError::UnsupportedTarget));
        }
        let target_name = pending.portal_request.request.target.as_str();
        let (property_type, format, bytes) = match target_name {
            "TARGETS" => {
                let targets = ["TARGETS", "UTF8_STRING", "text/plain;charset=utf-8"];
                let mut bytes = Vec::with_capacity(targets.len() * 4);
                for name in targets {
                    let atom = atoms
                        .intern(name, false)
                        .map_err(|_| ClipboardSelectionExecutionError::Property)?
                        .expect("intern without only-if-exists returns an atom");
                    match pending.byte_order {
                        XByteOrder::LittleEndian => bytes.extend_from_slice(&atom.to_le_bytes()),
                        XByteOrder::BigEndian => bytes.extend_from_slice(&atom.to_be_bytes()),
                    }
                }
                (X_ATOM_ATOM, 32, bytes)
            }
            "UTF8_STRING" | "text/plain" | "text/plain;charset=utf-8" => {
                if core::str::from_utf8(payload).is_err() {
                    return Ok(fail(ClipboardSelectionExecutionError::InvalidUtf8));
                }
                (failure.target, 8, payload.to_vec())
            }
            _ => return Ok(fail(ClipboardSelectionExecutionError::UnsupportedTarget)),
        };
        if properties
            .apply_change(
                pending.namespace,
                XPropertyChange {
                    mode: XPropertyMode::Replace,
                    window: failure.requestor,
                    property: pending.portal_request.property,
                    property_type,
                    format,
                    bytes: bytes.clone(),
                },
            )
            .is_err()
        {
            return Ok(fail(ClipboardSelectionExecutionError::Property));
        }
        Ok(ClipboardSelectionExecutionOutcome::Handoff(
            ClipboardSelectionHandoff {
                transfer,
                property: ClipboardTextProperty {
                    requestor: failure.requestor,
                    property: pending.portal_request.property,
                    target: failure.target,
                    bytes,
                },
                notify: ClipboardSelectionNotify {
                    time: failure.time,
                    requestor: failure.requestor,
                    selection: failure.selection,
                    target: failure.target,
                    property: pending.portal_request.property,
                },
            },
        ))
    }

    pub fn fail_clipboard_transfer(
        &mut self,
        transfer: sophia_protocol::PortalTransferId,
        error: ClipboardSelectionExecutionError,
    ) -> Result<ClipboardSelectionExecutionOutcome, ClipboardSelectionExecutionError> {
        let pending = self
            .pending_clipboard
            .remove(&transfer)
            .ok_or(ClipboardSelectionExecutionError::UnknownTransfer)?;
        let request = pending.portal_request.failure;
        Ok(ClipboardSelectionExecutionOutcome::Failed {
            error,
            notify: ClipboardSelectionNotify {
                time: request.time,
                requestor: request.requestor,
                selection: request.selection,
                target: request.target,
                property: X_ATOM_NONE,
            },
        })
    }

    pub fn execute_clipboard_payload_frame(
        &mut self,
        frame: &[u8],
        grant: &sophia_protocol::PortalGrant,
        atoms: &mut XAtomTable,
        properties: &mut XPropertyTable,
    ) -> Result<ClipboardSelectionExecutionOutcome, ClipboardSelectionExecutionError> {
        let (transfer, payload) = sophia_protocol::decode_portal_clipboard_payload_frame(frame)
            .map_err(|_| ClipboardSelectionExecutionError::ExecutorFailure)?;
        self.execute_clipboard_payload(transfer, grant, &payload, atoms, properties)
    }

    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn shm_segment_count(&self) -> usize {
        self.shm_segments.len()
    }

    pub fn attach_shm_segment(
        &mut self,
        namespace: NamespaceId,
        segment: crate::XResourceId,
        shmid: u32,
        read_only: bool,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.shm_segments
            .attach(namespace, segment, shmid, read_only, generation)
            .map_err(Into::into)
    }

    pub fn detach_shm_segment(
        &mut self,
        namespace: NamespaceId,
        segment: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.shm_segments
            .detach(namespace, segment)
            .map_err(Into::into)
    }

    pub fn validate_shm_segment_access(
        &self,
        namespace: NamespaceId,
        segment: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.shm_segments
            .lookup(namespace, segment)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn validate_window_access(
        &self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        if self.is_clipboard_proxy(namespace, window) {
            return Ok(());
        }
        self.resources
            .lookup(namespace, window, XResourceKind::Window)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn window_geometry(
        &self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<Rect, XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, window, XResourceKind::Window)?;
        self.windows
            .get(window)
            .map(|record| record.geometry)
            .ok_or(XAuthorityRuntimeError::UnknownResource)
    }

    pub fn configure_window_geometry(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, window, XResourceKind::Window)?;
        self.windows.apply(XWindowLifecycleEvent::Configured {
            id: window,
            x,
            y,
            width,
            height,
            generation,
        })?;
        Ok(())
    }

    /// Ends an X11 window's lifetime and returns the Sophia surface that the
    /// Engine must remove from its committed snapshot.
    pub fn destroy_window(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<sophia_protocol::SurfaceId, XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, window, XResourceKind::Window)?;
        let surface = self
            .windows
            .get(window)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?
            .surface;
        self.selections.clear_window_owner(
            window,
            &self.windows,
            crate::XSelectionChangeKind::SelectionWindowDestroyed,
        );
        self.windows
            .apply(XWindowLifecycleEvent::Destroyed { id: window })?;
        self.resources.remove(window);
        self.software_buffers.remove(window);
        self.window_background_pixels.remove(&window);
        Ok(surface)
    }

    /// Reclaims every supported resource created from a disconnected client's
    /// XID range. Existing-resource references are intentionally not used as
    /// ownership evidence: classic shared-X clients may refer to one another's
    /// resources, while allocation is constrained by the setup range.
    pub fn release_client_resource_range(
        &mut self,
        namespace: NamespaceId,
        range: crate::XWireClientResourceRange,
    ) -> Result<XAuthorityClientResourceRelease, XAuthorityRuntimeError> {
        if !namespace.is_valid() {
            return Err(XAuthorityRuntimeError::InvalidNamespace);
        }

        let mut release = XAuthorityClientResourceRelease::default();
        for gc in self
            .graphics_contexts
            .ids_for_namespace_in_client_range(namespace, range)
        {
            self.free_graphics_context(namespace, gc)?;
            release.released_graphics_contexts =
                release.released_graphics_contexts.saturating_add(1);
        }
        for segment in self
            .shm_segments
            .ids_for_namespace_in_client_range(namespace, range)
        {
            self.detach_shm_segment(namespace, segment)?;
            release.released_shm_segments = release.released_shm_segments.saturating_add(1);
        }

        for record in self
            .resources
            .records_for_namespace_in_client_range(namespace, range)
        {
            match record.kind {
                XResourceKind::Window => {
                    let surface = self.destroy_window(namespace, record.id)?;
                    release.destroyed_windows.push(record.id);
                    release.removed_surfaces.push(surface);
                }
                XResourceKind::Pixmap => {
                    self.free_pixmap(namespace, record.id)?;
                    release.released_pixmaps = release.released_pixmaps.saturating_add(1);
                }
                XResourceKind::Font => {
                    self.close_font(namespace, record.id)?;
                    release.released_fonts = release.released_fonts.saturating_add(1);
                }
                XResourceKind::Cursor => {
                    self.free_cursor(namespace, record.id)?;
                    release.released_cursors = release.released_cursors.saturating_add(1);
                }
                // The reduced frontend does not currently persist client atoms,
                // colormaps, or GCs in the resource table. Remove any future
                // record in this range rather than retaining a disconnect leak.
                XResourceKind::Atom | XResourceKind::Property | XResourceKind::GraphicsContext => {
                    self.resources.remove(record.id);
                }
            }
        }
        Ok(release)
    }

    pub fn configure_window_size_from_engine(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
        size: Size,
    ) -> Result<Rect, XAuthorityRuntimeError> {
        if size.width <= 0
            || size.height <= 0
            || size.width > i32::from(u16::MAX)
            || size.height > i32::from(u16::MAX)
        {
            return Err(XAuthorityRuntimeError::InvalidResource);
        }
        let current = self.window_geometry(namespace, window)?;
        let generation = self
            .windows
            .get(window)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?
            .generation;
        self.configure_window_geometry(
            namespace,
            window,
            None,
            None,
            Some(u16::try_from(size.width).expect("validated above")),
            Some(u16::try_from(size.height).expect("validated above")),
            generation,
        )?;
        Ok(Rect {
            width: size.width,
            height: size.height,
            ..current
        })
    }

    pub fn map_namespace_windows(
        &mut self,
        namespace: NamespaceId,
        generation: u64,
    ) -> Result<Vec<AuthoritySurface>, XAuthorityRuntimeError> {
        let mut surfaces = Vec::new();
        for window in self.windows.ids_for_namespace(namespace) {
            if let Some(surface) = self.windows.apply(XWindowLifecycleEvent::Mapped {
                id: window,
                generation,
            })? {
                surfaces.push(surface);
            }
        }
        Ok(surfaces)
    }

    pub fn create_pixmap(
        &mut self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .insert(pixmap, XResourceKind::Pixmap, namespace, generation)
            .map_err(Into::into)
    }

    pub fn free_pixmap(
        &mut self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, pixmap, XResourceKind::Pixmap)?;
        self.resources.remove(pixmap);
        Ok(())
    }

    pub fn validate_pixmap_access(
        &self,
        namespace: NamespaceId,
        pixmap: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, pixmap, XResourceKind::Pixmap)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn open_font(
        &mut self,
        namespace: NamespaceId,
        font: crate::XResourceId,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .insert(font, XResourceKind::Font, namespace, generation)
            .map_err(Into::into)
    }

    pub fn close_font(
        &mut self,
        namespace: NamespaceId,
        font: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, font, XResourceKind::Font)?;
        self.resources.remove(font);
        Ok(())
    }

    pub fn validate_font_access(
        &self,
        namespace: NamespaceId,
        font: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, font, XResourceKind::Font)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn create_cursor(
        &mut self,
        namespace: NamespaceId,
        cursor: crate::XResourceId,
        generation: u64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .insert(cursor, XResourceKind::Cursor, namespace, generation)
            .map_err(Into::into)
    }

    pub fn free_cursor(
        &mut self,
        namespace: NamespaceId,
        cursor: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, cursor, XResourceKind::Cursor)?;
        self.resources.remove(cursor);
        Ok(())
    }

    pub fn validate_cursor_access(
        &self,
        namespace: NamespaceId,
        cursor: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, cursor, XResourceKind::Cursor)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn validate_drawable_access(
        &self,
        namespace: NamespaceId,
        drawable: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        if drawable.local.raw() == u64::from(crate::X_SETUP_DEFAULT_ROOT) {
            return Ok(());
        }
        if !namespace.is_valid() {
            return Err(XAuthorityRuntimeError::InvalidNamespace);
        }
        let record = self
            .resources
            .get(drawable)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?;
        if !matches!(record.kind, XResourceKind::Window | XResourceKind::Pixmap) {
            return Err(XAuthorityRuntimeError::WrongResourceKind);
        }
        if record.owner_namespace != namespace {
            return Err(XAuthorityRuntimeError::CrossNamespaceDenied);
        }
        Ok(())
    }

    pub fn create_graphics_context(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
        drawable: crate::XResourceId,
        values: XGraphicsContextValues,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_drawable_access(namespace, drawable)?;
        if let Some(font) = values.font {
            self.validate_font_access(namespace, font)?;
        }
        self.graphics_contexts
            .create(namespace, gc, drawable, values)
            .map_err(XAuthorityRuntimeError::from)?;
        Ok(())
    }

    pub fn graphics_context_values(
        &self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
    ) -> Result<XGraphicsContextValues, XAuthorityRuntimeError> {
        self.graphics_contexts
            .get(namespace, gc)
            .map(|record| record.values.clone())
            .map_err(Into::into)
    }

    pub fn set_graphics_context_clip_rectangles(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
        rectangles: Vec<Rect>,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.graphics_contexts
            .set_clip_rectangles(namespace, gc, rectangles)
            .map_err(Into::into)
    }

    pub fn free_graphics_context(
        &mut self,
        namespace: NamespaceId,
        gc: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.graphics_contexts
            .remove(namespace, gc)
            .map_err(Into::into)
    }

    pub fn window_background_pixel(
        &self,
        namespace: NamespaceId,
        window: crate::XResourceId,
    ) -> Result<u32, XAuthorityRuntimeError> {
        self.validate_window_access(namespace, window)?;
        Ok(self
            .window_background_pixels
            .get(&window)
            .copied()
            .unwrap_or(0))
    }

    pub fn set_window_background_pixel(
        &mut self,
        namespace: NamespaceId,
        window: crate::XResourceId,
        pixel: u32,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_window_access(namespace, window)?;
        self.window_background_pixels.insert(window, pixel);
        Ok(())
    }

    pub fn apply_core_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
    ) -> XAuthorityResponsePacket {
        self.apply_core_draw_with_gc(
            transaction,
            namespace,
            window,
            damage,
            &XGraphicsContextValues::default(),
        )
    }

    pub fn apply_core_draw_with_gc(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(buffer) = self.software_buffers.paint_damage(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            &damage.rects,
            gc,
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        self.last_cpu_buffer_update = Some(buffer);
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_copy_area(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        source: crate::XResourceId,
        destination: crate::XResourceId,
        damage: Region,
    ) -> XAuthorityResponsePacket {
        if let Err(error) = self.validate_drawable_access(namespace, source) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        if self.validate_pixmap_access(namespace, destination).is_ok() {
            return XAuthorityResponsePacket::accepted(transaction);
        }
        self.apply_core_draw(transaction, namespace, destination, damage)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_copy_area_with_gc(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        source: crate::XResourceId,
        destination: crate::XResourceId,
        src_x: i16,
        src_y: i16,
        dst_x: i16,
        dst_y: i16,
        width: u16,
        height: u16,
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        if let Err(error) = self.validate_drawable_access(namespace, source) {
            return XAuthorityResponsePacket::rejected(transaction, error);
        }
        if self.validate_pixmap_access(namespace, destination).is_ok() {
            return XAuthorityResponsePacket::accepted(transaction);
        }
        let Some(record) = self.windows.get(destination) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let damage = Region::single(Rect {
            x: i32::from(dst_x),
            y: i32::from(dst_y),
            width: i32::from(width),
            height: i32::from(height),
        });
        let Some(update) = self.software_buffers.copy_area(
            source,
            destination,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            Rect {
                x: i32::from(src_x),
                y: i32::from(src_y),
                width: i32::from(width),
                height: i32::from(height),
            },
            dst_x,
            dst_y,
            gc,
        ) else {
            return self.apply_core_draw_with_gc(transaction, namespace, destination, damage, gc);
        };
        let handle = update.handle();
        self.last_cpu_buffer_update = Some(update);
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            destination,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_line_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        points: &[XPoint],
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(update) = self.software_buffers.draw_lines(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            points,
            gc,
        ) else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let damage = Region::single(Rect {
            x: points
                .iter()
                .map(|point| i32::from(point.x))
                .min()
                .unwrap_or(0),
            y: points
                .iter()
                .map(|point| i32::from(point.y))
                .min()
                .unwrap_or(0),
            width: points
                .iter()
                .map(|point| i32::from(point.x))
                .max()
                .unwrap_or(0)
                .saturating_sub(
                    points
                        .iter()
                        .map(|point| i32::from(point.x))
                        .min()
                        .unwrap_or(0),
                )
                .saturating_add(i32::from(gc.line_width.max(1))),
            height: points
                .iter()
                .map(|point| i32::from(point.y))
                .max()
                .unwrap_or(0)
                .saturating_sub(
                    points
                        .iter()
                        .map(|point| i32::from(point.y))
                        .min()
                        .unwrap_or(0),
                )
                .saturating_add(i32::from(gc.line_width.max(1))),
        });
        let handle = update.handle();
        self.last_cpu_buffer_update = Some(update);
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_put_image(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
        data: Option<&[u8]>,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let size = Size {
            width: record.geometry.width,
            height: record.geometry.height,
        };
        let Some(buffer) = data
            .and_then(|data| {
                damage
                    .rects
                    .first()
                    .and_then(|rect| self.software_buffers.put_image(window, size, *rect, data))
            })
            .or_else(|| {
                self.software_buffers.paint_damage(
                    window,
                    size,
                    &damage.rects,
                    &XGraphicsContextValues::default(),
                )
            })
        else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        self.last_cpu_buffer_update = Some(buffer);
        self.finish_drawing_update(XDrawingUpdate::shm_put_image(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_text_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        x: i16,
        baseline: i16,
        text: &[u8],
        opaque: bool,
        gc: &XGraphicsContextValues,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let damage = Region::single(Rect {
            x: i32::from(x),
            y: i32::from(baseline).saturating_sub(10),
            width: i32::try_from(text.len().saturating_mul(8))
                .unwrap_or(i32::MAX)
                .max(1),
            height: 12,
        });
        let Some(buffer) = self.software_buffers.draw_text(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            x,
            baseline,
            text,
            opaque,
            gc,
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        self.last_cpu_buffer_update = Some(buffer);
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    pub fn apply_clear(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
    ) -> XAuthorityResponsePacket {
        self.apply_clear_with_pixel(transaction, namespace, window, damage, 0)
    }

    pub fn apply_clear_with_pixel(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
        pixel: u32,
    ) -> XAuthorityResponsePacket {
        let Some(record) = self.windows.get(window) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::UnknownResource,
            );
        };
        let Some(rect) = damage.rects.first().copied() else {
            return XAuthorityResponsePacket::accepted(transaction);
        };
        let Some(buffer) = self.software_buffers.clear(
            window,
            Size {
                width: record.geometry.width,
                height: record.geometry.height,
            },
            rect,
            pixel,
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle();
        self.last_cpu_buffer_update = Some(buffer);
        self.finish_drawing_update(XDrawingUpdate::core_draw(
            transaction,
            namespace,
            window,
            handle,
            damage,
            record.generation,
            250,
        ))
    }

    fn finish_drawing_update(&mut self, update: XDrawingUpdate) -> XAuthorityResponsePacket {
        let transaction_id = update.transaction;
        let window = update.target_window;
        let previous_generation = update.previous_committed_generation;
        let transaction = match surface_transaction_from_drawing_update(&self.windows, update) {
            Ok(transaction) => transaction,
            Err(error) => {
                return XAuthorityResponsePacket::rejected(transaction_id, error.into());
            }
        };
        if let Err(error) = self.windows.advance_generation(window, previous_generation) {
            return XAuthorityResponsePacket::rejected(transaction_id, error.into());
        }
        let mut response = XAuthorityResponsePacket::accepted(transaction_id);
        response.transactions.push(transaction);
        response
    }
}

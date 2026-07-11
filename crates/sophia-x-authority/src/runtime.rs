use sophia_portal::ClipboardPortal;
use sophia_protocol::{AuthoritySurface, NamespaceId, Rect, Region, Size, TransactionId};

use crate::{
    ClipboardSelectionFailureRequest, XAuthorityCpuBufferSnapshot, XAuthorityPortalCommand,
    XAuthorityRequestKind, XAuthorityRequestPacket, XAuthorityResponsePacket,
    XAuthorityRuntimeError, XAuthoritySelectionArtifact, XDrawingUpdate, XResourceKind,
    XResourceTable, XSelectionEvent, XSelectionMonitor, XShmSegmentTable, XSoftwareBufferStore,
    XWindowLifecycleEvent, XWindowTable, clipboard_selection_failure_notify,
    dispatch_clipboard_selection_request, surface_transaction_from_drawing_update,
};

#[derive(Debug, Default)]
pub struct XAuthorityRuntime {
    resources: XResourceTable,
    windows: XWindowTable,
    shm_segments: XShmSegmentTable,
    selections: XSelectionMonitor,
    clipboard: ClipboardPortal,
    software_buffers: XSoftwareBufferStore,
    last_cpu_buffer_update: Option<XAuthorityCpuBufferSnapshot>,
}

impl XAuthorityRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_dispatch(&mut self) {
        self.last_cpu_buffer_update = None;
    }

    pub fn take_cpu_buffer_update(&mut self) -> Option<XAuthorityCpuBufferSnapshot> {
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
                response.transactions.push(transaction);
            }
            XAuthorityRequestKind::SetSelectionOwner {
                selection,
                owner,
                timestamp,
                selection_timestamp,
                kind,
            } => {
                self.selections.apply_event(
                    XSelectionEvent {
                        selection: *selection,
                        owner: *owner,
                        timestamp: *timestamp,
                        selection_timestamp: *selection_timestamp,
                        kind: *kind,
                    },
                    &self.windows,
                );
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
                if let Some(command) =
                    XAuthorityPortalCommand::from_portal_command(dispatch.command)
                {
                    response.portal_commands.push(command);
                }
            }
        }

        Ok(response)
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
        if self.validate_window_access(namespace, drawable).is_ok() {
            return Ok(());
        }
        self.validate_pixmap_access(namespace, drawable)
    }

    pub fn apply_core_draw(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
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
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle;
        self.last_cpu_buffer_update = Some(buffer);
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        match surface_transaction_from_drawing_update(
            &self.windows,
            XDrawingUpdate::core_draw(
                transaction,
                namespace,
                window,
                handle,
                damage,
                record.generation,
                250,
            ),
        ) {
            Ok(transaction) => response.transactions.push(transaction),
            Err(error) => return XAuthorityResponsePacket::rejected(transaction, error.into()),
        }
        response
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
                self.software_buffers
                    .paint_damage(window, size, &damage.rects)
            })
        else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle;
        self.last_cpu_buffer_update = Some(buffer);
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        match surface_transaction_from_drawing_update(
            &self.windows,
            XDrawingUpdate::shm_put_image(
                transaction,
                namespace,
                window,
                handle,
                damage,
                record.generation,
                250,
            ),
        ) {
            Ok(transaction) => response.transactions.push(transaction),
            Err(error) => return XAuthorityResponsePacket::rejected(transaction, error.into()),
        }
        response
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
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle;
        self.last_cpu_buffer_update = Some(buffer);
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        match surface_transaction_from_drawing_update(
            &self.windows,
            XDrawingUpdate::core_draw(
                transaction,
                namespace,
                window,
                handle,
                damage,
                record.generation,
                250,
            ),
        ) {
            Ok(transaction) => response.transactions.push(transaction),
            Err(error) => return XAuthorityResponsePacket::rejected(transaction, error.into()),
        }
        response
    }

    pub fn apply_clear(
        &mut self,
        transaction: TransactionId,
        namespace: NamespaceId,
        window: crate::XResourceId,
        damage: Region,
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
        ) else {
            return XAuthorityResponsePacket::rejected(
                transaction,
                XAuthorityRuntimeError::InvalidResource,
            );
        };
        let handle = buffer.handle;
        self.last_cpu_buffer_update = Some(buffer);
        let mut response = XAuthorityResponsePacket::accepted(transaction);
        match surface_transaction_from_drawing_update(
            &self.windows,
            XDrawingUpdate::core_draw(
                transaction,
                namespace,
                window,
                handle,
                damage,
                record.generation,
                250,
            ),
        ) {
            Ok(transaction) => response.transactions.push(transaction),
            Err(error) => return XAuthorityResponsePacket::rejected(transaction, error.into()),
        }
        response
    }
}

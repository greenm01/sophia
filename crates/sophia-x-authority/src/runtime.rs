use std::collections::BTreeMap;

use sophia_portal::ClipboardPortal;
use sophia_protocol::{AuthoritySurface, NamespaceId, Rect, Region, Size, TransactionId};

use crate::{
    ClipboardSelectionFailureRequest, XAuthorityCpuBufferUpdate, XAuthorityPortalCommand,
    XAuthorityRequestKind, XAuthorityRequestPacket, XAuthorityResponsePacket,
    XAuthorityRuntimeError, XAuthoritySelectionArtifact, XDrawingUpdate, XGraphicsContextTable,
    XGraphicsContextValues, XPoint, XResourceKind, XResourceTable, XSelectionEvent,
    XSelectionMonitor, XShmSegmentTable, XSoftwareBufferStore, XWindowLifecycleEvent, XWindowTable,
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
        if self.validate_window_access(namespace, drawable).is_ok() {
            return Ok(());
        }
        self.validate_pixmap_access(namespace, drawable)
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

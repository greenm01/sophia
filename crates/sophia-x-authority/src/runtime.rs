use sophia_portal::ClipboardPortal;

use crate::{
    ClipboardSelectionFailureRequest, XAuthorityPortalCommand, XAuthorityRequestKind,
    XAuthorityRequestPacket, XAuthorityResponsePacket, XAuthorityRuntimeError,
    XAuthoritySelectionArtifact, XDrawingUpdate, XResourceKind, XResourceTable, XSelectionEvent,
    XSelectionMonitor, XWindowLifecycleEvent, XWindowTable, clipboard_selection_failure_notify,
    dispatch_clipboard_selection_request, surface_transaction_from_drawing_update,
};

#[derive(Debug, Default)]
pub struct XAuthorityRuntime {
    resources: XResourceTable,
    windows: XWindowTable,
    selections: XSelectionMonitor,
    clipboard: ClipboardPortal,
}

impl XAuthorityRuntime {
    pub fn new() -> Self {
        Self::default()
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
}

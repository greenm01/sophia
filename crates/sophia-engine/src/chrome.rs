use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChromeActionDecision {
    RequestPoliteClose { surface: SurfaceId },
    Rejected(ChromeActionRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeActionRejectReason {
    UnknownSurface,
    StaleGeneration,
    NotClosable,
    UnsupportedAction,
}

#[derive(Clone, Debug, Default)]
pub struct ChromeBroker {
    descriptors: BTreeMap<SurfaceId, ChromeDescriptor>,
}

pub const MAX_CHROME_LABEL_LEN: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanitizedChromeMetadata {
    pub surface: SurfaceId,
    pub label: Option<String>,
    pub label_redacted: bool,
    pub icon: Option<IconTokenId>,
    pub trust_level: TrustLevel,
    pub attention: AttentionState,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataChromeUpdate {
    Upserted { surface: SurfaceId },
    Removed { surface: SurfaceId },
    Rejected(MetadataChromeRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataChromeRejectReason {
    InvalidSurface,
    InvalidLabel,
    StaleGeneration,
}

impl ChromeBroker {
    pub fn upsert(&mut self, descriptor: ChromeDescriptor) {
        debug!(
            surface_index = descriptor.surface.index(),
            surface_generation = descriptor.surface.generation(),
            descriptor_generation = descriptor.generation,
            has_label = descriptor.label.is_some(),
            has_icon = descriptor.icon.is_some(),
            trust_level = ?descriptor.trust_level,
            attention = ?descriptor.attention,
            "upserting chrome descriptor"
        );
        self.descriptors.insert(descriptor.surface, descriptor);
    }

    pub fn apply_metadata(&mut self, metadata: SanitizedChromeMetadata) -> MetadataChromeUpdate {
        let surface = metadata.surface;
        let generation = metadata.generation;
        let Ok(descriptor) = chrome_descriptor_from_metadata(metadata) else {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected sanitized chrome metadata with invalid label"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel);
        };

        if !descriptor.surface.is_valid() {
            warn!(
                surface_index = descriptor.surface.index(),
                surface_generation = descriptor.surface.generation(),
                metadata_generation = descriptor.generation,
                "rejected sanitized chrome metadata with invalid surface"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidSurface);
        }

        if self
            .get(descriptor.surface)
            .is_some_and(|existing| existing.generation > descriptor.generation)
        {
            warn!(
                surface_index = descriptor.surface.index(),
                surface_generation = descriptor.surface.generation(),
                metadata_generation = descriptor.generation,
                "rejected stale sanitized chrome metadata"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration);
        }

        let surface = descriptor.surface;
        self.upsert(descriptor);
        MetadataChromeUpdate::Upserted { surface }
    }

    pub fn remove_metadata(&mut self, surface: SurfaceId, generation: u64) -> MetadataChromeUpdate {
        if !surface.is_valid() {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected chrome descriptor removal with invalid surface"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidSurface);
        }

        if self
            .get(surface)
            .is_some_and(|existing| existing.generation > generation)
        {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected stale chrome descriptor removal"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration);
        }

        self.remove_surface(surface);
        debug!(
            surface_index = surface.index(),
            surface_generation = surface.generation(),
            metadata_generation = generation,
            "removed chrome descriptor metadata"
        );
        MetadataChromeUpdate::Removed { surface }
    }

    pub fn get(&self, surface: SurfaceId) -> Option<&ChromeDescriptor> {
        self.descriptors.get(&surface)
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> Option<ChromeDescriptor> {
        self.descriptors.remove(&surface)
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

pub const MAX_CHROME_NOTIFICATIONS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromeNotification {
    pub transfer: PortalTransferId,
    pub summary: String,
    pub body: Option<String>,
    pub urgency: NotificationUrgency,
    pub actions: Vec<String>,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationChromeCommand {
    Present { transfer: PortalTransferId },
    Dismiss { transfer: PortalTransferId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationChromeUpdate {
    Staged { transfer: PortalTransferId },
    Presented { transfer: PortalTransferId },
    Dismissed { transfer: PortalTransferId },
    Ignored,
    Rejected(NotificationChromeRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationChromeRejectReason {
    InvalidTransfer,
    InvalidText,
    TooManyActions,
    TooManyVisibleNotifications,
    UnknownTransfer,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationChromePresenter {
    pending: BTreeMap<PortalTransferId, ChromeNotification>,
    visible: BTreeMap<PortalTransferId, ChromeNotification>,
}

impl NotificationChromePresenter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stage_request(&mut self, request: &NotificationRequest) -> NotificationChromeUpdate {
        if !request.transfer.is_valid() {
            warn!(
                transfer = request.transfer.raw(),
                "rejected notification chrome request with invalid transfer"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::InvalidTransfer,
            );
        }

        if !valid_notification_chrome_text(&request.summary, MAX_NOTIFICATION_SUMMARY_LEN)
            || request.body.as_ref().is_some_and(|body| {
                !valid_notification_chrome_text(body, MAX_NOTIFICATION_BODY_LEN)
            })
            || request
                .actions
                .iter()
                .any(|action| !valid_notification_chrome_text(action, MAX_NOTIFICATION_ACTION_LEN))
        {
            warn!(
                transfer = request.transfer.raw(),
                generation = request.generation,
                action_count = request.actions.len(),
                "rejected notification chrome request with invalid text"
            );
            return NotificationChromeUpdate::Rejected(NotificationChromeRejectReason::InvalidText);
        }

        if request.actions.len() > MAX_NOTIFICATION_ACTIONS {
            warn!(
                transfer = request.transfer.raw(),
                generation = request.generation,
                action_count = request.actions.len(),
                "rejected notification chrome request with too many actions"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::TooManyActions,
            );
        }

        let notification = ChromeNotification {
            transfer: request.transfer,
            summary: request.summary.clone(),
            body: request.body.clone(),
            urgency: request.urgency,
            actions: request.actions.clone(),
            generation: request.generation,
        };

        self.pending.insert(request.transfer, notification);
        debug!(
            transfer = request.transfer.raw(),
            generation = request.generation,
            urgency = ?request.urgency,
            action_count = request.actions.len(),
            pending_count = self.pending.len(),
            "staged notification chrome request"
        );
        NotificationChromeUpdate::Staged {
            transfer: request.transfer,
        }
    }

    pub fn apply_portal_command(&mut self, command: &PortalCommand) -> NotificationChromeUpdate {
        let Some(command) = notification_chrome_command_from_portal(command) else {
            return NotificationChromeUpdate::Ignored;
        };

        self.apply_command(command)
    }

    pub fn apply_command(
        &mut self,
        command: NotificationChromeCommand,
    ) -> NotificationChromeUpdate {
        match command {
            NotificationChromeCommand::Present { transfer } => self.present(transfer),
            NotificationChromeCommand::Dismiss { transfer } => self.dismiss(transfer),
        }
    }

    pub fn pending(&self, transfer: PortalTransferId) -> Option<&ChromeNotification> {
        self.pending.get(&transfer)
    }

    pub fn visible(&self, transfer: PortalTransferId) -> Option<&ChromeNotification> {
        self.visible.get(&transfer)
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    fn present(&mut self, transfer: PortalTransferId) -> NotificationChromeUpdate {
        let Some(notification) = self.pending.remove(&transfer) else {
            warn!(
                transfer = transfer.raw(),
                "rejected notification chrome present for unknown transfer"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::UnknownTransfer,
            );
        };

        if self.visible.len() >= MAX_CHROME_NOTIFICATIONS && !self.visible.contains_key(&transfer) {
            self.pending.insert(transfer, notification);
            warn!(
                transfer = transfer.raw(),
                visible_count = self.visible.len(),
                max_visible = MAX_CHROME_NOTIFICATIONS,
                "rejected notification chrome present because visible set is full"
            );
            return NotificationChromeUpdate::Rejected(
                NotificationChromeRejectReason::TooManyVisibleNotifications,
            );
        }

        self.visible.insert(transfer, notification);
        debug!(
            transfer = transfer.raw(),
            pending_count = self.pending.len(),
            visible_count = self.visible.len(),
            "presented notification chrome"
        );
        NotificationChromeUpdate::Presented { transfer }
    }

    fn dismiss(&mut self, transfer: PortalTransferId) -> NotificationChromeUpdate {
        let removed_pending = self.pending.remove(&transfer).is_some();
        let removed_visible = self.visible.remove(&transfer).is_some();

        if removed_pending || removed_visible {
            debug!(
                transfer = transfer.raw(),
                removed_pending,
                removed_visible,
                pending_count = self.pending.len(),
                visible_count = self.visible.len(),
                "dismissed notification chrome"
            );
            NotificationChromeUpdate::Dismissed { transfer }
        } else {
            warn!(
                transfer = transfer.raw(),
                "rejected notification chrome dismiss for unknown transfer"
            );
            NotificationChromeUpdate::Rejected(NotificationChromeRejectReason::UnknownTransfer)
        }
    }
}

pub fn notification_chrome_command_from_portal(
    command: &PortalCommand,
) -> Option<NotificationChromeCommand> {
    match command {
        PortalCommand::DeliverNotification { transfer } => {
            Some(NotificationChromeCommand::Present {
                transfer: *transfer,
            })
        }
        PortalCommand::DropNotification { transfer } => Some(NotificationChromeCommand::Dismiss {
            transfer: *transfer,
        }),
        _ => None,
    }
}

fn chrome_descriptor_from_metadata(
    metadata: SanitizedChromeMetadata,
) -> Result<ChromeDescriptor, MetadataChromeRejectReason> {
    let label = metadata
        .label
        .map(|text| {
            if valid_chrome_label(&text) {
                Ok(DisplayLabel {
                    text,
                    redacted: metadata.label_redacted,
                })
            } else {
                Err(MetadataChromeRejectReason::InvalidLabel)
            }
        })
        .transpose()?;

    Ok(ChromeDescriptor {
        surface: metadata.surface,
        label,
        icon: metadata.icon,
        trust_level: metadata.trust_level,
        attention: metadata.attention,
        generation: metadata.generation,
    })
}

fn valid_chrome_label(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_CHROME_LABEL_LEN && !text.chars().any(char::is_control)
}

fn valid_notification_chrome_text(text: &str, max_len: usize) -> bool {
    !text.is_empty() && text.len() <= max_len && !text.chars().any(char::is_control)
}

pub fn validate_chrome_action(
    request: &ChromeActionRequest,
    nodes: &[LayoutNodeSnapshot],
) -> ChromeActionDecision {
    let Some(node) = nodes.iter().find(|node| node.surface == request.surface) else {
        warn!(
            surface_index = request.surface.index(),
            surface_generation = request.surface.generation(),
            request_generation = request.generation,
            action = ?request.kind,
            "rejected chrome action for unknown surface"
        );
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::UnknownSurface);
    };

    if node.generation != request.generation {
        warn!(
            surface_index = request.surface.index(),
            surface_generation = request.surface.generation(),
            request_generation = request.generation,
            current_generation = node.generation,
            action = ?request.kind,
            "rejected stale chrome action"
        );
        return ChromeActionDecision::Rejected(ChromeActionRejectReason::StaleGeneration);
    }

    match request.kind {
        ChromeActionKind::CloseSurfaceRequested => {
            if node.capabilities.closable {
                debug!(
                    surface_index = request.surface.index(),
                    surface_generation = request.surface.generation(),
                    request_generation = request.generation,
                    action = ?request.kind,
                    "accepted chrome action"
                );
                ChromeActionDecision::RequestPoliteClose {
                    surface: request.surface,
                }
            } else {
                warn!(
                    surface_index = request.surface.index(),
                    surface_generation = request.surface.generation(),
                    request_generation = request.generation,
                    action = ?request.kind,
                    "rejected chrome action for non-closable surface"
                );
                ChromeActionDecision::Rejected(ChromeActionRejectReason::NotClosable)
            }
        }
    }
}

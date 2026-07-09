use crate::prelude::*;

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

fn valid_notification_chrome_text(text: &str, max_len: usize) -> bool {
    !text.is_empty() && text.len() <= max_len && !text.chars().any(char::is_control)
}

use crate::prelude::*;
use crate::{MetadataChromeUpdate, NotificationChromeUpdate};

use super::super::observation::{
    runtime_observation_from_metadata_chrome_updates,
    runtime_observation_from_notification_chrome_updates,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveChromeRuntimeAdapter {
    pub command_count: u32,
}

impl LiveChromeRuntimeAdapter {
    pub fn from_command_count(count: u32) -> Self {
        Self {
            command_count: count,
        }
    }

    pub fn from_notification_updates<'a>(
        updates: impl IntoIterator<Item = &'a NotificationChromeUpdate>,
    ) -> Self {
        let SessionRuntimeObservation::ChromeCommandsReady { count } =
            runtime_observation_from_notification_chrome_updates(updates)
        else {
            unreachable!("notification chrome updates always map to chrome command counts");
        };

        Self::from_command_count(count)
    }

    pub fn from_metadata_updates<'a>(
        updates: impl IntoIterator<Item = &'a MetadataChromeUpdate>,
    ) -> Self {
        let SessionRuntimeObservation::ChromeCommandsReady { count } =
            runtime_observation_from_metadata_chrome_updates(updates)
        else {
            unreachable!("metadata chrome updates always map to chrome command counts");
        };

        Self::from_command_count(count)
    }

    pub fn present_observation(&self) -> SessionRuntimeObservation {
        SessionRuntimeObservation::ChromeCommandsReady {
            count: self.command_count,
        }
    }
}

use crate::WmTransactionUpdate;
use crate::prelude::*;

use super::super::observation::runtime_observation_from_wm_transaction_update;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveWmRuntimeAdapter {
    pub update: Option<WmTransactionUpdate>,
}

impl LiveWmRuntimeAdapter {
    pub fn from_transaction_update(update: WmTransactionUpdate) -> Self {
        Self {
            update: Some(update),
        }
    }

    pub fn layout_observation(&self) -> SessionRuntimeObservation {
        self.update
            .as_ref()
            .map(runtime_observation_from_wm_transaction_update)
            .unwrap_or(SessionRuntimeObservation::WmLayoutReady)
    }
}

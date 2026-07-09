use crate::prelude::*;

pub const MAX_CLIENT_CLASS_KEY_LEN: usize = 128;
pub const DEFAULT_SYNC_TIMEOUT_STRIKE_LIMIT: u32 = 3;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientClassKey(String);

impl ClientClassKey {
    pub fn new(value: impl AsRef<str>) -> Option<Self> {
        let value = value.as_ref().trim();
        if value.is_empty()
            || value.len() > MAX_CLIENT_CLASS_KEY_LEN
            || value.chars().any(char::is_control)
        {
            return None;
        }

        Some(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSyncProfile {
    pub window: XWindowId,
    pub namespace: Option<NamespaceId>,
    pub class_key: Option<ClientClassKey>,
    pub advertised_sync: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SyncReputationTracker {
    timeout_strikes: BTreeMap<(Option<NamespaceId>, ClientClassKey), u32>,
    strike_limit: u32,
}

impl SyncReputationTracker {
    pub fn new(strike_limit: u32) -> Self {
        Self {
            timeout_strikes: BTreeMap::new(),
            strike_limit,
        }
    }

    pub fn record_timeout(&mut self, namespace: Option<NamespaceId>, class_key: &ClientClassKey) {
        let strikes = self
            .timeout_strikes
            .entry((namespace, class_key.clone()))
            .or_insert(0);
        *strikes = strikes.saturating_add(1);
    }

    pub fn strikes_for(&self, namespace: Option<NamespaceId>, class_key: &ClientClassKey) -> u32 {
        self.timeout_strikes
            .get(&(namespace, class_key.clone()))
            .copied()
            .unwrap_or(0)
    }

    pub fn is_downgraded(
        &self,
        namespace: Option<NamespaceId>,
        class_key: &ClientClassKey,
    ) -> bool {
        self.strikes_for(namespace, class_key) >= self.strike_limit()
    }

    pub fn strike_limit(&self) -> u32 {
        if self.strike_limit == 0 {
            DEFAULT_SYNC_TIMEOUT_STRIKE_LIMIT
        } else {
            self.strike_limit
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurfaceSyncRegistry {
    profiles: BTreeMap<XWindowId, ClientSyncProfile>,
    reputation: SyncReputationTracker,
}

impl SurfaceSyncRegistry {
    pub fn new(reputation: SyncReputationTracker) -> Self {
        Self {
            profiles: BTreeMap::new(),
            reputation,
        }
    }

    pub fn upsert_profile(&mut self, profile: ClientSyncProfile) {
        self.profiles.insert(profile.window, profile);
    }

    pub fn record_timeout_for_window(&mut self, window: XWindowId) -> bool {
        let Some(profile) = self.profiles.get(&window) else {
            return false;
        };
        let Some(class_key) = &profile.class_key else {
            return false;
        };

        self.reputation.record_timeout(profile.namespace, class_key);
        true
    }

    pub fn capability_for_window(&self, window: XWindowId) -> ResizeSyncCapability {
        let Some(profile) = self.profiles.get(&window) else {
            return ResizeSyncCapability::ImplicitOnly;
        };
        if !profile.advertised_sync {
            return ResizeSyncCapability::ImplicitOnly;
        }
        if let Some(class_key) = &profile.class_key {
            if self.reputation.is_downgraded(profile.namespace, class_key) {
                return ResizeSyncCapability::ImplicitOnly;
            }
        }

        ResizeSyncCapability::ExplicitSync
    }
}

pub fn sync_capability_from_wm_protocols(
    protocols: &[Atom],
    net_wm_sync_request_atom: Atom,
) -> ResizeSyncCapability {
    if protocols.contains(&net_wm_sync_request_atom) {
        ResizeSyncCapability::ExplicitSync
    } else {
        ResizeSyncCapability::ImplicitOnly
    }
}

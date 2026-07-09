use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::NamespaceId;

use crate::{XAuthorityAccessError, XResourceId, XResourceKind, XResourceTable};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum XEventClass {
    Structure,
    Property,
    Focus,
    Keyboard,
    Pointer,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XEventSubscriptionTable {
    subscriptions: BTreeMap<XResourceId, BTreeMap<NamespaceId, BTreeSet<XEventClass>>>,
}

impl XEventSubscriptionTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(
        &mut self,
        resources: &XResourceTable,
        requester_namespace: NamespaceId,
        target: XResourceId,
        class: XEventClass,
    ) -> Result<(), XAuthorityAccessError> {
        resources.lookup(requester_namespace, target, XResourceKind::Window)?;
        self.subscriptions
            .entry(target)
            .or_default()
            .entry(requester_namespace)
            .or_default()
            .insert(class);
        Ok(())
    }

    pub fn subscribers(
        &self,
        target: XResourceId,
        owner_namespace: NamespaceId,
        class: XEventClass,
    ) -> Vec<NamespaceId> {
        self.subscriptions
            .get(&target)
            .into_iter()
            .flat_map(|by_namespace| by_namespace.iter())
            .filter_map(|(namespace, classes)| {
                (*namespace == owner_namespace && classes.contains(&class)).then_some(*namespace)
            })
            .collect()
    }
}

use sophia_protocol::{PortalGrant, PortalRequest, PortalTransferId};

use crate::{PortalLifecycleError, PortalPolicyDecision, PortalRequestGrantLifecycle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadlessPortalPolicy {
    Deny,
    Allow,
}

impl Default for HeadlessPortalPolicy {
    fn default() -> Self {
        Self::Deny
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortalCapabilityAdmission {
    pub source_may_publish: bool,
    pub target_may_request: bool,
}

impl PortalCapabilityAdmission {
    pub const fn allowed(self) -> bool {
        self.source_may_publish && self.target_may_request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortalBrokerDecision {
    Allowed(PortalGrant),
    Denied,
}

#[derive(Debug)]
pub struct DeterministicPortalBroker {
    policy: HeadlessPortalPolicy,
    lifecycle: PortalRequestGrantLifecycle,
}

impl DeterministicPortalBroker {
    pub fn new(
        broker_generation: u64,
        policy: HeadlessPortalPolicy,
    ) -> Result<Self, PortalLifecycleError> {
        Ok(Self {
            policy,
            lifecycle: PortalRequestGrantLifecycle::new(broker_generation)?,
        })
    }

    pub fn request(
        &mut self,
        request: PortalRequest,
        admission: PortalCapabilityAdmission,
        now_msec: u64,
    ) -> Result<PortalBrokerDecision, PortalLifecycleError> {
        let transfer = request.transfer.transfer;
        let source_generation = request.transfer.generation;
        self.lifecycle.submit(request, now_msec)?;
        let allow = admission.allowed() && self.policy == HeadlessPortalPolicy::Allow;
        let grant = self.lifecycle.decide(
            transfer,
            if allow {
                PortalPolicyDecision::Allow
            } else {
                PortalPolicyDecision::Deny
            },
            source_generation,
            now_msec,
        )?;
        Ok(match grant {
            Some(grant) => PortalBrokerDecision::Allowed(grant.clone()),
            None => PortalBrokerDecision::Denied,
        })
    }

    pub fn complete(&mut self, transfer: PortalTransferId) -> Result<(), PortalLifecycleError> {
        self.lifecycle.complete(transfer)
    }

    pub fn executor_failed(
        &mut self,
        transfer: PortalTransferId,
    ) -> Result<(), PortalLifecycleError> {
        self.lifecycle.executor_failed(transfer)
    }

    pub const fn policy(&self) -> HeadlessPortalPolicy {
        self.policy
    }

    pub fn lifecycle(&self) -> &PortalRequestGrantLifecycle {
        &self.lifecycle
    }

    pub fn lifecycle_mut(&mut self) -> &mut PortalRequestGrantLifecycle {
        &mut self.lifecycle
    }
}

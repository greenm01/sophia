use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveInputReadinessGateReport {
    pub status: LiveInputReadinessGateStatus,
}

impl LiveInputReadinessGateReport {
    pub const fn idle() -> Self {
        Self {
            status: LiveInputReadinessGateStatus::Idle,
        }
    }

    pub const fn ready() -> Self {
        Self {
            status: LiveInputReadinessGateStatus::Ready,
        }
    }

    pub const fn polled() -> Self {
        Self {
            status: LiveInputReadinessGateStatus::Polled,
        }
    }

    pub const fn read_failed() -> Self {
        Self {
            status: LiveInputReadinessGateStatus::ReadFailed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveInputReadinessGateStatus {
    Idle,
    Ready,
    Polled,
    ReadFailed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveInputReadinessGatedPoller<P> {
    poller: P,
    ready_once: bool,
    last_gate: LiveInputReadinessGateReport,
}

impl<P> LiveInputReadinessGatedPoller<P> {
    pub fn new(poller: P) -> Self {
        Self {
            poller,
            ready_once: false,
            last_gate: LiveInputReadinessGateReport::idle(),
        }
    }

    pub fn observe_ready(&mut self) {
        self.ready_once = true;
        self.last_gate = LiveInputReadinessGateReport::ready();
    }

    pub fn clear_ready(&mut self) {
        self.ready_once = false;
        self.last_gate = LiveInputReadinessGateReport::idle();
    }

    pub const fn last_gate_report(&self) -> LiveInputReadinessGateReport {
        self.last_gate
    }

    pub const fn ready(&self) -> bool {
        self.ready_once
    }

    pub fn inner(&self) -> &P {
        &self.poller
    }

    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.poller
    }
}

impl<P> NonBlockingInputPoller for LiveInputReadinessGatedPoller<P>
where
    P: NonBlockingInputPoller,
{
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>> {
        if !self.ready_once {
            self.last_gate = LiveInputReadinessGateReport::idle();
            return Ok(Vec::new());
        }

        self.ready_once = false;
        match self.poller.poll_ready() {
            Ok(events) => {
                self.last_gate = LiveInputReadinessGateReport::polled();
                Ok(events)
            }
            Err(error) => {
                self.last_gate = LiveInputReadinessGateReport::read_failed();
                Err(error)
            }
        }
    }
}

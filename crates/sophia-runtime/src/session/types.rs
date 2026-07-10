use crate::prelude::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SessionRuntimePhase {
    #[default]
    Idle,
    PollingX,
    ApplyingWmPolicy,
    WaitingForFrame,
    Rendering,
    SubmittingScanout,
    DrainingPortals,
    PresentingChrome,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionRuntimeState {
    pub phase: SessionRuntimePhase,
    pub x_events_polled: u64,
    pub frames_rendered: u64,
    pub scanout_submissions: u64,
    pub scanout_retirements: u64,
    pub scanout_rejections: u64,
    pub in_flight_scanouts: u64,
    pub portal_commands_drained: u64,
    pub chrome_commands_presented: u64,
    pub wm_restart_requests: u64,
    pub authority_transactions_committed: u64,
    pub authority_transactions_rejected: u64,
    pub authority_transactions_timed_out: u64,
    pub authority_surfaces_applied: u64,
    pub slow_client_timeouts: u64,
    pub slow_client_preserved: u64,
    pub slow_client_degraded: u64,
    pub last_frame_serial: Option<u64>,
    pub last_scanout_frame_serial: Option<u64>,
    pub last_scanout_state: Option<RuntimeScanoutState>,
    pub portal_broker_health: Option<RuntimeBrokerHealth>,
    pub metadata_broker_health: Option<RuntimeBrokerHealth>,
    pub x_authority_health: Option<RuntimeAuthorityHealth>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeScanoutState {
    Submitted,
    Retired,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBrokerHealth {
    pub state: BrokerHealthState,
    pub generation: u64,
    pub status_message_len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAuthorityHealth {
    pub process: SupervisedProcessKind,
    pub state: BrokerHealthState,
    pub generation: u64,
    pub status_message_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionRuntimeEvent {
    TickStarted,
    XEventsPolled {
        count: u32,
    },
    WmLayoutReady,
    WmRestartRequested,
    FrameScheduled {
        frame_serial: u64,
    },
    FrameRendered {
        frame_serial: u64,
    },
    ScanoutStateChanged {
        state: RuntimeScanoutState,
        frame_serial: Option<u64>,
    },
    PortalCommandsReady {
        count: u32,
    },
    ChromeCommandsReady {
        count: u32,
    },
    BrokerHealthChanged {
        broker: BrokerKind,
        state: BrokerHealthState,
        generation: u64,
        status_message_len: usize,
    },
    AuthorityProcessHealthChanged {
        process: SupervisedProcessKind,
        state: BrokerHealthState,
        generation: u64,
        status_message_len: usize,
    },
    AuthorityTransactionObserved {
        outcome: TransactionOutcome,
        applied_surface_count: u32,
    },
    SlowClientVisualDecisionsObserved {
        timeout_count: u32,
        preserved_count: u32,
        degraded_count: u32,
    },
    TickCompleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRuntimeCommand {
    None,
    PollXEvents,
    RequestWmLayout,
    ScheduleFrame,
    RenderFrame { frame_serial: u64 },
    SubmitScanout { frame_serial: u64 },
    DrainPortalCommands,
    PresentChrome,
    RestartWindowManager,
}

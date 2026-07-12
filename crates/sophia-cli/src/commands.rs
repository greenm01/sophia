#[cfg(feature = "atomic-scanout-live")]
mod backend;
mod help;
#[cfg(feature = "atomic-scanout-live")]
mod live_session;
mod runtime;
#[cfg(feature = "atomic-scanout-live")]
mod wayland;
mod x_authority;

#[allow(unused_imports)]
mod prelude {
    pub(crate) use crate::support::*;

    pub(crate) use sophia_engine::{
        AuthorityTransactionInbox, AuthorityTransactionIntake, CompositorBackendTickInput,
        FrameClockTick, FrameScheduleDecision, HeadlessCompositorBackendAssembly, HeadlessEngine,
        HeadlessSessionDriver, HeadlessSessionDriverTick, LayoutEpochState,
        LiveRuntimeDriverAdapter, LiveRuntimeDriverIntake, WmSocketTransport,
        WmSocketTransportConfig, WmTransactionUpdate, schedule_frame_from_damage,
    };
    pub(crate) use sophia_portal::PortalCommand;
    pub(crate) use sophia_protocol::{
        BrokerHealthPacket, BrokerHealthState, BrokerKind, BufferSource, CommittedSurfaceState,
        DamageFrame, LayerSnapshot, LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot,
        LayoutNodeState, NamespaceId, PortalTransferId, Rect, Region, ResizeSyncCapability, Size,
        SurfaceConstraints, SurfaceId, SurfaceTransaction, TransactionCommit, TransactionId,
        TransactionOutcome, Transform, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket,
        WorkspaceId, decode_broker_health_frame, encode_broker_health_frame,
    };
    pub(crate) use sophia_runtime::{
        ProcessLaunchSpec, ProcessSupervisor, RestartPolicy, RuntimeBrokerSupervisors,
        SessionRuntimeCommand, SessionRuntimeLoop, SessionRuntimeObservation,
        SupervisedProcessKind, SupervisorEvent, update_supervisor,
    };
    pub(crate) use sophia_x_authority::{
        X_SOPHIA_PRESENT_EXTENSION_NAME, X_SOPHIA_PRESENT_MAJOR_OPCODE,
        X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE, XAuthorityCpuBufferSnapshot,
        XAuthorityCpuBufferUpdate, XAuthorityKeyEvent, XAuthorityObservedTransactionBatch,
        XAuthorityRequestKind, XAuthorityRequestPacket, XByteOrder, XClientOutput, XResourceId,
        XSelectionChangeKind as XAuthoritySelectionChangeKind, read_x_authority_response,
        run_x_authority_socket_server_once, run_x11_core_socket_server_once,
        run_x11_core_socket_server_once_channel, run_x11_core_socket_server_once_channels,
        run_x11_core_socket_server_once_observed,
        run_x11_core_socket_server_once_traced_with_idle_timeout, write_x_authority_request,
        x_fixed_glyph_rows,
    };
    pub(crate) use std::os::unix::net::UnixStream;
    pub(crate) use std::sync::mpsc::sync_channel;
    pub(crate) use std::time::{Duration, SystemTime, UNIX_EPOCH};
    pub(crate) use x11rb::protocol::Event;
}

pub(crate) fn run(args: &[String], verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "atomic-scanout-live")]
    if backend::try_run(args)? {
        return Ok(());
    }
    if runtime::try_run(args)? {
        return Ok(());
    }
    if x_authority::try_run(args)? {
        return Ok(());
    }
    #[cfg(feature = "atomic-scanout-live")]
    if wayland::try_run(args)? {
        return Ok(());
    }

    help::print(verbose);
    Ok(())
}

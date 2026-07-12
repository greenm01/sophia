#[cfg(feature = "atomic-scanout-live")]
mod backend;
mod help;
#[cfg(feature = "atomic-scanout-live")]
mod live_session;
#[cfg(feature = "xlibre-research")]
mod portal;
#[cfg(feature = "xlibre-research")]
mod routed_input;
mod runtime;
#[cfg(feature = "atomic-scanout-live")]
mod wayland;
#[cfg(feature = "xlibre-research")]
mod x;
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
    #[cfg(feature = "xlibre-research")]
    pub(crate) use sophia_engine::{
        FramePlanRequest, runtime_observation_from_wm_transaction_update,
    };
    pub(crate) use sophia_portal::PortalCommand;
    #[cfg(feature = "xlibre-research")]
    pub(crate) use sophia_portal::{ClipboardPortal, ClipboardTarget, ClipboardTransferRequest};
    #[cfg(feature = "xlibre-research")]
    pub(crate) use sophia_protocol::XWindowId;
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
    #[cfg(feature = "xlibre-research")]
    pub(crate) use sophia_wm_demo::{ExternalWmClient, tile_workspace};
    #[cfg(all(feature = "atomic-scanout-live", feature = "xlibre-research"))]
    pub(crate) use sophia_x_authority::XAuthorityCpuBufferPatch;
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
    #[cfg(feature = "xlibre-research")]
    pub(crate) use sophia_x_bridge::{
        ClipboardSelectionFailureRequest, TestClientConfig, XMirrorState, XSelectionChangeKind,
        XSelectionEvent, XSelectionMonitor, capture_readback_display,
        clipboard_selection_failure_notify, clipboard_selection_text_handoff_notify,
        dispatch_clipboard_selection_request_event, run_test_client_window,
        smoke_live_clipboard_portal, smoke_routed_input, smoke_routed_input_edges,
        stress_routed_input,
    };
    pub(crate) use std::os::unix::net::UnixStream;
    pub(crate) use std::sync::mpsc::sync_channel;
    pub(crate) use std::time::{Duration, SystemTime, UNIX_EPOCH};
    pub(crate) use x11rb::protocol::Event;
    #[cfg(feature = "xlibre-research")]
    pub(crate) use x11rb::protocol::xproto::SelectionRequestEvent;
}

pub(crate) fn run(args: &[String], verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "atomic-scanout-live")]
    if backend::try_run(args)? {
        return Ok(());
    }
    #[cfg(feature = "xlibre-research")]
    if x::try_run(args)? {
        return Ok(());
    }
    if runtime::try_run(args)? {
        return Ok(());
    }
    #[cfg(feature = "xlibre-research")]
    if portal::try_run(args)? {
        return Ok(());
    }
    #[cfg(feature = "xlibre-research")]
    if routed_input::try_run(args)? {
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

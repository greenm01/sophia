mod prelude {
    pub(crate) use core::fmt;
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    pub(crate) use std::fs;
    pub(crate) use std::io::{self, Read, Write};
    #[cfg(unix)]
    pub(crate) use std::os::unix::net::UnixStream;
    pub(crate) use std::path::{Path, PathBuf};
    pub(crate) use std::sync::mpsc::{Receiver, TryRecvError};
    pub(crate) use std::time::Duration;

    pub(crate) use sophia_portal::{
        MAX_NOTIFICATION_ACTION_LEN, MAX_NOTIFICATION_ACTIONS, MAX_NOTIFICATION_BODY_LEN,
        MAX_NOTIFICATION_SUMMARY_LEN, NotificationRequest, NotificationUrgency, PortalCommand,
    };
    pub(crate) use sophia_protocol::{
        AttentionState, BrokerHealthPacket, BufferSource, ChromeActionKind, ChromeActionRequest,
        ChromeDescriptor, CommittedSurfaceState, DamageFrame, DeviceId, DisplayLabel,
        FrameSnapshot, IconTokenId, InputEventKind, InputEventPacket, InputRoute,
        InputRouteOutcome, IpcCodecError, LayerSnapshot, LayoutNodeSnapshot, LayoutTransaction,
        OutputId, Point, PortalTransferId, Rect, Region, RenderCommand, RenderCommandKind,
        ResizeSyncCapability, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, SeatId, Size,
        SurfaceId, SurfaceTransaction, SurfaceTransactionReadiness, TransactionCommit,
        TransactionId, TransactionOutcome, TrustLevel, WmRequestKind, WmRequestPacket,
        WmResponsePacket, WorkspaceId, XLibreRoutedInputRequest, XWindowId,
        decode_wm_response_frame, encode_wm_request_frame,
    };
    pub(crate) use sophia_runtime::{
        RestartPolicy, RuntimeScanoutState, SessionRuntimeCommand, SessionRuntimeLoop,
        SessionRuntimeObservation, SessionRuntimeObservationError, SessionRuntimeState,
        SophiaErrorExt, SophiaErrorKind, SupervisedProcessKind, SupervisorCommand, SupervisorEvent,
        SupervisorState, update_supervisor,
    };
    pub(crate) use tracing::{debug, instrument, trace, warn};
}

mod backend_assembly;
mod chrome;
mod drm;
mod engine;
mod error;
mod frame;
mod input;
mod live_backend;
mod output;
mod render;
mod runtime_driver;
mod session;
mod visual_state;
mod wm;

pub use backend_assembly::*;
pub use chrome::*;
pub use drm::*;
pub use engine::*;
pub use error::*;
pub use frame::*;
pub use input::*;
pub use live_backend::*;
pub use output::*;
pub use render::*;
pub use runtime_driver::*;
pub use session::*;
pub use visual_state::*;
pub use wm::*;

pub use sophia_runtime::{RuntimeScanoutState, SessionRuntimeObservation};

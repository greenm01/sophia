mod live;
mod routed_input;
mod selection;
mod state;

mod prelude {
    pub(crate) use core::fmt;
    pub(crate) use std::collections::{BTreeMap, BTreeSet, VecDeque};
    pub(crate) use std::thread;
    pub(crate) use std::time::Duration;
    pub(crate) use std::{io::IoSlice, time::Instant};

    pub(crate) use sophia_portal::{
        ClipboardPortal, ClipboardTarget, ClipboardTransferRequest, PortalCommand, PortalError,
    };
    pub(crate) use sophia_protocol::{
        BufferSource, DamageFrame, DeviceId, InputEventKind, InputEventPacket, InputRoute,
        InputRouteOutcome, LayerSnapshot, NamespaceId, OutputId, Point, PortalTransferId, Rect,
        Region, ResizeSyncCapability, SeatId, Size, SurfaceId, SurfaceSnapshot, Transform,
        XLIBRE_ROUTED_INPUT_EXTENSION_NAME, XLIBRE_ROUTED_INPUT_ROUTE_EVENT_LENGTH,
        XLIBRE_ROUTED_INPUT_ROUTE_EVENT_OPCODE, XLibreRoutedInputDecision,
        XLibreRoutedInputOutcome, XLibreRoutedInputRequest, XLibreRoutedInputWireRequest,
        XWindowId, XWindowMirror,
    };
    pub(crate) use x11rb::connection::{Connection, RequestConnection};
    pub(crate) use x11rb::errors::ParseError;
    pub(crate) use x11rb::protocol::Event;
    pub(crate) use x11rb::protocol::composite::{
        ConnectionExt as CompositeConnectionExt, Redirect,
    };
    pub(crate) use x11rb::protocol::damage::{ConnectionExt as DamageConnectionExt, ReportLevel};
    pub(crate) use x11rb::protocol::xfixes::{
        ConnectionExt as XFixesConnectionExt, SelectionEvent, SelectionEventMask,
    };
    pub(crate) use x11rb::protocol::xinput::{
        ConnectionExt as XInputConnectionExt, Device, DeviceType, XIDeviceInfo,
    };
    pub(crate) use x11rb::protocol::xproto::{
        Atom, AtomEnum, ClientMessageData, ClientMessageEvent, ConnectionExt as _, CreateGCAux,
        CreateWindowAux, EventMask, ImageFormat, MapState, Place, PropMode, Rectangle,
        SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Timestamp, Window,
        WindowClass,
    };
    pub(crate) use x11rb::wrapper::ConnectionExt as X11WrapperConnectionExt;
    pub(crate) use x11rb::x11_utils::{Serialize, TryParse};
}

pub use live::*;
pub use routed_input::*;
pub use selection::*;
pub use state::*;
